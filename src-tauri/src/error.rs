use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum AppError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("scope denied: {0}")]
    ScopeDenied(String),
    #[error("privileged operation requires direct-download build")]
    PrivilegedUnavailable,
    #[error("path not allowed: {0}")]
    PathNotAllowed(String),
    #[error("not implemented yet")]
    NotImplemented,
    #[error("sqlite error: {0}")]
    Sqlite(String),
    #[error("scan error: {0}")]
    Scan(String),
    #[error("no active scan with id {0}")]
    NoSuchScan(i64),
    #[error("scan already running")]
    ScanAlreadyRunning,
}

impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

#[allow(dead_code)]
pub type AppResult<T> = Result<T, AppError>;
