use regex::Regex;
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use super::CloneWorkspaceError;

pub(super) fn clone_destination(
    source: &str,
    parent_folder: &Path,
) -> Result<PathBuf, CloneWorkspaceError> {
    let repository_path = match source.split_once("://") {
        Some((_, address)) => address.split_once('/'),
        None => source.rsplit_once(':'),
    }
    .map(|(_, path)| path)
    .unwrap_or_default()
    .trim_end_matches('/');
    let repository_path = repository_path
        .strip_suffix("/.git")
        .or_else(|| repository_path.strip_suffix(".git"))
        .unwrap_or(repository_path);
    let name = repository_path.rsplit('/').next().unwrap_or_default();
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(std::path::Component::Normal(_)))
        || components.next().is_some()
        || (cfg!(windows) && (name.ends_with(['.', ' ']) || invalid_windows_folder(name)))
    {
        return Err(CloneWorkspaceError::InvalidRepositoryName);
    }
    Ok(parent_folder.join(name))
}

fn invalid_windows_folder(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let stem = stem.to_ascii_uppercase();
    name.contains(['<', '>', ':', '"', '|', '*'])
        || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
}

pub(super) fn validate_source(source: &str) -> Result<(), CloneWorkspaceError> {
    static SCP_URL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^(?:[A-Za-z0-9_.][A-Za-z0-9_.-]*@)?(?:[A-Za-z0-9_][A-Za-z0-9_.-]*|\[[A-Fa-f0-9:]+\]):[^:]+$")
            .expect("valid Git SSH URL pattern")
    });
    if source.chars().any(|c| c.is_whitespace() || c.is_control())
        || source.contains(['\\', '?', '#'])
    {
        return Err(CloneWorkspaceError::InvalidUrl);
    }
    if let Some((scheme, remainder)) = source.split_once("://") {
        let authority = remainder.split('/').next().unwrap_or_default();
        let valid_authority = match scheme {
            "https" => !authority.contains('@'),
            "ssh" => authority.rsplit_once('@').is_none_or(|(user, _)| {
                !user.is_empty()
                    && user
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
                    && !user.starts_with('-')
            }),
            _ => false,
        };
        if valid_authority
            && !authority.is_empty()
            && !authority.starts_with('-')
            && url::Url::parse(source).is_ok_and(|url| {
                url.host_str().is_some_and(|host| !host.starts_with('-')) && url.path().len() > 1
            })
        {
            return Ok(());
        }
    } else if SCP_URL.is_match(source) {
        return Ok(());
    }
    Err(CloneWorkspaceError::InvalidUrl)
}
