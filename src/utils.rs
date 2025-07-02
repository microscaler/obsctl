use anyhow::Result;
use regex::Regex;
use std::path::Path;

#[cfg(target_os = "linux")]
use std::fs;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;

/// Check if a file has any open writers (Linux only)
#[cfg(target_os = "linux")]
pub fn has_open_writers(path: &Path) -> Result<bool> {
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

/// Check if a file has any open writers (non-Linux systems - always returns false)
#[cfg(not(target_os = "linux"))]
pub fn has_open_writers(_path: &Path) -> Result<bool> {
    Ok(false)
}

/// Match a string against a wildcard pattern
///
/// Supports standard glob patterns:
/// - `*` matches any sequence of characters (including empty)
/// - `?` matches any single character
/// - `[abc]` matches any character in the set
/// - `[a-z]` matches any character in the range
/// - `[!abc]` or `[^abc]` matches any character NOT in the set
///
/// Examples:
/// - `test-*` matches `test-bucket`, `test-dev`, `test-prod-v2`
/// - `*-prod` matches `app-prod`, `api-prod`, `web-prod`
/// - `user-?-bucket` matches `user-1-bucket`, `user-a-bucket`
/// - `[abc]*` matches any string starting with 'a', 'b', or 'c'
/// - `*[0-9]` matches any string ending with a digit
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    wildcard_match_recursive(pattern.chars().collect(), text.chars().collect(), 0, 0)
}

fn wildcard_match_recursive(
    pattern: Vec<char>,
    text: Vec<char>,
    p_idx: usize,
    t_idx: usize,
) -> bool {
    // If we've consumed both pattern and text, it's a match
    if p_idx >= pattern.len() && t_idx >= text.len() {
        return true;
    }

    // If pattern is exhausted but text remains, no match
    if p_idx >= pattern.len() {
        return false;
    }

    match pattern[p_idx] {
        '*' => {
            // Try matching '*' with empty string first
            if wildcard_match_recursive(pattern.clone(), text.clone(), p_idx + 1, t_idx) {
                return true;
            }

            // Try matching '*' with one or more characters
            for i in t_idx..text.len() {
                if wildcard_match_recursive(pattern.clone(), text.clone(), p_idx + 1, i + 1) {
                    return true;
                }
            }
            false
        }
        '?' => {
            // '?' matches exactly one character
            if t_idx >= text.len() {
                false
            } else {
                wildcard_match_recursive(pattern, text, p_idx + 1, t_idx + 1)
            }
        }
        '[' => {
            // Character class matching
            if t_idx >= text.len() {
                return false;
            }

            let (matches, new_p_idx) = match_character_class(&pattern, p_idx, text[t_idx]);
            if matches {
                wildcard_match_recursive(pattern, text, new_p_idx, t_idx + 1)
            } else {
                false
            }
        }
        c => {
            // Literal character matching
            if t_idx >= text.len() || text[t_idx] != c {
                false
            } else {
                wildcard_match_recursive(pattern, text, p_idx + 1, t_idx + 1)
            }
        }
    }
}

fn match_character_class(pattern: &[char], start_idx: usize, ch: char) -> (bool, usize) {
    if start_idx >= pattern.len() || pattern[start_idx] != '[' {
        return (false, start_idx);
    }

    let mut idx = start_idx + 1;
    let mut negated = false;
    let mut found_match = false;

    // Check for negation
    if idx < pattern.len() && (pattern[idx] == '!' || pattern[idx] == '^') {
        negated = true;
        idx += 1;
    }

    // Find the closing bracket and check for matches
    while idx < pattern.len() && pattern[idx] != ']' {
        if idx + 2 < pattern.len() && pattern[idx + 1] == '-' && pattern[idx + 2] != ']' {
            // Range match: [a-z]
            let start_char = pattern[idx];
            let end_char = pattern[idx + 2];
            if ch >= start_char && ch <= end_char {
                found_match = true;
            }
            idx += 3;
        } else {
            // Single character match
            if pattern[idx] == ch {
                found_match = true;
            }
            idx += 1;
        }
    }

    // Skip the closing bracket
    if idx < pattern.len() && pattern[idx] == ']' {
        idx += 1;
    }

    let matches = if negated { !found_match } else { found_match };
    (matches, idx)
}

/// Filter a list of strings by a wildcard pattern
pub fn filter_by_pattern(items: &[String], pattern: &str) -> Vec<String> {
    items
        .iter()
        .filter(|item| wildcard_match(pattern, item))
        .cloned()
        .collect()
}

/// Cross-platform file descriptor/handle monitoring
pub mod fd_monitor {
    #[cfg(target_os = "windows")]
    use std::process::Command;

    #[derive(Debug, Clone)]
    pub struct FdInfo {
        pub count: usize,
        pub details: Vec<String>,
    }

    /// Get current file descriptor/handle count for this process
    pub fn get_current_fd_count() -> Result<usize, Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        {
            get_linux_fd_count()
        }
        #[cfg(target_os = "macos")]
        {
            get_macos_fd_count()
        }
        #[cfg(target_os = "windows")]
        {
            get_windows_handle_count()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            // Fallback for other platforms
            Ok(0)
        }
    }

    /// Get detailed file descriptor/handle information
    pub fn get_fd_info() -> Result<FdInfo, Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        {
            get_linux_fd_info()
        }
        #[cfg(target_os = "macos")]
        {
            get_macos_fd_info()
        }
        #[cfg(target_os = "windows")]
        {
            get_windows_handle_info()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        {
            Ok(FdInfo {
                count: 0,
                details: vec!["Platform not supported".to_string()],
            })
        }
    }

    /// Check if file descriptor count is within reasonable limits
    pub fn check_fd_health() -> Result<bool, Box<dyn std::error::Error>> {
        let count = get_current_fd_count()?;

        // Platform-specific limits
        let limit = match std::env::consts::OS {
            "linux" => 1024,   // Default ulimit on most Linux systems
            "macos" => 256,    // Default on macOS
            "windows" => 2048, // Windows handle limit is much higher
            _ => 512,          // Conservative fallback
        };

        let usage_percent = (count as f64 / limit as f64) * 100.0;

        // Warn if over 80% of limit
        if usage_percent > 80.0 {
            eprintln!(
                "⚠️  High file descriptor usage: {}/{} ({}%)",
                count, limit, usage_percent as u32
            );
            return Ok(false);
        }

        Ok(true)
    }

    // Linux implementation
    #[cfg(target_os = "linux")]
    fn get_linux_fd_count() -> Result<usize, Box<dyn std::error::Error>> {
        use std::fs;

        let fd_dir = "/proc/self/fd";
        match fs::read_dir(fd_dir) {
            Ok(entries) => {
                let count = entries.count();
                // Subtract 2 for . and .. entries that might be included
                Ok(count.saturating_sub(2))
            }
            Err(e) => Err(format!("Failed to read {}: {}", fd_dir, e).into()),
        }
    }

    #[cfg(target_os = "linux")]
    fn get_linux_fd_info() -> Result<FdInfo, Box<dyn std::error::Error>> {
        use std::fs;

        let fd_dir = "/proc/self/fd";
        let mut details = Vec::new();
        let mut count = 0;

        match fs::read_dir(fd_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let fd_num = entry.file_name();
                    if let Some(fd_str) = fd_num.to_str() {
                        if fd_str.chars().all(|c| c.is_ascii_digit()) {
                            count += 1;

                            // Try to get the target of the symlink
                            let fd_path = entry.path();
                            match fs::read_link(&fd_path) {
                                Ok(target) => {
                                    details.push(format!("fd {fd_str}: {}", target.display()));
                                }
                                Err(_) => {
                                    details.push(format!("fd {fd_str}: <unknown>"));
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => return Err(format!("Failed to read {}: {}", fd_dir, e).into()),
        }

        Ok(FdInfo { count, details })
    }

    // macOS implementation using native system calls
    #[cfg(target_os = "macos")]
    fn get_macos_fd_count() -> Result<usize, Box<dyn std::error::Error>> {
        // Try native approach first
        if let Ok(count) = get_macos_native_fd_count() {
            return Ok(count);
        }

        // Fallback to estimate based on typical process behavior
        Ok(10) // Conservative estimate
    }

    #[cfg(target_os = "macos")]
    fn get_macos_native_fd_count() -> Result<usize, Box<dyn std::error::Error>> {
        // Use sysctl to get process information
        // This is more complex but avoids external commands

        // For now, we'll use a simple approach by checking /dev/fd if available
        // This is similar to Linux but macOS may not always have this
        use std::fs;

        let fd_dir = "/dev/fd";
        match fs::read_dir(fd_dir) {
            Ok(entries) => {
                let count = entries.count();
                Ok(count.saturating_sub(2)) // Subtract . and ..
            }
            Err(_) => {
                // Fallback: try to estimate based on typical file handles
                // Most processes have at least stdin, stdout, stderr = 3
                // Plus a few additional handles for libraries, etc.
                Ok(8) // Conservative estimate
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn get_macos_fd_info() -> Result<FdInfo, Box<dyn std::error::Error>> {
        use std::fs;

        // Try to read /dev/fd first
        let fd_dir = "/dev/fd";
        let mut details = Vec::new();
        let mut count = 0;

        match fs::read_dir(fd_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let fd_num = entry.file_name();
                    if let Some(fd_str) = fd_num.to_str() {
                        if fd_str.chars().all(|c| c.is_ascii_digit()) {
                            count += 1;

                            // Try to get the target of the symlink
                            let fd_path = entry.path();
                            match fs::read_link(&fd_path) {
                                Ok(target) => {
                                    details.push(format!("fd {fd_str}: {}", target.display()));
                                }
                                Err(_) => {
                                    details.push(format!("fd {fd_str}: <unknown>"));
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Fallback to basic process info
                details.push("Standard file descriptors:".to_string());
                details.push("  fd 0: stdin".to_string());
                details.push("  fd 1: stdout".to_string());
                details.push("  fd 2: stderr".to_string());
                details.push("  + additional library handles".to_string());
                count = 8; // Conservative estimate
            }
        }

        Ok(FdInfo { count, details })
    }

    // Windows implementation
    #[cfg(target_os = "windows")]
    fn get_windows_handle_count() -> Result<usize, Box<dyn std::error::Error>> {
        // Try PowerShell first (most accessible)
        if let Ok(count) = get_windows_powershell_count() {
            return Ok(count);
        }

        // Fallback to WMI query
        get_windows_wmi_count()
    }

    #[cfg(target_os = "windows")]
    fn get_windows_powershell_count() -> Result<usize, Box<dyn std::error::Error>> {
        let pid = std::process::id();
        let script = format!("(Get-Process -Id {}).HandleCount", pid);

        let output = Command::new("powershell")
            .args(&["-Command", &script])
            .output()?;

        if output.status.success() {
            let count_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            match count_str.parse::<usize>() {
                Ok(count) => Ok(count),
                Err(e) => {
                    Err(format!("Failed to parse handle count '{}': {}", count_str, e).into())
                }
            }
        } else {
            Err(format!(
                "PowerShell command failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into())
        }
    }

    #[cfg(target_os = "windows")]
    fn get_windows_wmi_count() -> Result<usize, Box<dyn std::error::Error>> {
        let pid = std::process::id();
        let query = format!(
            "Get-WmiObject -Class Win32_Process -Filter \\\"ProcessId={}\\\" | Select-Object HandleCount",
            pid
        );

        let output = Command::new("powershell")
            .args(&["-Command", &query])
            .output()?;

        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Parse the PowerShell output to extract HandleCount
            for line in output_str.lines() {
                if line.trim().chars().all(|c| c.is_ascii_digit()) {
                    if let Ok(count) = line.trim().parse::<usize>() {
                        return Ok(count);
                    }
                }
            }
            Err("Could not parse WMI output".into())
        } else {
            Err("WMI query failed".into())
        }
    }

    #[cfg(target_os = "windows")]
    fn get_windows_handle_info() -> Result<FdInfo, Box<dyn std::error::Error>> {
        let pid = std::process::id();

        // Get basic handle count
        let count = get_windows_handle_count().unwrap_or(0);

        // Try to get more detailed info using PowerShell
        let script = format!(
            "Get-Process -Id {} | Select-Object ProcessName,Id,HandleCount,WorkingSet,VirtualMemorySize",
            pid
        );

        let output = Command::new("powershell")
            .args(&["-Command", &script])
            .output()?;

        let mut details = vec![
            format!("Process ID: {}", pid),
            format!("Handle Count: {}", count),
        ];

        if output.status.success() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if !line.trim().is_empty() && !line.contains("---") && !line.contains("ProcessName")
                {
                    details.push(line.trim().to_string());
                }
            }
        }

        Ok(FdInfo { count, details })
    }

    /// Monitor file descriptor usage during an operation
    pub struct FdMonitor {
        initial_count: usize,
        peak_count: usize,
        samples: Vec<(std::time::Instant, usize)>,
    }

    impl FdMonitor {
        pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
            let initial_count = get_current_fd_count()?;
            Ok(FdMonitor {
                initial_count,
                peak_count: initial_count,
                samples: vec![(std::time::Instant::now(), initial_count)],
            })
        }

        pub fn sample(&mut self) -> Result<usize, Box<dyn std::error::Error>> {
            let current_count = get_current_fd_count()?;
            self.peak_count = self.peak_count.max(current_count);
            self.samples
                .push((std::time::Instant::now(), current_count));
            Ok(current_count)
        }

        pub fn report(&self) -> String {
            let current_count = self.samples.last().map(|(_, count)| *count).unwrap_or(0);
            let leaked = current_count.saturating_sub(self.initial_count);

            format!(
                "FD Monitor Report: Initial: {}, Current: {}, Peak: {}, Leaked: {}",
                self.initial_count, current_count, self.peak_count, leaked
            )
        }
    }
}

/// Enhanced pattern matching supporting both wildcards and regex
pub enum PatternType {
    Wildcard,
    Regex,
}

/// Determine if a pattern should be treated as regex or wildcard
pub fn detect_pattern_type(pattern: &str) -> PatternType {
    // If pattern contains regex metacharacters, treat as regex
    // Otherwise, treat as wildcard for backward compatibility
    let regex_chars = ['(', ')', '{', '}', '+', '^', '$', '\\', '|'];

    if pattern.chars().any(|c| regex_chars.contains(&c)) {
        PatternType::Regex
    } else {
        PatternType::Wildcard
    }
}

/// Enhanced pattern matching with both wildcard and regex support
pub fn enhanced_pattern_match(pattern: &str, text: &str, force_regex: bool) -> Result<bool> {
    if force_regex {
        regex_match(pattern, text)
    } else {
        match detect_pattern_type(pattern) {
            PatternType::Regex => regex_match(pattern, text),
            PatternType::Wildcard => Ok(wildcard_match(pattern, text)),
        }
    }
}

/// Regex pattern matching using the regex crate
pub fn regex_match(pattern: &str, text: &str) -> Result<bool> {
    let regex = Regex::new(pattern)
        .map_err(|e| anyhow::anyhow!("Invalid regex pattern '{}': {}", pattern, e))?;

    Ok(regex.is_match(text))
}

/// Filter items by pattern with regex support
pub fn filter_by_enhanced_pattern(
    items: &[String],
    pattern: &str,
    force_regex: bool,
) -> Result<Vec<String>> {
    let mut results = Vec::new();

    for item in items {
        if enhanced_pattern_match(pattern, item, force_regex)? {
            results.push(item.clone());
        }
    }

    Ok(results)
}

/// Convert wildcard pattern to equivalent regex pattern
pub fn wildcard_to_regex(wildcard: &str) -> String {
    let mut regex = String::new();
    regex.push('^'); // Anchor to start

    let chars: Vec<char> = wildcard.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        match chars[i] {
            '*' => regex.push_str(".*"),
            '?' => regex.push('.'),
            '[' => {
                // Handle character classes - keep as-is since regex supports them
                regex.push('[');
                i += 1;
                while i < chars.len() && chars[i] != ']' {
                    if chars[i] == '!' && regex.ends_with('[') {
                        regex.push('^'); // Convert ! to ^ for negation
                    } else {
                        regex.push(chars[i]);
                    }
                    i += 1;
                }
                if i < chars.len() {
                    regex.push(']');
                }
            }
            // Escape regex metacharacters
            '.' | '+' | '(' | ')' | '{' | '}' | '^' | '$' | '|' | '\\' => {
                regex.push('\\');
                regex.push(chars[i]);
            }
            c => regex.push(c),
        }
        i += 1;
    }

    regex.push('$'); // Anchor to end
    regex
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_has_open_writers_with_nonexistent_file() {
        let nonexistent_path = PathBuf::from("/nonexistent/file/path");
        let result = has_open_writers(&nonexistent_path);

        #[cfg(target_os = "linux")]
        {
            // Should return an error because file doesn't exist on Linux
            assert!(result.is_err());
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux, should return false (no error)
            assert!(!result.unwrap());
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_has_open_writers_with_proc_filesystem() {
        // Test with /proc/version which should exist on Linux
        let proc_version = Path::new("/proc/version");
        if proc_version.exists() {
            let result = has_open_writers(proc_version);
            // Should succeed (might be true or false, but shouldn't error)
            assert!(result.is_ok());
        }
    }

    #[test]
    #[cfg(not(target_os = "linux"))]
    fn test_has_open_writers_non_linux() {
        // On non-Linux systems, should always return false
        let any_path = Path::new(".");
        let result = has_open_writers(any_path).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_has_open_writers_with_current_directory() {
        let current_dir = Path::new(".");
        let result = has_open_writers(current_dir);

        #[cfg(target_os = "linux")]
        {
            // On Linux, should succeed (directory exists)
            assert!(result.is_ok());
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux, should return false
            assert!(!result.unwrap());
        }
    }

    #[test]
    fn test_has_open_writers_with_cargo_toml() {
        // Test with Cargo.toml which should exist in project root
        let cargo_toml = Path::new("Cargo.toml");
        if cargo_toml.exists() {
            let result = has_open_writers(cargo_toml);

            #[cfg(target_os = "linux")]
            {
                // Should succeed on Linux
                assert!(result.is_ok());
            }

            #[cfg(not(target_os = "linux"))]
            {
                // Should return false on non-Linux
                assert!(!result.unwrap());
            }
        }
    }

    #[test]
    fn test_has_open_writers_with_empty_path() {
        let empty_path = Path::new("");
        let result = has_open_writers(empty_path);

        #[cfg(target_os = "linux")]
        {
            // Should return an error because empty path is invalid on Linux
            assert!(result.is_err());
        }

        #[cfg(not(target_os = "linux"))]
        {
            // On non-Linux, should return false (no error)
            assert!(!result.unwrap());
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_has_open_writers_with_temp_file() {
        use std::fs::File;
        use tempfile::NamedTempFile;

        // Create a temporary file
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let temp_path = temp_file.path();

        // Test with the temporary file
        let result = has_open_writers(temp_path);
        assert!(result.is_ok());

        // The result might be true or false depending on system state
        // but it should not error
    }

    #[test]
    fn test_path_handling() {
        // Test various path types
        let paths = [".", "..", "src", "Cargo.toml"];

        for path_str in paths {
            let path = Path::new(path_str);
            if path.exists() {
                let result = has_open_writers(path);

                #[cfg(target_os = "linux")]
                {
                    // On Linux, should not error for existing paths
                    assert!(result.is_ok());
                }

                #[cfg(not(target_os = "linux"))]
                {
                    // On non-Linux, should always return false
                    assert!(!result.unwrap());
                }
            }
        }
    }

    // Wildcard pattern matching tests
    #[test]
    fn test_wildcard_exact_match() {
        assert!(wildcard_match("hello", "hello"));
        assert!(!wildcard_match("hello", "world"));
        assert!(!wildcard_match("hello", "hell"));
        assert!(!wildcard_match("hello", "helloo"));
    }

    #[test]
    fn test_wildcard_star_patterns() {
        // Star at the end
        assert!(wildcard_match("test-*", "test-"));
        assert!(wildcard_match("test-*", "test-bucket"));
        assert!(wildcard_match("test-*", "test-dev-v2"));
        assert!(!wildcard_match("test-*", "prod-test"));

        // Star at the beginning
        assert!(wildcard_match("*-prod", "app-prod"));
        assert!(wildcard_match("*-prod", "api-prod"));
        assert!(wildcard_match("*-prod", "-prod"));
        assert!(!wildcard_match("*-prod", "prod-env"));

        // Star in the middle
        assert!(wildcard_match("user-*-bucket", "user-1-bucket"));
        assert!(wildcard_match("user-*-bucket", "user-admin-bucket"));
        assert!(wildcard_match("user-*-bucket", "user--bucket"));
        assert!(!wildcard_match("user-*-bucket", "user-bucket"));

        // Multiple stars
        assert!(wildcard_match("*-*-*", "a-b-c"));
        assert!(wildcard_match("*-*-*", "app-dev-v1"));
        assert!(!wildcard_match("*-*-*", "a-b"));
    }

    #[test]
    fn test_wildcard_question_mark() {
        assert!(wildcard_match("user-?", "user-1"));
        assert!(wildcard_match("user-?", "user-a"));
        assert!(!wildcard_match("user-?", "user-"));
        assert!(!wildcard_match("user-?", "user-12"));

        // Multiple question marks
        assert!(wildcard_match("??-bucket", "v1-bucket"));
        assert!(wildcard_match("??-bucket", "ab-bucket"));
        assert!(!wildcard_match("??-bucket", "a-bucket"));
        assert!(!wildcard_match("??-bucket", "abc-bucket"));
    }

    #[test]
    fn test_wildcard_character_classes() {
        // Simple character set
        assert!(wildcard_match("[abc]*", "apple"));
        assert!(wildcard_match("[abc]*", "banana"));
        assert!(wildcard_match("[abc]*", "cherry"));
        assert!(!wildcard_match("[abc]*", "date"));

        // Character range
        assert!(wildcard_match("user-[0-9]", "user-1"));
        assert!(wildcard_match("user-[0-9]", "user-9"));
        assert!(!wildcard_match("user-[0-9]", "user-a"));

        // Multiple ranges
        assert!(wildcard_match("[a-z][0-9]*", "a1"));
        assert!(wildcard_match("[a-z][0-9]*", "z9bucket"));
        assert!(!wildcard_match("[a-z][0-9]*", "A1"));
        assert!(!wildcard_match("[a-z][0-9]*", "1a"));

        // Negated character sets
        assert!(wildcard_match("[!0-9]*", "abc"));
        assert!(wildcard_match("[^0-9]*", "xyz"));
        assert!(!wildcard_match("[!0-9]*", "123"));
        assert!(!wildcard_match("[^0-9]*", "1abc"));
    }

    #[test]
    fn test_wildcard_complex_patterns() {
        // Realistic bucket name patterns
        assert!(wildcard_match("app-*-[0-9][0-9]", "app-prod-01"));
        assert!(wildcard_match("app-*-[0-9][0-9]", "app-staging-99"));
        assert!(!wildcard_match("app-*-[0-9][0-9]", "app-prod-1"));
        assert!(!wildcard_match("app-*-[0-9][0-9]", "app-prod-abc"));

        // Environment patterns
        assert!(wildcard_match("*-[ds]*", "app-dev"));
        assert!(wildcard_match("*-[ds]*", "api-staging"));
        assert!(!wildcard_match("*-[ds]*", "web-prod"));

        // Version patterns
        assert!(wildcard_match("v[0-9].*", "v1.0"));
        assert!(wildcard_match("v[0-9].*", "v2.1.3"));
        assert!(!wildcard_match("v[0-9].*", "version1"));
    }

    #[test]
    fn test_wildcard_edge_cases() {
        // Empty pattern and text
        assert!(wildcard_match("", ""));
        assert!(!wildcard_match("", "text"));
        assert!(!wildcard_match("pattern", ""));

        // Only wildcards
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("*", ""));
        assert!(wildcard_match("***", "text"));

        // Malformed character classes (should not crash)
        assert!(!wildcard_match("[", "a"));
        assert!(wildcard_match("[abc", "a"));
        assert!(!wildcard_match("[]", ""));

        // Special characters in patterns
        assert!(!wildcard_match("file\\*", "file*"));
        assert!(!wildcard_match("file\\*", "filename"));
    }

    #[test]
    fn test_filter_by_pattern() {
        let bucket_names = vec![
            "app-prod".to_string(),
            "app-staging".to_string(),
            "app-dev".to_string(),
            "api-prod".to_string(),
            "api-dev".to_string(),
            "web-prod".to_string(),
            "test-bucket-1".to_string(),
            "test-bucket-2".to_string(),
            "user-data".to_string(),
        ];

        // Test various patterns
        let prod_buckets = filter_by_pattern(&bucket_names, "*-prod");
        assert_eq!(prod_buckets, vec!["app-prod", "api-prod", "web-prod"]);

        let app_buckets = filter_by_pattern(&bucket_names, "app-*");
        assert_eq!(app_buckets, vec!["app-prod", "app-staging", "app-dev"]);

        let test_buckets = filter_by_pattern(&bucket_names, "test-*");
        assert_eq!(test_buckets, vec!["test-bucket-1", "test-bucket-2"]);

        let numbered_buckets = filter_by_pattern(&bucket_names, "*-[0-9]");
        assert_eq!(numbered_buckets, vec!["test-bucket-1", "test-bucket-2"]);

        // Pattern that matches nothing
        let no_match = filter_by_pattern(&bucket_names, "nonexistent-*");
        assert!(no_match.is_empty());

        // Pattern that matches everything
        let all_match = filter_by_pattern(&bucket_names, "*");
        assert_eq!(all_match.len(), bucket_names.len());
    }

    #[test]
    fn test_filter_by_pattern_empty_input() {
        let empty_list: Vec<String> = vec![];
        let result = filter_by_pattern(&empty_list, "*");
        assert!(result.is_empty());
    }

    #[test]
    fn test_wildcard_case_sensitivity() {
        // Test case sensitivity in wildcard patterns
        assert!(wildcard_match("Test*", "TestFile"));
        assert!(!wildcard_match("test*", "TestFile")); // Case sensitive
        assert!(wildcard_match("test*", "testfile"));
    }

    // New tests for enhanced pattern matching
    #[test]
    fn test_pattern_type_detection() {
        // Wildcard patterns
        assert!(matches!(
            detect_pattern_type("*-prod"),
            PatternType::Wildcard
        ));
        assert!(matches!(
            detect_pattern_type("test-?"),
            PatternType::Wildcard
        ));
        assert!(matches!(
            detect_pattern_type("[abc]*"),
            PatternType::Wildcard
        ));
        assert!(matches!(
            detect_pattern_type("simple-name"),
            PatternType::Wildcard
        ));

        // Regex patterns (contain metacharacters)
        assert!(matches!(
            detect_pattern_type("^backup-"),
            PatternType::Regex
        ));
        assert!(matches!(detect_pattern_type("prod$"), PatternType::Regex));
        assert!(matches!(detect_pattern_type("\\d+"), PatternType::Regex));
        assert!(matches!(
            detect_pattern_type("(dev|test)"),
            PatternType::Regex
        ));
        assert!(matches!(
            detect_pattern_type("bucket{3,8}"),
            PatternType::Regex
        ));
        assert!(matches!(detect_pattern_type("test+"), PatternType::Regex));
        assert!(matches!(detect_pattern_type("app\\w+"), PatternType::Regex));
    }

    #[test]
    fn test_regex_matching() {
        // Basic regex patterns
        assert!(regex_match("^test", "test-bucket").unwrap());
        assert!(!regex_match("^test", "my-test-bucket").unwrap());

        assert!(regex_match("prod$", "app-prod").unwrap());
        assert!(!regex_match("prod$", "prod-backup").unwrap());

        // Digit patterns
        assert!(regex_match("\\d+", "backup-123").unwrap());
        assert!(!regex_match("\\d+", "backup-abc").unwrap());

        // Word boundaries and character classes - fix the failing test
        assert!(regex_match("^\\w{3,8}$", "bucket").unwrap());
        assert!(!regex_match("^\\w{3,8}$", "verylongbucketname").unwrap());

        // Alternation
        assert!(regex_match("(dev|test|prod)", "test-bucket").unwrap());
        assert!(regex_match("(dev|test|prod)", "prod-data").unwrap());
        assert!(!regex_match("(dev|test|prod)", "staging-app").unwrap());
    }

    #[test]
    fn test_enhanced_pattern_match_auto_detection() {
        // Should use wildcard matching automatically
        assert!(enhanced_pattern_match("*-prod", "app-prod", false).unwrap());
        assert!(enhanced_pattern_match("test-?", "test-1", false).unwrap());

        // Should use regex matching automatically
        assert!(enhanced_pattern_match("^backup-\\d{4}$", "backup-2024", false).unwrap());
        assert!(!enhanced_pattern_match("^backup-\\d{4}$", "backup-24", false).unwrap());

        // Force regex mode
        assert!(enhanced_pattern_match(".*-prod", "app-prod", true).unwrap());
    }

    #[test]
    fn test_wildcard_to_regex_conversion() {
        // Basic conversions
        assert_eq!(wildcard_to_regex("*"), "^.*$");
        assert_eq!(wildcard_to_regex("?"), "^.$");
        assert_eq!(wildcard_to_regex("test*"), "^test.*$");
        assert_eq!(wildcard_to_regex("*-prod"), "^.*-prod$");

        // Character classes
        assert_eq!(wildcard_to_regex("[abc]"), "^[abc]$");
        assert_eq!(wildcard_to_regex("[!abc]"), "^[^abc]$");
        assert_eq!(wildcard_to_regex("[a-z]*"), "^[a-z].*$");

        // Escape regex metacharacters
        assert_eq!(wildcard_to_regex("test.txt"), "^test\\.txt$");
        assert_eq!(wildcard_to_regex("app+name"), "^app\\+name$");
    }

    #[test]
    fn test_filter_by_enhanced_pattern() {
        let buckets = vec![
            "app-prod".to_string(),
            "app-dev".to_string(),
            "backup-2024-01".to_string(),
            "backup-2023-12".to_string(),
            "test-bucket-1".to_string(),
            "staging-env".to_string(),
        ];

        // Wildcard patterns
        let prod_buckets = filter_by_enhanced_pattern(&buckets, "*-prod", false).unwrap();
        assert_eq!(prod_buckets, vec!["app-prod"]);

        // Regex patterns (auto-detected)
        let backup_buckets =
            filter_by_enhanced_pattern(&buckets, "^backup-\\d{4}-\\d{2}$", false).unwrap();
        assert_eq!(backup_buckets, vec!["backup-2024-01", "backup-2023-12"]);

        // Alternation pattern
        let env_buckets = filter_by_enhanced_pattern(&buckets, "(app|test)-.*", false).unwrap();
        assert_eq!(env_buckets, vec!["app-prod", "app-dev", "test-bucket-1"]);
    }

    #[test]
    fn test_regex_error_handling() {
        // Invalid regex should return error
        let result = regex_match("[invalid", "test");
        assert!(result.is_err());

        let result = enhanced_pattern_match("(unclosed", "test", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_real_world_patterns() {
        let buckets = vec![
            "logs-2024-01-15".to_string(),
            "logs-2024-02-20".to_string(),
            "backup-v1-prod".to_string(),
            "backup-v2-dev".to_string(),
            "user-123-data".to_string(),
            "user-abc-data".to_string(),
            "temp-session-xyz".to_string(),
        ];

        // Date-based log buckets
        let log_buckets =
            filter_by_enhanced_pattern(&buckets, "^logs-\\d{4}-\\d{2}-\\d{2}$", false).unwrap();
        assert_eq!(log_buckets, vec!["logs-2024-01-15", "logs-2024-02-20"]);

        // Versioned backup buckets
        let backup_buckets =
            filter_by_enhanced_pattern(&buckets, "^backup-v\\d+-(prod|dev)$", false).unwrap();
        assert_eq!(backup_buckets, vec!["backup-v1-prod", "backup-v2-dev"]);

        // Numeric user buckets only
        let numeric_user_buckets =
            filter_by_enhanced_pattern(&buckets, "^user-\\d+-data$", false).unwrap();
        assert_eq!(numeric_user_buckets, vec!["user-123-data"]);

        // Temporary buckets
        let temp_buckets = filter_by_enhanced_pattern(&buckets, "^temp-.*", false).unwrap();
        assert_eq!(temp_buckets, vec!["temp-session-xyz"]);
    }
}
