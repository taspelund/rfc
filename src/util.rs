/// Truncate `s` to `max_width` characters, replacing the tail with `...`
/// when the string would be longer. Counts unicode scalar values, not bytes.
/// Pass `usize::MAX` to disable truncation entirely.
pub(crate) fn truncate_str(s: &str, max_width: usize) -> String {
    if max_width == usize::MAX || s.chars().count() <= max_width {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_width.saturating_sub(3)).collect();
        format!("{}...", truncated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_short_string() {
        assert_eq!(truncate_str("hello", 80), "hello");
    }

    #[test]
    fn truncate_long_string() {
        assert_eq!(truncate_str("hello world", 8), "hello...");
    }

    #[test]
    fn truncate_unlimited() {
        assert_eq!(truncate_str("hello world", usize::MAX), "hello world");
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate_str("", 10), "");
    }
}
