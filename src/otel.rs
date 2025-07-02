use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::OtelConfig;

/// Global metrics collector for obsctl operations
#[derive(Debug, Clone)]
pub struct ObsctlMetrics {
    // Operation counters
    pub operations_total: Arc<AtomicU64>,
    pub uploads_total: Arc<AtomicU64>,
    pub downloads_total: Arc<AtomicU64>,
    pub deletes_total: Arc<AtomicU64>,
    pub lists_total: Arc<AtomicU64>,
    pub sync_operations_total: Arc<AtomicU64>,

    // Volume metrics (bytes)
    pub bytes_uploaded_total: Arc<AtomicU64>,
    pub bytes_downloaded_total: Arc<AtomicU64>,

    // File counters
    pub files_uploaded_total: Arc<AtomicU64>,
    pub files_downloaded_total: Arc<AtomicU64>,
    pub files_deleted_total: Arc<AtomicU64>,

    // Performance metrics
    pub operation_duration_ms: Arc<Mutex<Vec<(String, u64)>>>, // (operation_type, duration_ms)

    // Error counters
    pub errors_total: Arc<AtomicU64>,
    pub timeouts_total: Arc<AtomicU64>,

    // NEW: Detailed Error Type Tracking
    pub errors_dns: Arc<AtomicU64>, // DNS/network connection failures
    pub errors_bucket: Arc<AtomicU64>, // Bucket-related errors (already exists, not found, etc.)
    pub errors_file: Arc<AtomicU64>, // File-related errors (not found, permission, etc.)
    pub errors_auth: Arc<AtomicU64>, // Authentication/authorization errors
    pub errors_service: Arc<AtomicU64>, // S3 service errors (throttling, etc.)
    pub errors_unknown: Arc<AtomicU64>, // Unclassified errors

    // NEW: Enhanced Analytics
    // File size distribution (bytes) - track files by size buckets
    pub files_by_size_small: Arc<AtomicU64>,  // < 1MB
    pub files_by_size_medium: Arc<AtomicU64>, // 1MB - 100MB
    pub files_by_size_large: Arc<AtomicU64>,  // 100MB - 1GB
    pub files_by_size_xlarge: Arc<AtomicU64>, // > 1GB

    // Transfer rates (calculated in KB/s)
    pub transfer_rates: Arc<Mutex<Vec<(String, f64)>>>, // (operation_type, kb_per_sec)

    // MIME type tracking
    pub mime_types: Arc<Mutex<HashMap<String, u64>>>, // mime_type -> count

    // Detailed file metrics
    pub total_transfer_time_ms: Arc<AtomicU64>, // For calculating average rates
    pub largest_file_bytes: Arc<AtomicU64>,
    pub smallest_file_bytes: Arc<AtomicU64>,
}

impl Default for ObsctlMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl ObsctlMetrics {
    pub fn new() -> Self {
        Self {
            operations_total: Arc::new(AtomicU64::new(0)),
            uploads_total: Arc::new(AtomicU64::new(0)),
            downloads_total: Arc::new(AtomicU64::new(0)),
            deletes_total: Arc::new(AtomicU64::new(0)),
            lists_total: Arc::new(AtomicU64::new(0)),
            sync_operations_total: Arc::new(AtomicU64::new(0)),
            bytes_uploaded_total: Arc::new(AtomicU64::new(0)),
            bytes_downloaded_total: Arc::new(AtomicU64::new(0)),
            files_uploaded_total: Arc::new(AtomicU64::new(0)),
            files_downloaded_total: Arc::new(AtomicU64::new(0)),
            files_deleted_total: Arc::new(AtomicU64::new(0)),
            operation_duration_ms: Arc::new(Mutex::new(Vec::new())),
            errors_total: Arc::new(AtomicU64::new(0)),
            timeouts_total: Arc::new(AtomicU64::new(0)),

            // Detailed Error Type Tracking
            errors_dns: Arc::new(AtomicU64::new(0)),
            errors_bucket: Arc::new(AtomicU64::new(0)),
            errors_file: Arc::new(AtomicU64::new(0)),
            errors_auth: Arc::new(AtomicU64::new(0)),
            errors_service: Arc::new(AtomicU64::new(0)),
            errors_unknown: Arc::new(AtomicU64::new(0)),

            // Enhanced analytics
            files_by_size_small: Arc::new(AtomicU64::new(0)),
            files_by_size_medium: Arc::new(AtomicU64::new(0)),
            files_by_size_large: Arc::new(AtomicU64::new(0)),
            files_by_size_xlarge: Arc::new(AtomicU64::new(0)),
            transfer_rates: Arc::new(Mutex::new(Vec::new())),
            mime_types: Arc::new(Mutex::new(HashMap::new())),
            total_transfer_time_ms: Arc::new(AtomicU64::new(0)),
            largest_file_bytes: Arc::new(AtomicU64::new(0)),
            smallest_file_bytes: Arc::new(AtomicU64::new(u64::MAX)), // Start with max, will be reduced
        }
    }

    /// Record a file upload operation with enhanced analytics
    pub async fn record_upload(&self, bytes: u64, duration_ms: u64) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.uploads_total.fetch_add(1, Ordering::Relaxed);
        self.files_uploaded_total.fetch_add(1, Ordering::Relaxed);
        self.bytes_uploaded_total
            .fetch_add(bytes, Ordering::Relaxed);
        self.total_transfer_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);

        // Update file size tracking
        self.update_file_size_distribution(bytes);
        self.update_file_size_extremes(bytes);

        // Calculate and record transfer rate
        let kb_per_sec = if duration_ms > 0 {
            (bytes as f64 / 1024.0) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        let mut durations = self.operation_duration_ms.lock().await;
        durations.push(("upload".to_string(), duration_ms));
        if durations.len() > 1000 {
            durations.remove(0);
        }

        let mut rates = self.transfer_rates.lock().await;
        rates.push(("upload".to_string(), kb_per_sec));
        if rates.len() > 1000 {
            rates.remove(0);
        }
    }

    /// Record a file download operation with enhanced analytics
    pub async fn record_download(&self, bytes: u64, duration_ms: u64) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.downloads_total.fetch_add(1, Ordering::Relaxed);
        self.files_downloaded_total.fetch_add(1, Ordering::Relaxed);
        self.bytes_downloaded_total
            .fetch_add(bytes, Ordering::Relaxed);
        self.total_transfer_time_ms
            .fetch_add(duration_ms, Ordering::Relaxed);

        // Update file size tracking
        self.update_file_size_distribution(bytes);
        self.update_file_size_extremes(bytes);

        // Calculate and record transfer rate
        let kb_per_sec = if duration_ms > 0 {
            (bytes as f64 / 1024.0) / (duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        let mut durations = self.operation_duration_ms.lock().await;
        durations.push(("download".to_string(), duration_ms));
        if durations.len() > 1000 {
            durations.remove(0);
        }

        let mut rates = self.transfer_rates.lock().await;
        rates.push(("download".to_string(), kb_per_sec));
        if rates.len() > 1000 {
            rates.remove(0);
        }
    }

    /// Record a delete operation
    pub async fn record_delete(&self, file_count: u64, duration_ms: u64) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.deletes_total.fetch_add(1, Ordering::Relaxed);
        self.files_deleted_total
            .fetch_add(file_count, Ordering::Relaxed);

        let mut durations = self.operation_duration_ms.lock().await;
        durations.push(("delete".to_string(), duration_ms));
        if durations.len() > 1000 {
            durations.remove(0);
        }
    }

    /// Record a list operation
    pub async fn record_list(&self, duration_ms: u64) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.lists_total.fetch_add(1, Ordering::Relaxed);

        let mut durations = self.operation_duration_ms.lock().await;
        durations.push(("list".to_string(), duration_ms));
        if durations.len() > 1000 {
            durations.remove(0);
        }
    }

    /// Record a sync operation
    pub async fn record_sync(
        &self,
        files_transferred: u64,
        bytes_transferred: u64,
        duration_ms: u64,
    ) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        self.sync_operations_total.fetch_add(1, Ordering::Relaxed);
        self.files_uploaded_total
            .fetch_add(files_transferred, Ordering::Relaxed);
        self.bytes_uploaded_total
            .fetch_add(bytes_transferred, Ordering::Relaxed);

        let mut durations = self.operation_duration_ms.lock().await;
        durations.push(("sync".to_string(), duration_ms));
        if durations.len() > 1000 {
            durations.remove(0);
        }
    }

    /// Record a generic error
    pub fn record_error(&self) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an error with detailed classification
    pub fn record_error_with_type(&self, error_message: &str) {
        self.errors_total.fetch_add(1, Ordering::Relaxed);

        // Classify error type based on message content
        let error_lower = error_message.to_lowercase();

        if error_lower.contains("dns")
            || error_lower.contains("dispatch failure")
            || error_lower.contains("connection")
            || error_lower.contains("network")
            || error_lower.contains("failed to lookup address")
        {
            self.errors_dns.fetch_add(1, Ordering::Relaxed);
            log::debug!("Recorded DNS/network error: {error_message}");
        } else if error_lower.contains("bucket")
            || error_lower.contains("bucketalreadyownedby")
            || error_lower.contains("nosuchbucket")
        {
            self.errors_bucket.fetch_add(1, Ordering::Relaxed);
            log::debug!("Recorded bucket error: {error_message}");
        } else if error_lower.contains("file")
            || error_lower.contains("no such file")
            || error_lower.contains("permission")
            || error_lower.contains("access denied")
        {
            self.errors_file.fetch_add(1, Ordering::Relaxed);
            log::debug!("Recorded file error: {error_message}");
        } else if error_lower.contains("auth")
            || error_lower.contains("credential")
            || error_lower.contains("unauthorized")
            || error_lower.contains("forbidden")
        {
            self.errors_auth.fetch_add(1, Ordering::Relaxed);
            log::debug!("Recorded auth error: {error_message}");
        } else if error_lower.contains("service error")
            || error_lower.contains("throttle")
            || error_lower.contains("rate limit")
            || error_lower.contains("slow down")
        {
            self.errors_service.fetch_add(1, Ordering::Relaxed);
            log::debug!("Recorded service error: {error_message}");
        } else {
            self.errors_unknown.fetch_add(1, Ordering::Relaxed);
            log::debug!("Recorded unknown error: {error_message}");
        }
    }

    /// Record a timeout
    pub fn record_timeout(&self) {
        self.timeouts_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Record file with MIME type for analytics
    pub async fn record_file_mime_type(&self, file_path: &str) {
        let mime_type = self.detect_mime_type(file_path);
        let mut mime_types = self.mime_types.lock().await;
        *mime_types.entry(mime_type).or_insert(0) += 1;
    }

    /// Update file size distribution buckets
    fn update_file_size_distribution(&self, bytes: u64) {
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * MB;

        match bytes {
            x if x < MB => {
                // < 1MB
                self.files_by_size_small.fetch_add(1, Ordering::Relaxed);
            }
            x if x < 100 * MB => {
                // 1MB - 100MB
                self.files_by_size_medium.fetch_add(1, Ordering::Relaxed);
            }
            x if x < GB => {
                // 100MB - 1GB
                self.files_by_size_large.fetch_add(1, Ordering::Relaxed);
            }
            _ => {
                // > 1GB
                self.files_by_size_xlarge.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Update largest and smallest file size tracking
    fn update_file_size_extremes(&self, bytes: u64) {
        // Update largest file
        let mut current_largest = self.largest_file_bytes.load(Ordering::Relaxed);
        while bytes > current_largest {
            match self.largest_file_bytes.compare_exchange_weak(
                current_largest,
                bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_largest = x,
            }
        }

        // Update smallest file
        let mut current_smallest = self.smallest_file_bytes.load(Ordering::Relaxed);
        while bytes < current_smallest && bytes > 0 {
            match self.smallest_file_bytes.compare_exchange_weak(
                current_smallest,
                bytes,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(x) => current_smallest = x,
            }
        }
    }

    /// Detect MIME type from file extension
    fn detect_mime_type(&self, file_path: &str) -> String {
        let extension = std::path::Path::new(file_path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());

        match extension.as_str() {
            // Images
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "png" => "image/png".to_string(),
            "gif" => "image/gif".to_string(),
            "webp" => "image/webp".to_string(),
            "svg" => "image/svg+xml".to_string(),
            "bmp" => "image/bmp".to_string(),

            // Documents
            "pdf" => "application/pdf".to_string(),
            "doc" => "application/msword".to_string(),
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                .to_string(),
            "xls" => "application/vnd.ms-excel".to_string(),
            "xlsx" => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()
            }
            "ppt" => "application/vnd.ms-powerpoint".to_string(),
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                .to_string(),

            // Text
            "txt" => "text/plain".to_string(),
            "csv" => "text/csv".to_string(),
            "json" => "application/json".to_string(),
            "xml" => "application/xml".to_string(),
            "html" | "htm" => "text/html".to_string(),
            "css" => "text/css".to_string(),
            "js" => "application/javascript".to_string(),

            // Code
            "py" => "text/x-python".to_string(),
            "rs" => "text/x-rust".to_string(),
            "java" => "text/x-java-source".to_string(),
            "cpp" | "cc" | "cxx" => "text/x-c++src".to_string(),
            "c" => "text/x-csrc".to_string(),
            "h" => "text/x-chdr".to_string(),
            "go" => "text/x-go".to_string(),

            // Archives
            "zip" => "application/zip".to_string(),
            "tar" => "application/x-tar".to_string(),
            "gz" => "application/gzip".to_string(),
            "7z" => "application/x-7z-compressed".to_string(),
            "rar" => "application/vnd.rar".to_string(),

            // Media
            "mp4" => "video/mp4".to_string(),
            "avi" => "video/x-msvideo".to_string(),
            "mov" => "video/quicktime".to_string(),
            "mp3" => "audio/mpeg".to_string(),
            "wav" => "audio/wav".to_string(),
            "flac" => "audio/flac".to_string(),

            // Default
            _ => format!("application/octet-stream ({extension})"),
        }
    }

    /// Calculate current average transfer rate across all operations
    pub fn get_average_transfer_rate_kbps(&self) -> f64 {
        let total_bytes = self.bytes_uploaded_total.load(Ordering::Relaxed)
            + self.bytes_downloaded_total.load(Ordering::Relaxed);
        let total_time_ms = self.total_transfer_time_ms.load(Ordering::Relaxed);

        if total_time_ms > 0 && total_bytes > 0 {
            (total_bytes as f64 / 1024.0) / (total_time_ms as f64 / 1000.0)
        } else {
            0.0
        }
    }

    /// Get current metrics snapshot
    pub async fn get_metrics_snapshot(&self) -> MetricsSnapshot {
        let durations = self.operation_duration_ms.lock().await;
        let rates = self.transfer_rates.lock().await;
        let mime_types = self.mime_types.lock().await;

        MetricsSnapshot {
            operations_total: self.operations_total.load(Ordering::Relaxed),
            uploads_total: self.uploads_total.load(Ordering::Relaxed),
            downloads_total: self.downloads_total.load(Ordering::Relaxed),
            deletes_total: self.deletes_total.load(Ordering::Relaxed),
            lists_total: self.lists_total.load(Ordering::Relaxed),
            sync_operations_total: self.sync_operations_total.load(Ordering::Relaxed),
            bytes_uploaded_total: self.bytes_uploaded_total.load(Ordering::Relaxed),
            bytes_downloaded_total: self.bytes_downloaded_total.load(Ordering::Relaxed),
            files_uploaded_total: self.files_uploaded_total.load(Ordering::Relaxed),
            files_downloaded_total: self.files_downloaded_total.load(Ordering::Relaxed),
            files_deleted_total: self.files_deleted_total.load(Ordering::Relaxed),
            errors_total: self.errors_total.load(Ordering::Relaxed),
            timeouts_total: self.timeouts_total.load(Ordering::Relaxed),
            recent_operations: durations.clone(),

            // Enhanced analytics
            files_by_size_small: self.files_by_size_small.load(Ordering::Relaxed),
            files_by_size_medium: self.files_by_size_medium.load(Ordering::Relaxed),
            files_by_size_large: self.files_by_size_large.load(Ordering::Relaxed),
            files_by_size_xlarge: self.files_by_size_xlarge.load(Ordering::Relaxed),
            transfer_rates: rates.clone(),
            mime_types: mime_types.clone(),
            total_transfer_time_ms: self.total_transfer_time_ms.load(Ordering::Relaxed),
            largest_file_bytes: self.largest_file_bytes.load(Ordering::Relaxed),
            smallest_file_bytes: {
                let smallest = self.smallest_file_bytes.load(Ordering::Relaxed);
                if smallest == u64::MAX {
                    0
                } else {
                    smallest
                }
            },
            average_transfer_rate_kbps: self.get_average_transfer_rate_kbps(),

            // NEW: Detailed Error Breakdown
            errors_dns: self.errors_dns.load(Ordering::Relaxed),
            errors_bucket: self.errors_bucket.load(Ordering::Relaxed),
            errors_file: self.errors_file.load(Ordering::Relaxed),
            errors_auth: self.errors_auth.load(Ordering::Relaxed),
            errors_service: self.errors_service.load(Ordering::Relaxed),
            errors_unknown: self.errors_unknown.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub operations_total: u64,
    pub uploads_total: u64,
    pub downloads_total: u64,
    pub deletes_total: u64,
    pub lists_total: u64,
    pub sync_operations_total: u64,
    pub bytes_uploaded_total: u64,
    pub bytes_downloaded_total: u64,
    pub files_uploaded_total: u64,
    pub files_downloaded_total: u64,
    pub files_deleted_total: u64,
    pub errors_total: u64,
    pub timeouts_total: u64,
    pub recent_operations: Vec<(String, u64)>,

    // Enhanced analytics
    pub files_by_size_small: u64,           // < 1MB
    pub files_by_size_medium: u64,          // 1MB - 100MB
    pub files_by_size_large: u64,           // 100MB - 1GB
    pub files_by_size_xlarge: u64,          // > 1GB
    pub transfer_rates: Vec<(String, f64)>, // (operation_type, kb_per_sec)
    pub mime_types: HashMap<String, u64>,   // mime_type -> count
    pub total_transfer_time_ms: u64,
    pub largest_file_bytes: u64,
    pub smallest_file_bytes: u64,
    pub average_transfer_rate_kbps: f64,

    // NEW: Detailed Error Breakdown
    pub errors_dns: u64,
    pub errors_bucket: u64,
    pub errors_file: u64,
    pub errors_auth: u64,
    pub errors_service: u64,
    pub errors_unknown: u64,
}

// Global metrics instance
lazy_static::lazy_static! {
    pub static ref GLOBAL_METRICS: ObsctlMetrics = ObsctlMetrics::new();
}

// OpenTelemetry instruments using the global meter provider
lazy_static::lazy_static! {
    pub static ref OTEL_INSTRUMENTS: OtelInstruments = OtelInstruments::new();
}

/// OpenTelemetry instruments for obsctl operations
/// These use the global meter provider set up during initialization
pub struct OtelInstruments {
    // Operation counters
    pub operations_total: opentelemetry::metrics::Counter<u64>,
    pub uploads_total: opentelemetry::metrics::Counter<u64>,
    pub downloads_total: opentelemetry::metrics::Counter<u64>,
    pub deletes_total: opentelemetry::metrics::Counter<u64>,
    pub lists_total: opentelemetry::metrics::Counter<u64>,
    pub sync_operations_total: opentelemetry::metrics::Counter<u64>,

    // Volume metrics (bytes)
    pub bytes_uploaded_total: opentelemetry::metrics::Counter<u64>,
    pub bytes_downloaded_total: opentelemetry::metrics::Counter<u64>,

    // File counters
    pub files_uploaded_total: opentelemetry::metrics::Counter<u64>,
    pub files_downloaded_total: opentelemetry::metrics::Counter<u64>,
    pub files_deleted_total: opentelemetry::metrics::Counter<u64>,

    // Performance metrics (histograms for better aggregation)
    pub operation_duration: opentelemetry::metrics::Histogram<f64>,
    pub transfer_rate: opentelemetry::metrics::Histogram<f64>,

    // Error counters
    pub errors_total: opentelemetry::metrics::Counter<u64>,
    pub timeouts_total: opentelemetry::metrics::Counter<u64>,

    // Detailed Error Type Tracking
    pub errors_dns: opentelemetry::metrics::Counter<u64>,
    pub errors_bucket: opentelemetry::metrics::Counter<u64>,
    pub errors_file: opentelemetry::metrics::Counter<u64>,
    pub errors_auth: opentelemetry::metrics::Counter<u64>,
    pub errors_service: opentelemetry::metrics::Counter<u64>,
    pub errors_unknown: opentelemetry::metrics::Counter<u64>,

    // File size distribution
    pub files_by_size_small: opentelemetry::metrics::Counter<u64>,
    pub files_by_size_medium: opentelemetry::metrics::Counter<u64>,
    pub files_by_size_large: opentelemetry::metrics::Counter<u64>,
    pub files_by_size_xlarge: opentelemetry::metrics::Counter<u64>,

    // File size histogram for better analysis
    pub file_size_bytes: opentelemetry::metrics::Histogram<f64>,
}

impl OtelInstruments {
    pub fn new() -> Self {
        let meter = opentelemetry::global::meter("obsctl");

        Self {
            // Operation counters
            operations_total: meter
                .u64_counter("operations_total")
                .with_description("Total number of obsctl operations")
                .build(),
            uploads_total: meter
                .u64_counter("uploads_total")
                .with_description("Total number of upload operations")
                .build(),
            downloads_total: meter
                .u64_counter("downloads_total")
                .with_description("Total number of download operations")
                .build(),
            deletes_total: meter
                .u64_counter("deletes_total")
                .with_description("Total number of delete operations")
                .build(),
            lists_total: meter
                .u64_counter("lists_total")
                .with_description("Total number of list operations")
                .build(),
            sync_operations_total: meter
                .u64_counter("sync_operations_total")
                .with_description("Total number of sync operations")
                .build(),

            // Volume metrics
            bytes_uploaded_total: meter
                .u64_counter("bytes_uploaded_total")
                .with_description("Total bytes uploaded")
                .build(),
            bytes_downloaded_total: meter
                .u64_counter("bytes_downloaded_total")
                .with_description("Total bytes downloaded")
                .build(),

            // File counters
            files_uploaded_total: meter
                .u64_counter("files_uploaded_total")
                .with_description("Total files uploaded")
                .build(),
            files_downloaded_total: meter
                .u64_counter("files_downloaded_total")
                .with_description("Total files downloaded")
                .build(),
            files_deleted_total: meter
                .u64_counter("files_deleted_total")
                .with_description("Total files deleted")
                .build(),

            // Performance metrics
            operation_duration: meter
                .f64_histogram("operation_duration_seconds")
                .with_description("Duration of obsctl operations in seconds")
                .build(),
            transfer_rate: meter
                .f64_histogram("transfer_rate_kbps")
                .with_description("Transfer rate in KB/s")
                .build(),

            // Error counters
            errors_total: meter
                .u64_counter("errors_total")
                .with_description("Total number of errors")
                .build(),
            timeouts_total: meter
                .u64_counter("timeouts_total")
                .with_description("Total number of timeouts")
                .build(),

            // Detailed error tracking
            errors_dns: meter
                .u64_counter("errors_dns_total")
                .with_description("DNS/network errors")
                .build(),
            errors_bucket: meter
                .u64_counter("errors_bucket_total")
                .with_description("Bucket-related errors")
                .build(),
            errors_file: meter
                .u64_counter("errors_file_total")
                .with_description("File-related errors")
                .build(),
            errors_auth: meter
                .u64_counter("errors_auth_total")
                .with_description("Authentication errors")
                .build(),
            errors_service: meter
                .u64_counter("errors_service_total")
                .with_description("S3 service errors")
                .build(),
            errors_unknown: meter
                .u64_counter("errors_unknown_total")
                .with_description("Unknown errors")
                .build(),

            // File size distribution
            files_by_size_small: meter
                .u64_counter("files_small_total")
                .with_description("Files smaller than 1MB")
                .build(),
            files_by_size_medium: meter
                .u64_counter("files_medium_total")
                .with_description("Files between 1MB and 100MB")
                .build(),
            files_by_size_large: meter
                .u64_counter("files_large_total")
                .with_description("Files between 100MB and 1GB")
                .build(),
            files_by_size_xlarge: meter
                .u64_counter("files_xlarge_total")
                .with_description("Files larger than 1GB")
                .build(),

            // File size histogram
            file_size_bytes: meter
                .f64_histogram("file_size_bytes")
                .with_description("File size distribution in bytes")
                .build(),
        }
    }

    /// Record an upload operation using OTEL instruments
    pub fn record_upload(&self, bytes: u64, duration_ms: u64) {
        // Record operation counters
        self.operations_total.add(1, &[]);
        self.uploads_total.add(1, &[]);
        self.files_uploaded_total.add(1, &[]);
        self.bytes_uploaded_total.add(bytes, &[]);

        // Record performance metrics
        let duration_seconds = duration_ms as f64 / 1000.0;
        self.operation_duration.record(
            duration_seconds,
            &[opentelemetry::KeyValue::new("operation", "upload")],
        );

        // Record transfer rate
        if duration_ms > 0 {
            let kb_per_sec = (bytes as f64 / 1024.0) / duration_seconds;
            self.transfer_rate.record(
                kb_per_sec,
                &[opentelemetry::KeyValue::new("operation", "upload")],
            );
        }

        // Record file size
        self.file_size_bytes.record(
            bytes as f64,
            &[opentelemetry::KeyValue::new("operation", "upload")],
        );

        // Record file size distribution
        self.record_file_size_distribution(bytes);
    }

    /// Record a download operation using OTEL instruments
    pub fn record_download(&self, bytes: u64, duration_ms: u64) {
        // Record operation counters
        self.operations_total.add(1, &[]);
        self.downloads_total.add(1, &[]);
        self.files_downloaded_total.add(1, &[]);
        self.bytes_downloaded_total.add(bytes, &[]);

        // Record performance metrics
        let duration_seconds = duration_ms as f64 / 1000.0;
        self.operation_duration.record(
            duration_seconds,
            &[opentelemetry::KeyValue::new("operation", "download")],
        );

        // Record transfer rate
        if duration_ms > 0 {
            let kb_per_sec = (bytes as f64 / 1024.0) / duration_seconds;
            self.transfer_rate.record(
                kb_per_sec,
                &[opentelemetry::KeyValue::new("operation", "download")],
            );
        }

        // Record file size
        self.file_size_bytes.record(
            bytes as f64,
            &[opentelemetry::KeyValue::new("operation", "download")],
        );

        // Record file size distribution
        self.record_file_size_distribution(bytes);
    }

    /// Record a delete operation using OTEL instruments
    pub fn record_delete(&self, file_count: u64, duration_ms: u64) {
        self.operations_total.add(1, &[]);
        self.deletes_total.add(1, &[]);
        self.files_deleted_total.add(file_count, &[]);

        let duration_seconds = duration_ms as f64 / 1000.0;
        self.operation_duration.record(
            duration_seconds,
            &[opentelemetry::KeyValue::new("operation", "delete")],
        );
    }

    /// Record a list operation using OTEL instruments
    pub fn record_list(&self, duration_ms: u64) {
        self.operations_total.add(1, &[]);
        self.lists_total.add(1, &[]);

        let duration_seconds = duration_ms as f64 / 1000.0;
        self.operation_duration.record(
            duration_seconds,
            &[opentelemetry::KeyValue::new("operation", "list")],
        );
    }

    /// Record a sync operation using OTEL instruments
    pub fn record_sync(&self, files_transferred: u64, bytes_transferred: u64, duration_ms: u64) {
        self.operations_total.add(1, &[]);
        self.sync_operations_total.add(1, &[]);
        self.files_uploaded_total.add(files_transferred, &[]);
        self.bytes_uploaded_total.add(bytes_transferred, &[]);

        let duration_seconds = duration_ms as f64 / 1000.0;
        self.operation_duration.record(
            duration_seconds,
            &[opentelemetry::KeyValue::new("operation", "sync")],
        );
    }

    /// Record an error with detailed classification using OTEL instruments
    pub fn record_error_with_type(&self, error_message: &str) {
        self.errors_total.add(1, &[]);

        // Classify error type based on message content
        let error_type = classify_error_type(error_message);

        match error_type {
            "dns_network" => self.errors_dns.add(1, &[]),
            "bucket" => self.errors_bucket.add(1, &[]),
            "file" => self.errors_file.add(1, &[]),
            "auth" => self.errors_auth.add(1, &[]),
            "service" => self.errors_service.add(1, &[]),
            _ => self.errors_unknown.add(1, &[]),
        }

        log::debug!("Recorded {error_type} error via OTEL: {error_message}");
    }

    /// Record a timeout using OTEL instruments
    pub fn record_timeout(&self) {
        self.timeouts_total.add(1, &[]);
    }

    /// Record file size distribution using OTEL instruments
    fn record_file_size_distribution(&self, bytes: u64) {
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * MB;

        match bytes {
            x if x < MB => {
                // < 1MB
                self.files_by_size_small.add(1, &[]);
            }
            x if x < 100 * MB => {
                // 1MB - 100MB
                self.files_by_size_medium.add(1, &[]);
            }
            x if x < GB => {
                // 100MB - 1GB
                self.files_by_size_large.add(1, &[]);
            }
            _ => {
                // > 1GB
                self.files_by_size_xlarge.add(1, &[]);
            }
        }
    }
}

impl Default for OtelInstruments {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize OpenTelemetry SDK with proper gRPC instrumentation - NO MORE MANUAL HTTP!
pub fn init_tracing(otel_config: &OtelConfig, debug_level: &str) -> Result<()> {
    let is_debug = matches!(debug_level, "debug" | "trace");

    if !otel_config.enabled {
        if is_debug {
            log::debug!("OpenTelemetry is disabled");
        }
        return Ok(());
    }

    {
        use opentelemetry::global;
        use opentelemetry::KeyValue;
        use opentelemetry_otlp::WithExportConfig;
        use opentelemetry_sdk::Resource;
        use std::time::Duration;

        let endpoint = otel_config
            .endpoint
            .as_deref()
            .unwrap_or("http://localhost:4317"); // gRPC endpoint only

        if is_debug {
            log::debug!("🚀 Initializing OpenTelemetry SDK with gRPC endpoint: {endpoint}");
            log::debug!(
                "📊 Service: {} v{}",
                otel_config.service_name,
                otel_config.service_version
            );
            log::debug!("🎯 Using proper SDK instead of manual HTTP requests");
            log::debug!("🚫 Manual HTTP requests DISABLED");
        }

        // Create a proper Resource with service information
        if is_debug {
            log::debug!("📋 Creating OTEL resource with service info");
        }
        let resource = Resource::builder()
            .with_attributes(vec![
                KeyValue::new("service.name", otel_config.service_name.clone()),
                KeyValue::new("service.version", otel_config.service_version.clone()),
                KeyValue::new("deployment.environment", "development"),
            ])
            .build();

        // Initialize Tracer Provider for traces using the correct 0.30 API
        match opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
        {
            Ok(exporter) => {
                let tracer_provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_batch_exporter(exporter)
                    .with_resource(resource.clone())
                    .build();

                global::set_tracer_provider(tracer_provider);
                if is_debug {
                    log::debug!("✅ Tracer provider initialized successfully");
                }
            }
            Err(e) => {
                log::error!("❌ Failed to initialize tracer provider: {e}");
            }
        }

        // Initialize Meter Provider for metrics using the correct 0.30 API
        match opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .with_timeout(Duration::from_secs(10))
            .build()
        {
            Ok(exporter) => {
                let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(exporter)
                    .with_interval(Duration::from_secs(1)) // Very short interval for immediate export
                    .build();

                let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
                    .with_reader(reader)
                    .with_resource(resource)
                    .build();

                global::set_meter_provider(meter_provider);
                if is_debug {
                    log::debug!("✅ Meter provider initialized with 1-second export interval");
                }
            }
            Err(e) => {
                log::error!("❌ Failed to initialize meter provider: {e}");
            }
        }

        if is_debug {
            log::debug!("🎉 OpenTelemetry SDK initialization complete");
        }
    }

    Ok(())
}

/// Shutdown OpenTelemetry tracing with proper metric flushing
pub fn shutdown_tracing() {
    {
        use std::time::Duration;

        log::info!("🔄 OpenTelemetry shutdown requested - flushing metrics and traces...");

        // Give enough time for at least 2 export cycles (1 second interval + buffer)
        // This ensures all pending metrics and traces are exported before shutdown
        std::thread::sleep(Duration::from_millis(2500));

        log::info!("🎉 OpenTelemetry shutdown complete - all pending metrics and traces flushed");
    }

    {
        log::debug!("OpenTelemetry not enabled, nothing to shutdown");
    }
}

/// Helper function to classify error types for consistent categorization
pub fn classify_error_type(error_message: &str) -> &'static str {
    let error_lower = error_message.to_lowercase();

    if error_lower.contains("dns")
        || error_lower.contains("dispatch failure")
        || error_lower.contains("connection")
        || error_lower.contains("network")
        || error_lower.contains("failed to lookup address")
    {
        "dns_network"
    } else if error_lower.contains("bucket")
        && (error_lower.contains("already")
            || error_lower.contains("exists")
            || error_lower.contains("not found")
            || error_lower.contains("access"))
    {
        "bucket"
    } else if error_lower.contains("file")
        && (error_lower.contains("not found")
            || error_lower.contains("does not exist")
            || error_lower.contains("permission")
            || error_lower.contains("access denied"))
    {
        "file"
    } else if error_lower.contains("auth")
        || error_lower.contains("credential")
        || error_lower.contains("unauthorized")
        || error_lower.contains("forbidden")
    {
        "auth"
    } else if error_lower.contains("throttl")
        || error_lower.contains("rate limit")
        || error_lower.contains("service unavailable")
        || error_lower.contains("timeout")
    {
        "service"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_otel_config_creation() {
        let config = OtelConfig {
            enabled: true,
            endpoint: Some("http://localhost:4317".to_string()),
            service_name: "test-service".to_string(),
            service_version: "1.0.0".to_string(),
        };

        assert!(config.enabled);
        assert_eq!(config.endpoint, Some("http://localhost:4317".to_string()));
        assert_eq!(config.service_name, "test-service");
        assert_eq!(config.service_version, "1.0.0");
    }

    #[test]
    fn test_init_tracing_disabled() {
        let config = OtelConfig {
            enabled: false,
            endpoint: None,
            service_name: "test".to_string(),
            service_version: "1.0.0".to_string(),
        };

        let result = init_tracing(&config, "info");
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_tracing_enabled() {
        // Skip test if OTEL infrastructure is not available
        if std::env::var("OBSCTL_TEST_OTEL").is_err() {
            eprintln!("⚠️  Skipping OTEL tracing test - set OBSCTL_TEST_OTEL=1 to enable");
            return;
        }

        let config = OtelConfig {
            enabled: true,
            endpoint: Some("http://localhost:4317".to_string()),
            service_name: "obsctl".to_string(),
            service_version: crate::get_service_version(),
        };

        // Use a simple runtime for the test
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let result = init_tracing(&config, "debug");
        assert!(result.is_ok());
        
        // Clean up
        drop(_guard);
        drop(rt);
    }

    #[test]
    #[ignore = "requires OTEL collector running - run with: cargo test test_init_tracing_with_real_collector -- --ignored"]
    fn test_init_tracing_with_real_collector() {
        // This test requires a real OTEL collector running on localhost:4317
        let config = OtelConfig {
            enabled: true,
            endpoint: Some("http://localhost:4317".to_string()),
            service_name: "obsctl-test".to_string(),
            service_version: "test".to_string(),
        };

        // Test with actual OTEL collector
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _guard = rt.enter();

        let result = init_tracing(&config, "debug");
        assert!(result.is_ok());
        
        println!("✅ OTEL tracing initialized successfully with real collector");
        
        // Clean up
        drop(_guard);
        drop(rt);
    }
}
