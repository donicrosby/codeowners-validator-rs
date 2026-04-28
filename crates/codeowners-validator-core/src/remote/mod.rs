pub mod error;

pub use error::RemoteRepoError;

use secrecy::ExposeSecret;
use secrecy::SecretString;
use std::path::PathBuf;

/// A parsed, normalized remote repository URL.
#[derive(Debug, Clone)]
pub struct RemoteUrl {
    /// URL passed directly to `git clone` (HTTPS with `.git` suffix, or `file://` for local paths).
    pub clone_url: String,
    /// `owner/repo` slug when derivable from the URL (absent for `file://` URLs).
    pub owner_repo: Option<String>,
}

/// A shallow clone held in a temporary directory.
///
/// The temp directory is cleaned up when this value is dropped.
pub struct ClonedRepo {
    /// Path to the root of the cloned repository.
    pub path: PathBuf,
    /// `owner/repo` slug when derivable from the original URL.
    pub owner_repo: Option<String>,
    // RAII guard — keeps the temp dir alive for as long as this struct is alive.
    _guard: tempfile::TempDir,
}

impl std::fmt::Debug for ClonedRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClonedRepo")
            .field("path", &self.path)
            .field("owner_repo", &self.owner_repo)
            .finish()
    }
}

/// Parses and normalises a repository identifier into a canonical HTTPS URL.
///
/// Accepted input forms:
/// - `owner/repo` — shorthand, resolved against `base_host` (default: `github.com`)
/// - `https://<host>/owner/repo[.git]` — full HTTPS URL
/// - `git@...` — returns [`RemoteRepoError::SshNotSupported`]
pub fn parse_remote_url(
    input: &str,
    base_host: Option<&str>,
) -> Result<RemoteUrl, RemoteRepoError> {
    let input = input.trim();

    if input.starts_with("git@") || input.starts_with("ssh://") {
        return Err(RemoteRepoError::SshNotSupported);
    }

    if input.starts_with("file://") {
        return Ok(RemoteUrl {
            clone_url: input.to_string(),
            owner_repo: None,
        });
    }

    if let Some(rest) = input.strip_prefix("https://") {
        return parse_https_rest(rest);
    }

    if input.contains("://") {
        return Err(RemoteRepoError::InvalidUrl(format!(
            "unsupported scheme in '{input}'; use HTTPS or owner/repo shorthand"
        )));
    }

    // Treat as owner/repo shorthand.
    let parts: Vec<&str> = input.splitn(3, '/').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
        return Err(RemoteRepoError::InvalidUrl(format!(
            "'{input}' is not a valid owner/repo shorthand or HTTPS URL"
        )));
    }
    let owner = parts[0];
    let repo = parts[1].trim_end_matches(".git");
    if repo.is_empty() {
        return Err(RemoteRepoError::InvalidUrl(format!(
            "'{input}' has an empty repository name"
        )));
    }
    let host = base_host.unwrap_or("github.com");
    Ok(RemoteUrl {
        clone_url: format!("https://{host}/{owner}/{repo}.git"),
        owner_repo: Some(format!("{owner}/{repo}")),
    })
}

fn parse_https_rest(rest: &str) -> Result<RemoteUrl, RemoteRepoError> {
    // rest = "host/owner/repo[.git][/...]"
    let mut segments = rest.splitn(4, '/');
    let host = segments.next().unwrap_or("");
    let owner = segments.next().unwrap_or("");
    let repo_raw = segments.next().unwrap_or("");

    if host.is_empty() || owner.is_empty() || repo_raw.is_empty() {
        return Err(RemoteRepoError::InvalidUrl(format!(
            "'https://{rest}' is missing host, owner, or repository name"
        )));
    }

    let repo = repo_raw.trim_end_matches(".git");
    if repo.is_empty() {
        return Err(RemoteRepoError::InvalidUrl(format!(
            "'https://{rest}' has an empty repository name"
        )));
    }

    Ok(RemoteUrl {
        clone_url: format!("https://{host}/{owner}/{repo}.git"),
        owner_repo: Some(format!("{owner}/{repo}")),
    })
}

/// Shallow-clones a repository into a temporary directory.
///
/// Returns a [`ClonedRepo`] whose path can be passed to the rest of the
/// validation pipeline. The temporary directory is deleted when the
/// returned value is dropped.
///
/// # Arguments
///
/// - `url`: Repository identifier — any form accepted by [`parse_remote_url`].
/// - `auth`: Optional bearer token (PAT or installation access token) used as
///   `x-access-token` in the HTTPS URL.
/// - `base_host`: Host to use when resolving `owner/repo` shorthand.
///   Defaults to `github.com` when `None`.
pub fn shallow_clone(
    url: &str,
    auth: Option<&SecretString>,
    base_host: Option<&str>,
) -> Result<ClonedRepo, RemoteRepoError> {
    let remote_url = parse_remote_url(url, base_host)?;

    let clone_url = match auth {
        Some(token) if remote_url.clone_url.starts_with("https://") => {
            let raw = token.expose_secret();
            let without_scheme = remote_url
                .clone_url
                .strip_prefix("https://")
                .unwrap_or(&remote_url.clone_url);
            format!("https://x-access-token:{raw}@{without_scheme}")
        }
        _ => remote_url.clone_url.clone(),
    };

    let temp_dir = tempfile::TempDir::new()?;

    let output = std::process::Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--no-tags",
            "--single-branch",
            &clone_url,
        ])
        .arg(temp_dir.path())
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                RemoteRepoError::GitNotFound
            } else {
                RemoteRepoError::Io(e)
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let safe_stderr = match auth {
            Some(token) => stderr.replace(token.expose_secret(), "<token>"),
            None => stderr,
        };
        return Err(RemoteRepoError::CloneFailed {
            url: remote_url.clone_url,
            stderr: safe_stderr.trim().to_string(),
        });
    }

    Ok(ClonedRepo {
        path: temp_dir.path().to_path_buf(),
        owner_repo: remote_url.owner_repo,
        _guard: temp_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_remote_url ---

    #[test]
    fn test_parse_owner_repo_shorthand() {
        let url = parse_remote_url("owner/repo", None).unwrap();
        assert_eq!(url.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(url.owner_repo.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn test_parse_owner_repo_with_git_suffix() {
        let url = parse_remote_url("owner/repo.git", None).unwrap();
        assert_eq!(url.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(url.owner_repo.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn test_parse_owner_repo_custom_host() {
        let url = parse_remote_url("owner/repo", Some("github.mycompany.com")).unwrap();
        assert_eq!(url.clone_url, "https://github.mycompany.com/owner/repo.git");
        assert_eq!(url.owner_repo.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn test_parse_https_url_no_git_suffix() {
        let url = parse_remote_url("https://github.com/owner/repo", None).unwrap();
        assert_eq!(url.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(url.owner_repo.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn test_parse_https_url_with_git_suffix() {
        let url = parse_remote_url("https://github.com/owner/repo.git", None).unwrap();
        assert_eq!(url.clone_url, "https://github.com/owner/repo.git");
        assert_eq!(url.owner_repo.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn test_parse_https_ghe() {
        let url = parse_remote_url("https://github.mycompany.com/owner/repo.git", None).unwrap();
        assert_eq!(url.clone_url, "https://github.mycompany.com/owner/repo.git");
        assert_eq!(url.owner_repo.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn test_parse_ssh_returns_error() {
        let err = parse_remote_url("git@github.com:owner/repo.git", None).unwrap_err();
        assert!(matches!(err, RemoteRepoError::SshNotSupported));
    }

    #[test]
    fn test_parse_unsupported_scheme() {
        let err = parse_remote_url("ftp://github.com/owner/repo", None).unwrap_err();
        assert!(matches!(err, RemoteRepoError::InvalidUrl(_)));
    }

    #[test]
    fn test_parse_empty_owner() {
        let err = parse_remote_url("/repo", None).unwrap_err();
        assert!(matches!(err, RemoteRepoError::InvalidUrl(_)));
    }

    #[test]
    fn test_parse_missing_repo() {
        let err = parse_remote_url("owner", None).unwrap_err();
        assert!(matches!(err, RemoteRepoError::InvalidUrl(_)));
    }

    #[test]
    fn test_parse_too_many_segments() {
        // Extra path segments after owner/repo are tolerated for HTTPS form.
        let url = parse_remote_url("https://github.com/owner/repo/extra", None).unwrap();
        assert_eq!(url.clone_url, "https://github.com/owner/repo.git");
    }

    // --- shallow_clone (requires `git`) ---

    #[test]
    fn test_shallow_clone_local_bare_repo() {
        use std::fs;
        use std::process::Command;

        // Create a bare source repo.
        let src = tempfile::TempDir::new().unwrap();
        Command::new("git")
            .args(["init", "--bare", src.path().to_str().unwrap()])
            .output()
            .expect("git init --bare failed");

        // Create a working clone of the bare repo so we can add a commit.
        let work = tempfile::TempDir::new().unwrap();
        Command::new("git")
            .args([
                "clone",
                src.path().to_str().unwrap(),
                work.path().to_str().unwrap(),
            ])
            .output()
            .expect("git clone for setup failed");

        // Configure identity and add a commit.
        for (key, val) in [("user.email", "test@test.com"), ("user.name", "Test")] {
            Command::new("git")
                .args(["-C", work.path().to_str().unwrap(), "config", key, val])
                .output()
                .unwrap();
        }
        fs::write(work.path().join(".github/CODEOWNERS"), "* @owner\n").ok();
        fs::create_dir_all(work.path().join(".github")).unwrap();
        fs::write(work.path().join(".github/CODEOWNERS"), "* @owner\n").unwrap();
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "add", "."])
            .output()
            .unwrap();
        Command::new("git")
            .args(["-C", work.path().to_str().unwrap(), "commit", "-m", "init"])
            .output()
            .unwrap();
        Command::new("git")
            .args([
                "-C",
                work.path().to_str().unwrap(),
                "push",
                "origin",
                "HEAD",
            ])
            .output()
            .unwrap();

        // Clone via our helper using a file:// URL.
        let file_url = format!("file://{}", src.path().display());
        let cloned = shallow_clone(&file_url, None, None).unwrap();

        assert!(cloned.path.join(".github/CODEOWNERS").exists());
    }

    #[test]
    fn test_shallow_clone_bad_url_errors() {
        let err = shallow_clone("git@github.com:owner/repo.git", None, None).unwrap_err();
        assert!(matches!(err, RemoteRepoError::SshNotSupported));
    }
}
