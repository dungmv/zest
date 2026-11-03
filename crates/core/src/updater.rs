use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckResult {
    UpToDate {
        current_version: String,
    },
    UpdateAvailable {
        current_version: String,
        latest_version: String,
        release_url: String,
        release_name: Option<String>,
    },
}

/// Parses a version string like "0.1.0", "v0.2.1", "1.0.0-rc.1" into (major, minor, patch).
pub fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches(['v', 'V']);
    let mut parts = clean.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch_str = parts
        .next()
        .unwrap_or("0")
        .split('-')
        .next()?
        .split('+')
        .next()?;
    let patch = patch_str.parse().ok()?;
    Some((major, minor, patch))
}

/// Returns true if `latest` is strictly newer than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    match (parse_semver(current), parse_semver(latest)) {
        (Some(c), Some(l)) => l > c,
        _ => false,
    }
}

/// Queries the GitHub Releases API for the latest release and compares against `current_version`.
pub async fn check_github_release(
    repo_owner: &str,
    repo_name: &str,
    current_version: &str,
) -> Result<UpdateCheckResult, String> {
    let url = format!("https://api.github.com/repos/{repo_owner}/{repo_name}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent(format!("zest-app/{}", current_version))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {e}"))?;

    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Network request failed: {e}"))?;

    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        // Repository has no published releases yet
        return Ok(UpdateCheckResult::UpToDate {
            current_version: current_version.to_string(),
        });
    }

    if !resp.status().is_success() {
        return Err(format!("GitHub API returned HTTP {}", resp.status()));
    }

    let release: GithubRelease = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse release response: {e}"))?;

    let latest_tag = release.tag_name.trim();
    if is_newer_version(current_version, latest_tag) {
        Ok(UpdateCheckResult::UpdateAvailable {
            current_version: current_version.to_string(),
            latest_version: latest_tag.to_string(),
            release_url: release.html_url,
            release_name: release.name,
        })
    } else {
        Ok(UpdateCheckResult::UpToDate {
            current_version: current_version.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver() {
        assert_eq!(parse_semver("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_semver("v0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_semver("V1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("v2.0.0-beta.1"), Some((2, 0, 0)));
        assert_eq!(parse_semver("v2.0.0+build123"), Some((2, 0, 0)));
        assert_eq!(parse_semver("invalid"), None);
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.1.0", "0.2.0"));
        assert!(is_newer_version("0.1.0", "v0.1.1"));
        assert!(is_newer_version("0.1.0", "1.0.0"));
        assert!(is_newer_version("v0.1.0", "v0.2.0"));

        assert!(!is_newer_version("0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("0.2.0", "0.1.0"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
    }

    #[test]
    fn test_deserialize_github_release() {
        let json_data = r#"{
            "tag_name": "v0.2.0",
            "name": "Zest 0.2.0 Release",
            "html_url": "https://github.com/dungmv/zest/releases/tag/v0.2.0",
            "body": "Bugfixes and performance updates",
            "published_at": "2026-11-03T12:00:00Z"
        }"#;

        let release: GithubRelease = serde_json::from_str(json_data).expect("Must parse");
        assert_eq!(release.tag_name, "v0.2.0");
        assert_eq!(release.name, Some("Zest 0.2.0 Release".to_string()));
        assert_eq!(
            release.html_url,
            "https://github.com/dungmv/zest/releases/tag/v0.2.0"
        );
    }
}
