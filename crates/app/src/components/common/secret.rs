/// Masks a secret value for read-only display with a fixed-length placeholder so the
/// value length is not leaked. Values that reference an environment variable (contain
/// `{{`) are shown verbatim: they reference configuration rather than embed the secret.
pub fn mask_secret(value: &str) -> String {
    if value.trim().is_empty() || value.contains("{{") {
        value.to_string()
    } else {
        "••••••••".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret_empty_or_whitespace() {
        assert_eq!(mask_secret(""), "");
        assert_eq!(mask_secret("   "), "   ");
    }

    #[test]
    fn test_mask_secret_variable_syntax() {
        assert_eq!(mask_secret("{{my_token}}"), "{{my_token}}");
        assert_eq!(mask_secret("prefix_{{key}}"), "prefix_{{key}}");
    }

    #[test]
    fn test_mask_secret_raw_values() {
        assert_eq!(mask_secret("secret123"), "••••••••");
        assert_eq!(mask_secret("super-long-token-value-here"), "••••••••");
    }
}
