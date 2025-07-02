# ADR-0002: Intelligent Pattern Matching Engine

## Status
**Accepted** - Implemented (July 2025)

## Context
obsctl needed advanced pattern matching for bucket operations beyond basic wildcards. Users wanted both simple wildcard patterns (`*-prod`) and complex regex patterns (`^backup-\d{4}$`) without having to specify which type they're using.

## Decision
Implement an **intelligent auto-detection pattern matching engine** that automatically determines whether a pattern is a wildcard or regex based on metacharacter analysis.

### Detection Algorithm
```rust
pub fn detect_pattern_type(pattern: &str) -> PatternType {
    // Regex metacharacters: (){}+^$\|
    if pattern.chars().any(|c| matches!(c, '(' | ')' | '{' | '}' | '+' | '^' | '$' | '\\' | '|')) {
        PatternType::Regex
    } else {
        PatternType::Wildcard
    }
}
```

### Auto-Detection Examples
- `*-prod` → Wildcard (simple asterisk)
- `user-?-bucket` → Wildcard (simple question mark)
- `^backup-\d{4}$` → Regex (contains `^`, `\`, `$`)
- `logs-20(23|24)` → Regex (contains parentheses)

## Consequences

### Positive
- ✅ **Zero learning curve** - Users don't need to specify pattern type
- ✅ **Rubular.com compatibility** - Full regex support for power users
- ✅ **Backward compatible** - All existing wildcard patterns work unchanged
- ✅ **Intelligent behavior** - System chooses optimal matching algorithm

### Negative
- ⚠️ **Edge cases** - Rare patterns might be misclassified
- ⚠️ **Performance** - Regex patterns are slower than wildcards

## Implementation
- **Location**: `src/utils.rs` - `enhanced_pattern_match()` function
- **Testing**: 22 comprehensive tests covering detection and matching
- **Integration**: Used in `ls` and `rb` commands for bucket filtering

## Related ADRs
- ADR-0001: Advanced Filtering System (combines with pattern matching)

## References
- Implementation: `src/utils.rs` - pattern matching functions
- Documentation: Links to Rubular.com for regex testing 