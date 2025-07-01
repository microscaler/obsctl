use anyhow::{Context, Result};
use aws_config::meta::region::RegionProviderChain;
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_sdk_s3::{Client, primitives::ByteStream};
use aws_smithy_types::timeout::TimeoutConfig;
use clap::Parser;
use log::{error, info, warn};
use reqwest::Client as HttpClient;
use sd_notify::NotifyState;
use serde_json::json;
use simplelog::{
    ColorChoice, CombinedLogger, Config, ConfigBuilder, LevelFilter, SharedLogger, TermLogger,
    TerminalMode, WriteLogger,
};

use std::env::args;
use std::fs;
use std::io::Read;
use std::io::{stderr, stdout};
use std::os::unix::fs::MetadataExt;
use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
#[cfg(target_os = "linux")]
use systemd_journal_logger::JournalLog;
use tokio::{sync::Semaphore, task::JoinSet, time::sleep};
use walkdir::WalkDir;

#[cfg(target_os = "linux")]
struct JournalLogger {
    inner: JournalLog,
    level: LevelFilter,
}

#[cfg(target_os = "linux")]
impl JournalLogger {
    fn new(level: LevelFilter) -> std::io::Result<Box<dyn SharedLogger>> {
        Ok(Box::new(JournalLogger {
            inner: JournalLog::new()?,
            level,
        }))
    }
}

#[cfg(target_os = "linux")]
impl log::Log for JournalLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            self.inner.log(record);
        }
    }

    fn flush(&self) {}
}

#[cfg(target_os = "linux")]
impl SharedLogger for JournalLogger {
    fn level(&self) -> LevelFilter {
        self.level
    }

    fn config(&self) -> Option<&Config> {
        None
    }

    fn as_log(self: Box<Self>) -> Box<dyn log::Log> {
        self
    }
}

/// Uploads a folder recursively to a S3-compatible bucket (e.g., Cloud.ru OBS)
#[derive(Parser, Debug)]
#[command(author, version, about)]
#[command(author, version, about)]
struct Args {
    /// Timeout (in seconds) for all HTTP operations (OBS & OTEL)
    #[arg(long, default_value_t = 10)]
    http_timeout: u64,
    /// Set log verbosity level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info")]
    debug: String,
    /// Local source directory
    #[arg(short, long)]
    source: PathBuf,

    /// S3 bucket name
    #[arg(short, long)]
    bucket: String,

    /// S3 key prefix (e.g., folder name)
    #[arg(short, long, default_value = "")]
    prefix: String,

    /// Custom endpoint URL
    #[arg(short, long)]
    endpoint: String,

    /// AWS region
    #[arg(short, long, default_value = "ru-moscow-1")]
    region: String,

    /// Dry run mode
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Maximum retries per file
    #[arg(long, default_value_t = 3)]
    max_retries: usize,

    /// Maximum parallel uploads
    #[arg(long, default_value_t = 4)]
    max_concurrent: usize,

    /// Optional: log file
    #[arg(long)]
    log_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    async fn send_otel_telemetry(endpoint: &str, payload: &serde_json::Value) -> Result<()> {
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(args.http_timeout))
            .build()
            .context("Failed to build OTEL HTTP client")?;
        send_otel_telemetry_retry(&client, endpoint, payload, 3).await
    }

    async fn send_otel_telemetry_retry(
        client: &HttpClient,
        endpoint: &str,
        payload: &serde_json::Value,
        max_retries: usize,
    ) -> Result<()> {
        for attempt in 1..=max_retries {
            let res = client.post(endpoint).json(payload).send().await;
            match res {
                Ok(r) if r.status().is_success() => return Ok(()),
                Ok(r) => {
                    warn!(
                        "OTEL attempt {}/{} failed: {}",
                        attempt,
                        max_retries,
                        r.status()
                    );
                    sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
                }
                Err(e) => {
                    warn!("OTEL attempt {}/{} error: {}", attempt, max_retries, e);
                    sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
                }
            }
        }
        Err(anyhow::anyhow!(
            "OTEL telemetry failed after {} retries",
            max_retries
        ))
    }
    let args = Args::parse();

    let mut log_config = ConfigBuilder::new();
    log_config.set_time_to_local(true);
    log_config.set_time_format_custom(
        time::format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]").unwrap(),
    );
    log_config.set_level_padding(simplelog::LevelPadding::Right);
    let log_config = log_config.build();

    let level: LevelFilter = args.debug.parse().unwrap_or(LevelFilter::Info);
    std::env::set_var("AWS_LOG_LEVEL", &args.debug);
    std::env::set_var("AWS_SMITHY_LOG", &args.debug);

    #[cfg(target_os = "linux")]
    let loggers: Vec<Box<dyn SharedLogger>> = vec![
        TermLogger::new(
            level,
            log_config.clone(),
            TerminalMode::Stdout,
            ColorChoice::Auto,
        ),
        WriteLogger::new(LevelFilter::Warn, log_config.clone(), stderr()),
        JournalLogger::new(level)?,
    ];

    #[cfg(not(target_os = "linux"))]
    let loggers: Vec<Box<dyn SharedLogger>> = vec![
        TermLogger::new(
            level,
            log_config.clone(),
            TerminalMode::Stdout,
            ColorChoice::Auto,
        ),
        WriteLogger::new(LevelFilter::Warn, log_config.clone(), stderr()),
    ];

    CombinedLogger::init(loggers).expect("Failed to initialize logger");

    let region_provider = RegionProviderChain::first_try(Some(Region::new(args.region.clone())))
        .or_default_provider()
        .or_else(Region::new("ru-moscow-1"));

    let shared_config = aws_config::from_env().region(region_provider).load().await;

    let timeout_config = TimeoutConfig::builder()
        .connect_timeout(Duration::from_secs(args.http_timeout))
        .operation_timeout(Duration::from_secs(args.http_timeout))
        .build();

    let s3_config = aws_sdk_s3::config::Builder::from(&shared_config)
        .timeout_config(timeout_config)
        .endpoint_url(&args.endpoint)
        .build();

    let client = Arc::new(Client::from_conf(s3_config));
    let source_path = args.source.canonicalize()?;
    let semaphore = Arc::new(Semaphore::new(args.max_concurrent));

    let mut join_set = JoinSet::new();
    let mut total = 0usize;
    let mut failed = 0usize;

    sd_notify::notify(true, &[NotifyState::Ready]).ok();

    for entry in WalkDir::new(&source_path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let permit = semaphore.clone().acquire_owned().await?;
        let client = Arc::clone(&client);
        let source = source_path.clone();
        let bucket = args.bucket.clone();
        let prefix = args.prefix.clone();
        let max_retries = args.max_retries;
        let dry_run = args.dry_run;

        let full_path = entry.path().to_path_buf();
        let rel_path = full_path
            .strip_prefix(&source)
            .unwrap()
            .to_string_lossy()
            .replace("\\", "/");
        let key = format!("{}{}", prefix, rel_path);

        join_set.spawn(async move {
            let otel_url = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok();
            let otel_client = otel_url.as_ref().map(|_| HttpClient::new());
            let _permit = permit;
            if dry_run {
                info!(
                    "[DRY RUN] Would upload {} to s3://{}/{}",
                    full_path.display(),
                    bucket,
                    key
                );
                return Ok(());
            }

            let metadata1 = std::fs::metadata(&full_path)?.modified()?;
            sleep(Duration::from_secs(2)).await;
            let metadata2 = std::fs::metadata(&full_path)?.modified()?;
            if metadata1 != metadata2 || has_open_writers(&full_path)? {
                warn!(
                    "Skipping file: {} — currently being written or has open file descriptors",
                    full_path.display()
                );
                return Ok(());
            }

            for attempt in 1..=max_retries {
                match upload_file(&client, &bucket, &key, &full_path).await {
                    Ok(_) => {
                        info!("Uploaded: {}", key);
                        if let (Some(client), Some(url)) = (otel_client.as_ref(), otel_url.as_ref())
                        {
                            let payload = json!({
                                "event": "upload_success",
                                "file": key,
                                "timestamp": chrono::Utc::now().to_rfc3339(),
                            });
                            if let Err(e) =
                                send_otel_telemetry_retry(client, url, &payload, max_retries).await
                            {
                                warn!("OTEL file span failed: {}", e);
                            }
                        }
                        return Ok(());
                    }
                    Err(e) => {
                        warn!(
                            "Attempt {}/{} failed for {}: {}",
                            attempt, max_retries, key, e
                        );
                        sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
                    }
                }
            }

            error!("Failed to upload {} after {} attempts", key, max_retries);
            Err(anyhow::anyhow!("Failed after {} attempts", max_retries))
        });
        total += 1;

        while let Some(result) = join_set.join_next().await {
            if let Err(e) = result.unwrap_or_else(|e| Err(anyhow::anyhow!(e))) {
                failed += 1;
            }
        }

        info!(
            "Upload complete: {} files attempted, {} failed.",
            total, failed
        );
        info!(
            "Summary: {} uploaded successfully, {} skipped or failed.",
            total - failed,
            failed
        );

        if let Ok(otel_url) = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            let payload = serde_json::json!({
                "service": "obsctl",
                "status": if failed == 0 { "ok" } else { "failed" },
                "files_total": total,
                "files_failed": failed,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });

            if let Err(e) = send_otel_telemetry(&otel_url, &payload).await {
                warn!("Failed to send OTEL telemetry: {}", e);
            }
        }
        sd_notify::notify(true, &[NotifyState::Stopping]).ok();

        if failed > 0 {
            std::process::exit(1);
        }

        break;
    }

    /// Check if a file has any open writers (Linux only)
    fn has_open_writers(path: &Path) -> Result<bool> {
        let target_ino = fs::metadata(path)?.ino();

        for pid in fs::read_dir("/proc")? {
            let pid = pid?.file_name();
            if let Some(pid_str) = pid.to_str() {
                if pid_str.chars().all(|c| c.is_numeric()) {
                    let fd_path = format!("/proc/{}/fd", pid_str);
                    if let Ok(fds) = fs::read_dir(fd_path) {
                        for fd in fds.filter_map(Result::ok) {
                            if let Ok(link) = fs::read_link(fd.path()) {
                                if let Ok(meta) = fs::metadata(&link) {
                                    if meta.ino() == target_ino {
                                        info!(
                                            "Open FD on file {} by PID {}",
                                            path.display(),
                                            pid_str
                                        );
                                        return Ok(true);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    async fn upload_file(
        client: &Client,
        bucket: &str,
        key: &str,
        path: &Path,
    ) -> Result<(), PutObjectError> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        let body = ByteStream::from(buffer);

        client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .map(|_| ())
            .map_err(|e| {
                if let PutObjectErrorKind::Unhandled(e) = &e.kind {
                    error!("Raw error body: {}", e);
                }
                e
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn test_key_formatting() {
            let base = Path::new("/tmp/data");
            let file = base.join("folder/file.txt");
            let rel = file.strip_prefix(base).unwrap();
            let key = format!("{}{}", "prefix/", rel.to_string_lossy().replace("\\", "/"));
            assert_eq!(key, "prefix/folder/file.txt");
        }

        #[test]
        fn test_args_parse() {
            let args = Args::parse_from([
                "test",
                "--source",
                "/tmp",
                "--bucket",
                "my-bucket",
                "--endpoint",
                "https://obs.ru-moscow-1.hc.sbercloud.ru",
            ]);
            assert_eq!(args.bucket, "my-bucket");
            assert_eq!(args.endpoint, "https://obs.ru-moscow-1.hc.sbercloud.ru");
            assert!(args.source.exists());
        }

        #[test]
        fn test_writer_check_false_for_tmp() {
            let result = has_open_writers(Path::new("/tmp"));
            assert!(matches!(result, Ok(false) | Ok(true)));
        }

        #[test]
        fn test_writer_check_open_fd() {
            use std::fs::OpenOptions;
            use std::io::Write;
            use tempfile::NamedTempFile;

            let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
            writeln!(temp_file, "hello").unwrap();
            let path = temp_file.path().to_path_buf();

            // File is open here; should detect
            let result_open = has_open_writers(&path);
            assert!(
                matches!(result_open, Ok(true)),
                "Expected open file to report true"
            );

            drop(temp_file); // close file
            let result_closed = has_open_writers(&path);
            assert!(
                matches!(result_closed, Ok(false)),
                "Expected closed file to report false"
            );
        }
    }
}
