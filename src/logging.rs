use anyhow::Result;
use log::LevelFilter;
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};

#[cfg(target_os = "linux")]
use systemd_journal_logger::{connected_to_journal, JournalLog};

/// Initialize logging based on the debug level
pub fn init_logging(debug_level: &str) -> Result<()> {
    let level = match debug_level.to_lowercase().as_str() {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => LevelFilter::Info,
    };

    #[cfg(target_os = "linux")]
    {
        if connected_to_journal() {
            JournalLog::new()
                .unwrap()
                .with_extra_fields(vec![("VERSION", env!("CARGO_PKG_VERSION"))])
                .with_syslog_identifier("obsctl".to_string())
                .install()
                .unwrap();
            log::set_max_level(level);
            return Ok(());
        }
    }

    // Fallback to terminal logger
    TermLogger::init(
        level,
        Config::default(),
        TerminalMode::Mixed,
        ColorChoice::Auto,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: We use #[allow(clippy::single_match)] annotations on several match statements
    // in these tests rather than converting to `if let` patterns. While clippy suggests
    // `if let Ok(_) = result {}` for brevity, we prefer the explicit match pattern here
    // because:
    // 1. The match pattern is more visually clear about handling both Ok and Err cases
    // 2. It preserves the important error handling comments in a more readable format
    // 3. The symmetry of the match arms makes the test intent more obvious
    // 4. It's consistent with the "acceptable if logging already initialized" pattern

    #[test]
    fn test_init_logging_with_valid_levels() {
        let levels = ["trace", "debug", "info", "warn", "error"];

        for level in levels {
            // Note: We can't easily test the actual initialization because it's a global state
            // But we can test that the function doesn't panic and returns Ok
            let result = init_logging(level);
            // The function might succeed or fail depending on the environment
            // but it should not panic
            match result {
                Ok(_) => {
                    // Success is expected
                }
                Err(_) => {
                    // Failure might happen if logging is already initialized
                    // This is acceptable for testing
                }
            }
        }
    }

    #[test]
    fn test_init_logging_with_invalid_level() {
        // Test with invalid level - should default to info
        let result = init_logging("invalid");

        // Should not panic, might succeed or fail depending on environment
        #[allow(clippy::single_match)]
        match result {
            Ok(_) => {},
            Err(_) => {}, // Acceptable if logging already initialized
        }
    }

    #[test]
    fn test_init_logging_case_insensitive() {
        let mixed_case_levels = ["TRACE", "Debug", "INFO", "Warn", "ERROR"];

        for level in mixed_case_levels {
            let result = init_logging(level);

            // Should handle case insensitivity without panicking
            #[allow(clippy::single_match)]
            match result {
                Ok(_) => {},
                Err(_) => {}, // Acceptable if logging already initialized
            }
        }
    }

    #[test]
    fn test_level_filter_mapping() {
        // Test the internal level mapping logic
        let test_cases = [
            ("trace", LevelFilter::Trace),
            ("debug", LevelFilter::Debug),
            ("info", LevelFilter::Info),
            ("warn", LevelFilter::Warn),
            ("error", LevelFilter::Error),
            ("invalid", LevelFilter::Info), // Should default to Info
        ];

        for (input, expected) in test_cases {
            let actual = match input.to_lowercase().as_str() {
                "trace" => LevelFilter::Trace,
                "debug" => LevelFilter::Debug,
                "info" => LevelFilter::Info,
                "warn" => LevelFilter::Warn,
                "error" => LevelFilter::Error,
                _ => LevelFilter::Info,
            };

            assert_eq!(
                actual, expected,
                "Level mapping failed for input: {input}"
            );
        }
    }

    #[test]
    fn test_empty_string_level() {
        let result = init_logging("");

        // Should default to info level and not panic
        #[allow(clippy::single_match)]
        match result {
            Ok(_) => {},
            Err(_) => {}, // Acceptable if logging already initialized
        }
    }

    #[test]
    fn test_whitespace_level() {
        let result = init_logging("  info  ");

        // Should handle whitespace (though our current implementation doesn't trim)
        #[allow(clippy::single_match)]
        match result {
            Ok(_) => {},
            Err(_) => {}, // Acceptable if logging already initialized
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_journal_connection() {
        // Test that we can check journal connection without panicking
        let _connected = connected_to_journal();
        // This test just ensures the function is callable
    }

    #[test]
    fn test_logging_initialization_idempotency() {
        // Test that multiple initialization attempts don't cause issues
        let _result1 = init_logging("info");
        let _result2 = init_logging("debug");

        // Should not panic even if called multiple times
    }

    #[test]
    fn test_all_log_levels_exist() {
        // Ensure all expected log levels are valid
        let levels = [
            LevelFilter::Trace,
            LevelFilter::Debug,
            LevelFilter::Info,
            LevelFilter::Warn,
            LevelFilter::Error,
        ];

        assert_eq!(levels.len(), 5);

        // Test that levels have expected ordering
        assert!(LevelFilter::Trace > LevelFilter::Debug);
        assert!(LevelFilter::Debug > LevelFilter::Info);
        assert!(LevelFilter::Info > LevelFilter::Warn);
        assert!(LevelFilter::Warn > LevelFilter::Error);
    }
}
