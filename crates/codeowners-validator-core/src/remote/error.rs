use thiserror::Error;

#[derive(Debug, Error)]
pub enum RemoteRepoError {
    #[error("invalid repository URL: {0}")]
    InvalidUrl(String),
    #[error("SSH URLs are not supported; use HTTPS or owner/repo shorthand")]
    SshNotSupported,
    #[error("`git` binary not found in PATH")]
    GitNotFound,
    #[error("git clone failed for {url}: {stderr}")]
    CloneFailed { url: String, stderr: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
