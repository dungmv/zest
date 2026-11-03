/// Canonical application version resolved at compile time from GitHub Actions build / git tag.
pub const APP_VERSION: &str = env!("ZEST_APP_VERSION");

/// Short git commit hash resolved at compile time.
pub const COMMIT_HASH: &str = env!("ZEST_COMMIT_HASH");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_constants_present() {
        assert!(!APP_VERSION.is_empty(), "APP_VERSION must not be empty");
        assert!(!COMMIT_HASH.is_empty(), "COMMIT_HASH must not be empty");
        assert!(
            !APP_VERSION.starts_with('v') && !APP_VERSION.starts_with('V'),
            "APP_VERSION should be stripped of leading 'v'"
        );
    }
}
