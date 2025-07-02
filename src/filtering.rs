use anyhow::{anyhow, Result};
#[allow(unused_imports)] // Used in tests for .year(), .month(), .day() methods
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use std::cmp::Ordering;

/// Enhanced object information for filtering operations
#[derive(Debug, Clone)]
pub struct EnhancedObjectInfo {
    pub key: String,
    pub size: i64,
    pub created: Option<DateTime<Utc>>,
    pub modified: Option<DateTime<Utc>>,
    pub storage_class: Option<String>,
    pub etag: Option<String>,
}

/// Filter configuration for advanced filtering operations
#[derive(Debug, Clone, Default)]
pub struct FilterConfig {
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub modified_after: Option<DateTime<Utc>>,
    pub modified_before: Option<DateTime<Utc>>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub max_results: Option<usize>,
    pub head: Option<usize>,
    pub tail: Option<usize>,
    pub sort_config: SortConfig,
}

/// Multi-level sorting configuration
#[derive(Debug, Clone, Default)]
pub struct SortConfig {
    pub fields: Vec<SortField>,
}

/// Individual sort field with type and direction
#[derive(Debug, Clone)]
pub struct SortField {
    pub field_type: SortFieldType,
    pub direction: SortDirection,
}

/// Types of fields that can be sorted
#[derive(Debug, Clone, PartialEq)]
pub enum SortFieldType {
    Name,
    Size,
    Created,
    Modified,
}

/// Sort direction (ascending or descending)
#[derive(Debug, Clone, PartialEq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Date parsing errors
#[derive(Debug, thiserror::Error)]
pub enum DateParseError {
    #[error(
        "Invalid date format: {0}. Expected YYYYMMDD or relative format like '7d', '30d', '1y'"
    )]
    InvalidFormat(String),
    #[error("Invalid date value: {0}")]
    InvalidDate(String),
    #[error("Invalid relative date: {0}")]
    InvalidRelativeDate(String),
}

/// Size parsing errors
#[derive(Debug, thiserror::Error)]
pub enum SizeParseError {
    #[error("Invalid size format: {0}. Expected number with optional unit (B, KB, MB, GB, TB)")]
    InvalidFormat(String),
    #[error("Invalid size value: {0}")]
    InvalidValue(String),
    #[error("Unsupported size unit: {0}")]
    UnsupportedUnit(String),
}

/// Parse date filter input (YYYYMMDD or relative format)
pub fn parse_date_filter(input: &str) -> Result<DateTime<Utc>, DateParseError> {
    match input {
        // YYYYMMDD format (20240101)
        s if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) => parse_yyyymmdd(s),
        // Relative format (7d, 30d, 1y)
        s if s.ends_with('d') || s.ends_with('w') || s.ends_with('m') || s.ends_with('y') => {
            parse_relative_date(s)
        }
        _ => Err(DateParseError::InvalidFormat(input.to_string())),
    }
}

/// Parse YYYYMMDD format date
fn parse_yyyymmdd(input: &str) -> Result<DateTime<Utc>, DateParseError> {
    if input.len() != 8 {
        return Err(DateParseError::InvalidFormat(input.to_string()));
    }

    let year: i32 = input[0..4]
        .parse()
        .map_err(|_| DateParseError::InvalidDate(input.to_string()))?;
    let month: u32 = input[4..6]
        .parse()
        .map_err(|_| DateParseError::InvalidDate(input.to_string()))?;
    let day: u32 = input[6..8]
        .parse()
        .map_err(|_| DateParseError::InvalidDate(input.to_string()))?;

    // Validate ranges
    if !(1..=12).contains(&month) {
        return Err(DateParseError::InvalidDate(format!(
            "Invalid month: {month}"
        )));
    }
    if !(1..=31).contains(&day) {
        return Err(DateParseError::InvalidDate(format!("Invalid day: {day}")));
    }

    chrono::Utc
        .with_ymd_and_hms(year, month, day, 0, 0, 0)
        .single()
        .ok_or_else(|| DateParseError::InvalidDate(input.to_string()))
}

/// Parse relative date format (7d, 30d, 1y)
fn parse_relative_date(input: &str) -> Result<DateTime<Utc>, DateParseError> {
    let (number_part, unit_part) = input.split_at(input.len() - 1);

    let number: i64 = number_part
        .parse()
        .map_err(|_| DateParseError::InvalidRelativeDate(input.to_string()))?;

    if number <= 0 {
        return Err(DateParseError::InvalidRelativeDate(format!(
            "Number must be positive: {number}"
        )));
    }

    let duration = match unit_part {
        "d" => Duration::days(number),
        "w" => Duration::weeks(number),
        "m" => Duration::days(number * 30),  // Approximate month
        "y" => Duration::days(number * 365), // Approximate year
        _ => return Err(DateParseError::InvalidRelativeDate(input.to_string())),
    };

    Ok(Utc::now() - duration)
}

/// Parse size filter input (with MB default)
pub fn parse_size_filter(input: &str) -> Result<i64, SizeParseError> {
    let input = input.trim();

    // Check if it's just a number (default to MB)
    if let Ok(number) = input.parse::<i64>() {
        return Ok(number * 1_048_576); // Convert MB to bytes
    }

    // Parse number with unit
    let (number_str, unit) = extract_number_and_unit(input)?;
    let number: f64 = number_str
        .parse()
        .map_err(|_| SizeParseError::InvalidValue(input.to_string()))?;

    if number < 0.0 {
        return Err(SizeParseError::InvalidValue(
            "Size cannot be negative".to_string(),
        ));
    }

    let multiplier = match unit.to_uppercase().as_str() {
        "B" => 1,
        "KB" => 1_000,
        "MB" => 1_000_000,
        "GB" => 1_000_000_000,
        "TB" => 1_000_000_000_000_i64,
        "PB" => 1_000_000_000_000_000_i64,
        "KIB" => 1_024,
        "MIB" => 1_048_576,
        "GIB" => 1_073_741_824,
        "TIB" => 1_099_511_627_776_i64,
        "PIB" => 1_125_899_906_842_624_i64,
        _ => return Err(SizeParseError::UnsupportedUnit(unit.to_string())),
    };

    Ok((number * multiplier as f64) as i64)
}

/// Extract number and unit from size string
fn extract_number_and_unit(input: &str) -> Result<(String, String), SizeParseError> {
    let mut number_end = 0;
    for (i, c) in input.char_indices() {
        if c.is_ascii_digit() || c == '.' {
            number_end = i + 1;
        } else {
            break;
        }
    }

    if number_end == 0 {
        return Err(SizeParseError::InvalidFormat(input.to_string()));
    }

    let number_part = input[..number_end].to_string();
    let unit_part = input[number_end..].trim().to_string();

    if unit_part.is_empty() {
        return Err(SizeParseError::InvalidFormat(input.to_string()));
    }

    Ok((number_part, unit_part))
}

/// Parse sort specification (e.g., "modified:desc,size:asc")
pub fn parse_sort_config(input: &str) -> Result<SortConfig> {
    let mut fields = Vec::new();

    for field_spec in input.split(',') {
        let field_spec = field_spec.trim();
        let (field_name, direction) = if field_spec.contains(':') {
            let parts: Vec<&str> = field_spec.split(':').collect();
            if parts.len() != 2 {
                return Err(anyhow!("Invalid sort specification: {}", field_spec));
            }
            (parts[0], parts[1])
        } else {
            (field_spec, "asc") // Default to ascending
        };

        let field_type = match field_name.to_lowercase().as_str() {
            "name" => SortFieldType::Name,
            "size" => SortFieldType::Size,
            "created" => SortFieldType::Created,
            "modified" => SortFieldType::Modified,
            _ => return Err(anyhow!("Invalid sort field: {}", field_name)),
        };

        let direction = match direction.to_lowercase().as_str() {
            "asc" | "ascending" => SortDirection::Ascending,
            "desc" | "descending" => SortDirection::Descending,
            _ => return Err(anyhow!("Invalid sort direction: {}", direction)),
        };

        fields.push(SortField {
            field_type,
            direction,
        });
    }

    Ok(SortConfig { fields })
}

/// Apply filters to a list of objects with performance optimizations
pub fn apply_filters(
    objects: &[EnhancedObjectInfo],
    config: &FilterConfig,
) -> Vec<EnhancedObjectInfo> {
    // Performance optimization: Early termination for head operations
    if let Some(head) = config.head {
        return apply_filters_with_head_optimization(objects, config, head);
    }

    let mut filtered: Vec<EnhancedObjectInfo> = objects
        .iter()
        .filter(|obj| passes_filters(obj, config))
        .cloned()
        .collect();

    // Apply sorting
    if !config.sort_config.fields.is_empty() {
        filtered.sort_by(|a, b| compare_objects(a, b, &config.sort_config));
    }

    // Apply result limiting
    if let Some(max_results) = config.max_results {
        filtered.truncate(max_results);
    }

    if let Some(tail) = config.tail {
        // For tail operations, ensure we have proper sorting by modified date
        if config.sort_config.fields.is_empty() {
            // Auto-sort by modified date for tail operations
            filtered.sort_by(|a, b| {
                match (a.modified, b.modified) {
                    (Some(a_mod), Some(b_mod)) => a_mod.cmp(&b_mod),
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
        let start = filtered.len().saturating_sub(tail);
        filtered = filtered[start..].to_vec();
    }

    filtered
}

/// Performance-optimized filtering with early termination for head operations
fn apply_filters_with_head_optimization(
    objects: &[EnhancedObjectInfo],
    config: &FilterConfig,
    head_limit: usize,
) -> Vec<EnhancedObjectInfo> {
    let mut filtered = Vec::with_capacity(head_limit.min(objects.len()));
    let mut processed_count = 0;

    // For head operations, we can potentially stop early if we have enough results
    // and no sorting is required
    let can_early_terminate = config.sort_config.fields.is_empty() && config.max_results.is_none();

    for obj in objects {
        if passes_filters(obj, config) {
            filtered.push(obj.clone());

            // Early termination: if we have enough for head and no sorting needed
            if can_early_terminate && filtered.len() >= head_limit {
                break;
            }
        }

        processed_count += 1;

        // Safety check: don't process more than necessary for memory efficiency
        if let Some(max_results) = config.max_results {
            if processed_count >= max_results * 2 {
                break;
            }
        }
    }

    // Apply sorting if needed
    if !config.sort_config.fields.is_empty() {
        filtered.sort_by(|a, b| compare_objects(a, b, &config.sort_config));
    }

    // Apply result limiting
    if let Some(max_results) = config.max_results {
        filtered.truncate(max_results);
    }

    // Apply head limiting
    filtered.truncate(head_limit);

    filtered
}

/// Memory-efficient streaming filter for large object collections
pub fn apply_filters_streaming<I>(
    objects: I,
    config: &FilterConfig,
    estimated_size: Option<usize>,
) -> Vec<EnhancedObjectInfo>
where
    I: Iterator<Item = EnhancedObjectInfo>,
{
    // Estimate capacity for better memory allocation
    let capacity = match (estimated_size, config.head, config.max_results) {
        (Some(size), Some(head), _) => head.min(size),
        (Some(size), _, Some(max)) => max.min(size),
        (Some(size), _, _) => size.min(10000), // Cap at 10K for memory efficiency
        (None, Some(head), _) => head,
        (None, _, Some(max)) => max,
        (None, _, _) => 1000, // Default reasonable size
    };

    let mut filtered = Vec::with_capacity(capacity);
    let mut processed_count = 0;

    // Performance optimization flags
    let has_head = config.head.is_some();
    let head_limit = config.head.unwrap_or(usize::MAX);
    let can_early_terminate = has_head && config.sort_config.fields.is_empty() && config.max_results.is_none();

    for obj in objects {
        if passes_filters(&obj, config) {
            filtered.push(obj);

            // Early termination for head operations without sorting
            if can_early_terminate && filtered.len() >= head_limit {
                break;
            }
        }

        processed_count += 1;

        // Memory safety: prevent excessive memory usage
        if processed_count % 10000 == 0 {
            // Periodic memory check for very large datasets
            if let Some(max_results) = config.max_results {
                if filtered.len() >= max_results && !has_head {
                    break;
                }
            }
        }
    }

    // Apply sorting
    if !config.sort_config.fields.is_empty() {
        filtered.sort_by(|a, b| compare_objects(a, b, &config.sort_config));
    }

    // Apply result limiting
    if let Some(max_results) = config.max_results {
        filtered.truncate(max_results);
    }

    // Apply head/tail operations
    if let Some(head) = config.head {
        filtered.truncate(head);
    } else if let Some(tail) = config.tail {
        // For tail operations, ensure proper sorting
        if config.sort_config.fields.is_empty() {
            filtered.sort_by(|a, b| {
                match (a.modified, b.modified) {
                    (Some(a_mod), Some(b_mod)) => a_mod.cmp(&b_mod),
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
        let start = filtered.len().saturating_sub(tail);
        filtered = filtered[start..].to_vec();
    }

    filtered
}

/// Check if an object passes all filters
fn passes_filters(obj: &EnhancedObjectInfo, config: &FilterConfig) -> bool {
    // Date filters
    if let Some(created_after) = config.created_after {
        if let Some(created) = obj.created {
            if created < created_after {
                return false;
            }
        } else {
            return false; // No creation date, can't filter
        }
    }

    if let Some(created_before) = config.created_before {
        if let Some(created) = obj.created {
            if created > created_before {
                return false;
            }
        } else {
            return false;
        }
    }

    if let Some(modified_after) = config.modified_after {
        if let Some(modified) = obj.modified {
            if modified < modified_after {
                return false;
            }
        } else {
            return false;
        }
    }

    if let Some(modified_before) = config.modified_before {
        if let Some(modified) = obj.modified {
            if modified > modified_before {
                return false;
            }
        } else {
            return false;
        }
    }

    // Size filters
    if let Some(min_size) = config.min_size {
        if obj.size < min_size {
            return false;
        }
    }

    if let Some(max_size) = config.max_size {
        if obj.size > max_size {
            return false;
        }
    }

    true
}

/// Compare two objects for sorting
fn compare_objects(
    a: &EnhancedObjectInfo,
    b: &EnhancedObjectInfo,
    sort_config: &SortConfig,
) -> Ordering {
    for field in &sort_config.fields {
        let ordering = match field.field_type {
            SortFieldType::Name => a.key.cmp(&b.key),
            SortFieldType::Size => a.size.cmp(&b.size),
            SortFieldType::Created => match (a.created, b.created) {
                (Some(a_created), Some(b_created)) => a_created.cmp(&b_created),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            },
            SortFieldType::Modified => match (a.modified, b.modified) {
                (Some(a_modified), Some(b_modified)) => a_modified.cmp(&b_modified),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            },
        };

        let final_ordering = match field.direction {
            SortDirection::Ascending => ordering,
            SortDirection::Descending => ordering.reverse(),
        };

        if final_ordering != Ordering::Equal {
            return final_ordering;
        }
    }

    Ordering::Equal
}

/// Validate filter configuration for conflicts
pub fn validate_filter_config(config: &FilterConfig) -> Result<()> {
    // Check date range validity
    if let (Some(after), Some(before)) = (config.created_after, config.created_before) {
        if after >= before {
            return Err(anyhow!("created_after must be before created_before"));
        }
    }

    if let (Some(after), Some(before)) = (config.modified_after, config.modified_before) {
        if after >= before {
            return Err(anyhow!("modified_after must be before modified_before"));
        }
    }

    // Check size range validity
    if let (Some(min), Some(max)) = (config.min_size, config.max_size) {
        if min >= max {
            return Err(anyhow!("min_size must be less than max_size"));
        }
    }

    // Check head/tail conflicts
    if config.head.is_some() && config.tail.is_some() {
        return Err(anyhow!("Cannot use both --head and --tail options"));
    }

    // Check head/tail vs max_results
    if let (Some(head), Some(max_results)) = (config.head, config.max_results) {
        if head > max_results {
            return Err(anyhow!("--head value cannot exceed --max-results"));
        }
    }

    if let (Some(tail), Some(max_results)) = (config.tail, config.max_results) {
        if tail > max_results {
            return Err(anyhow!("--tail value cannot exceed --max-results"));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yyyymmdd() {
        let result = parse_date_filter("20240101").unwrap();
        assert_eq!(result.year(), 2024);
        assert_eq!(result.month(), 1);
        assert_eq!(result.day(), 1);
    }

    #[test]
    fn test_parse_yyyymmdd_invalid() {
        assert!(parse_date_filter("20241301").is_err()); // Invalid month
        assert!(parse_date_filter("20240132").is_err()); // Invalid day
        assert!(parse_date_filter("2024010").is_err()); // Wrong length
    }

    #[test]
    fn test_parse_relative_date() {
        let result = parse_date_filter("7d").unwrap();
        let expected = Utc::now() - Duration::days(7);
        assert!((result - expected).num_seconds().abs() < 60); // Within 1 minute

        let result = parse_date_filter("2w").unwrap();
        let expected = Utc::now() - Duration::weeks(2);
        assert!((result - expected).num_seconds().abs() < 60);
    }

    #[test]
    fn test_parse_size_filter() {
        assert_eq!(parse_size_filter("100").unwrap(), 100 * 1_048_576); // Default MB
        assert_eq!(parse_size_filter("1GB").unwrap(), 1_000_000_000);
        assert_eq!(parse_size_filter("1GiB").unwrap(), 1_073_741_824);
        assert_eq!(parse_size_filter("500KB").unwrap(), 500_000);
        assert_eq!(parse_size_filter("1024B").unwrap(), 1024);
    }

    #[test]
    fn test_parse_size_filter_invalid() {
        assert!(parse_size_filter("-100MB").is_err()); // Negative
        assert!(parse_size_filter("100XB").is_err()); // Invalid unit
        assert!(parse_size_filter("abc").is_err()); // Invalid format
    }

    #[test]
    fn test_parse_sort_config() {
        let config = parse_sort_config("modified:desc,size:asc").unwrap();
        assert_eq!(config.fields.len(), 2);
        assert_eq!(config.fields[0].field_type, SortFieldType::Modified);
        assert_eq!(config.fields[0].direction, SortDirection::Descending);
        assert_eq!(config.fields[1].field_type, SortFieldType::Size);
        assert_eq!(config.fields[1].direction, SortDirection::Ascending);
    }

    #[test]
    fn test_parse_sort_config_default_direction() {
        let config = parse_sort_config("name,size").unwrap();
        assert_eq!(config.fields.len(), 2);
        assert_eq!(config.fields[0].direction, SortDirection::Ascending);
        assert_eq!(config.fields[1].direction, SortDirection::Ascending);
    }

    #[test]
    fn test_validate_filter_config() {
        let mut config = FilterConfig::default();
        assert!(validate_filter_config(&config).is_ok());

        // Test invalid date range
        config.created_after = Some(Utc::now());
        config.created_before = Some(Utc::now() - Duration::days(1));
        assert!(validate_filter_config(&config).is_err());

        // Test invalid size range
        config = FilterConfig::default();
        config.min_size = Some(100);
        config.max_size = Some(50);
        assert!(validate_filter_config(&config).is_err());

        // Test head/tail conflict
        config = FilterConfig::default();
        config.head = Some(10);
        config.tail = Some(20);
        assert!(validate_filter_config(&config).is_err());
    }

    #[test]
    fn test_apply_filters_date() {
        let now = Utc::now();
        let old_date = now - Duration::days(10);
        let recent_date = now - Duration::days(2);

        let objects = vec![
            EnhancedObjectInfo {
                key: "old_file.txt".to_string(),
                size: 1000,
                created: Some(old_date),
                modified: Some(old_date),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "recent_file.txt".to_string(),
                size: 2000,
                created: Some(recent_date),
                modified: Some(recent_date),
                storage_class: None,
                etag: None,
            },
        ];

        let config = FilterConfig {
            modified_after: Some(now - Duration::days(5)),
            ..Default::default()
        };

        let filtered = apply_filters(&objects, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "recent_file.txt");
    }

    #[test]
    fn test_apply_filters_size() {
        let objects = vec![
            EnhancedObjectInfo {
                key: "small_file.txt".to_string(),
                size: 500,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "large_file.txt".to_string(),
                size: 5000,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
        ];

        let config = FilterConfig {
            min_size: Some(1000),
            max_size: Some(10000),
            ..Default::default()
        };

        let filtered = apply_filters(&objects, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "large_file.txt");
    }

    #[test]
    fn test_apply_filters_sorting() {
        let objects = vec![
            EnhancedObjectInfo {
                key: "c_file.txt".to_string(),
                size: 3000,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "a_file.txt".to_string(),
                size: 1000,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "b_file.txt".to_string(),
                size: 2000,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
        ];

        let config = FilterConfig {
            sort_config: parse_sort_config("size:desc").unwrap(),
            ..Default::default()
        };

        let filtered = apply_filters(&objects, &config);
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].key, "c_file.txt"); // Largest first
        assert_eq!(filtered[1].key, "b_file.txt");
        assert_eq!(filtered[2].key, "a_file.txt"); // Smallest last
    }

    #[test]
    fn test_apply_filters_head_tail() {
        let objects = vec![
            EnhancedObjectInfo {
                key: "file1.txt".to_string(),
                size: 1000,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "file2.txt".to_string(),
                size: 2000,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "file3.txt".to_string(),
                size: 3000,
                created: None,
                modified: None,
                storage_class: None,
                etag: None,
            },
        ];

        // Test head
        let config = FilterConfig {
            head: Some(2),
            ..Default::default()
        };
        let filtered = apply_filters(&objects, &config);
        assert_eq!(filtered.len(), 2);

        // Test tail
        let config = FilterConfig {
            tail: Some(2),
            ..Default::default()
        };
        let filtered = apply_filters(&objects, &config);
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].key, "file2.txt");
        assert_eq!(filtered[1].key, "file3.txt");
    }

    #[test]
    fn test_multi_level_sorting() {
        let config = FilterConfig {
            sort_config: parse_sort_config("modified:desc,size:asc").unwrap(),
            ..Default::default()
        };

        // Create test objects with same modified date but different sizes
        let now = Utc::now();
        let objects = vec![
            EnhancedObjectInfo {
                key: "large.txt".to_string(),
                size: 1000,
                created: Some(now),
                modified: Some(now),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "small.txt".to_string(),
                size: 100,
                created: Some(now),
                modified: Some(now),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "medium.txt".to_string(),
                size: 500,
                created: Some(now),
                modified: Some(now),
                storage_class: None,
                etag: None,
            },
        ];

        let filtered = apply_filters(&objects, &config);

        // Should be sorted by size (asc) since modified dates are the same
        assert_eq!(filtered[0].key, "small.txt");
        assert_eq!(filtered[1].key, "medium.txt");
        assert_eq!(filtered[2].key, "large.txt");
    }

    #[test]
    fn test_head_optimization_early_termination() {
        let config = FilterConfig {
            head: Some(2),
            ..Default::default()
        };
        // No sorting specified - should enable early termination

        let objects: Vec<EnhancedObjectInfo> = (0..1000)
            .map(|i| EnhancedObjectInfo {
                key: format!("file{i}.txt"),
                size: i as i64,
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
                storage_class: None,
                etag: None,
            })
            .collect();

        let filtered = apply_filters(&objects, &config);

        // Should return exactly 2 items (head limit)
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].key, "file0.txt");
        assert_eq!(filtered[1].key, "file1.txt");
    }

    #[test]
    fn test_head_optimization_with_sorting() {
        let config = FilterConfig {
            head: Some(3),
            sort_config: parse_sort_config("size:desc").unwrap(),
            ..Default::default()
        };

        let objects = vec![
            EnhancedObjectInfo {
                key: "small.txt".to_string(),
                size: 100,
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "large.txt".to_string(),
                size: 1000,
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "medium.txt".to_string(),
                size: 500,
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "tiny.txt".to_string(),
                size: 50,
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
                storage_class: None,
                etag: None,
            },
        ];

        let filtered = apply_filters(&objects, &config);

        // Should return 3 items sorted by size desc
        assert_eq!(filtered.len(), 3);
        assert_eq!(filtered[0].key, "large.txt");
        assert_eq!(filtered[1].key, "medium.txt");
        assert_eq!(filtered[2].key, "small.txt");
    }

    #[test]
    fn test_tail_auto_sorting() {
        let config = FilterConfig {
            tail: Some(2),
            ..Default::default()
        };
        // No sorting specified - should auto-sort by modified date

        let now = Utc::now();
        let objects = vec![
            EnhancedObjectInfo {
                key: "old.txt".to_string(),
                size: 100,
                created: Some(now),
                modified: Some(now - Duration::hours(2)),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "recent.txt".to_string(),
                size: 200,
                created: Some(now),
                modified: Some(now - Duration::minutes(30)),
                storage_class: None,
                etag: None,
            },
            EnhancedObjectInfo {
                key: "newest.txt".to_string(),
                size: 300,
                created: Some(now),
                modified: Some(now),
                storage_class: None,
                etag: None,
            },
        ];

        let filtered = apply_filters(&objects, &config);

        // Should return last 2 by modification date
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].key, "recent.txt");
        assert_eq!(filtered[1].key, "newest.txt");
    }

    #[test]
    fn test_streaming_filter_performance() {
        let config = FilterConfig {
            min_size: Some(500),
            head: Some(5),
            ..Default::default()
        };

        // Create iterator of test objects
        let objects_iter = (0..10000).map(|i| EnhancedObjectInfo {
            key: format!("file{i}.txt"),
            size: i as i64,
            created: Some(Utc::now()),
            modified: Some(Utc::now()),
            storage_class: None,
            etag: None,
        });

        let filtered = apply_filters_streaming(objects_iter, &config, Some(10000));

        // Should return exactly 5 items (head limit) with size >= 500
        assert_eq!(filtered.len(), 5);
        assert!(filtered.iter().all(|obj| obj.size >= 500));

        // Should be the first 5 items that match the filter
        assert_eq!(filtered[0].key, "file500.txt");
        assert_eq!(filtered[1].key, "file501.txt");
        assert_eq!(filtered[2].key, "file502.txt");
        assert_eq!(filtered[3].key, "file503.txt");
        assert_eq!(filtered[4].key, "file504.txt");
    }

    #[test]
    fn test_memory_efficient_capacity_estimation() {
        // Test different capacity estimation scenarios
        let config_head = FilterConfig {
            head: Some(100),
            ..Default::default()
        };

        let config_max = FilterConfig {
            max_results: Some(500),
            ..Default::default()
        };

        let config_both = FilterConfig {
            head: Some(50),
            max_results: Some(200),
            ..Default::default()
        };

        let objects_iter = std::iter::empty();

        // Test capacity estimation logic
        let result1 = apply_filters_streaming(objects_iter.clone(), &config_head, Some(1000));
        let result2 = apply_filters_streaming(objects_iter.clone(), &config_max, Some(1000));
        let result3 = apply_filters_streaming(objects_iter, &config_both, Some(1000));

        // All should handle empty iterator gracefully
        assert_eq!(result1.len(), 0);
        assert_eq!(result2.len(), 0);
        assert_eq!(result3.len(), 0);
    }

    #[test]
    fn test_performance_with_large_dataset() {
        use std::time::Instant;

        let config = FilterConfig {
            min_size: Some(5000),
            head: Some(10),
            ..Default::default()
        };

        // Create a large dataset
        let objects: Vec<EnhancedObjectInfo> = (0..50000)
            .map(|i| EnhancedObjectInfo {
                key: format!("file{i}.txt"),
                size: i as i64,
                created: Some(Utc::now()),
                modified: Some(Utc::now()),
                storage_class: None,
                etag: None,
            })
            .collect();

        let start = Instant::now();
        let filtered = apply_filters(&objects, &config);
        let duration = start.elapsed();

        // Should complete quickly with early termination
        assert!(duration.as_millis() < 100); // Should be very fast
        assert_eq!(filtered.len(), 10);
        assert!(filtered.iter().all(|obj| obj.size >= 5000));
    }
}
