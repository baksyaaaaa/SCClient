use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("Unable to parse JSON: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Database execution failed: {0}")]
    SqlError(#[from] sqlx::Error),

    #[error(transparent)]
    IoError(#[from] csio::IOError),

    #[error(transparent)]
    StdIoError(#[from] std::io::Error),

    #[error(transparent)]
    RequestError(#[from] csclient_net::RequestError),

    #[error(transparent)]
    PathsError(#[from] csclient_common::PathsError),

    #[error(transparent)]
    DbError(#[from] csclient_db::DbError),

    #[error("launcher core is not initialized")]
    NotInitialized,

    #[error("launcher core is already initialized")]
    AlreadyInitialized,

    #[error("invalid settings profile: {reason}")]
    InvalidSettingsProfile { reason: String },

    #[error(transparent)]
    JavaError(#[from] csclient_java::JavaError),

    #[error(transparent)]
    UrlError(#[from] url::ParseError),

    #[error(transparent)]
    EventError(#[from] csclient_events::EventError),

    #[error(transparent)]
    PackageError(#[from] csclient_content::packages::PackageError),

    #[error(transparent)]
    AuthError(#[from] csclient_auth::AuthError),

    #[error(transparent)]
    ClusterError(#[from] crate::clusters::ClusterError),

    #[error(transparent)]
    McError(#[from] csclient_mc::McError),

    #[error(transparent)]
    ContentError(#[from] csclient_content::ContentError),

    #[error(transparent)]
    BundleError(#[from] csclient_content::bundles::BundleError),

    #[error(transparent)]
    GameError(#[from] crate::game::GameError),

    #[error(transparent)]
    LogsError(#[from] csclient_cluster::logs::LogsError),

    #[error(transparent)]
    ScreenshotsError(#[from] csclient_cluster::screenshots::ScreenshotsError),

    #[error(transparent)]
    BisectError(#[from] crate::bisect::BisectError),

    #[error(transparent)]
    SharingError(#[from] crate::sharing::SharingError),

    #[error(transparent)]
    SyncError(#[from] crate::sync::SyncError),

    #[error("minecraft: {0}")]
    Minecraft(String),

    #[error("CSClient exports are not checked for improvements")]
    ShareNotScreened,
}

impl LauncherError {
    #[must_use]
    pub fn auth_guidance(&self) -> Option<csclient_auth::AuthErrorGuidance> {
        match self {
            LauncherError::AuthError(csclient_auth::AuthError::Minecraft(err)) => {
                csclient_auth::diagnose_auth_error(err)
            }
            _ => None,
        }
    }

    /// Not something to report the dialog is already gone by the time it lands
    #[must_use]
    pub fn is_login_cancelled(&self) -> bool {
        matches!(
            self,
            LauncherError::AuthError(csclient_auth::AuthError::LoginCancelled)
        )
    }

    /// A dialog the user backed out of rather than anything that went wrong
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.is_login_cancelled()
            || matches!(
                self,
                LauncherError::JavaError(csclient_java::JavaError::Cancelled)
            )
    }

    /// Whether the install is missing pieces and so is worth repairing
    #[must_use]
    pub fn indicates_missing_files(&self) -> bool {
        let mut source: Option<&(dyn std::error::Error + 'static)> = Some(self);

        // Walk the chain the `io::Error` carrying the kind is usually wrapped
        // two or three layers deep by the time this surfaces
        while let Some(err) = source {
            if let Some(io) = err.downcast_ref::<std::io::Error>()
                && io.kind() == std::io::ErrorKind::NotFound
            {
                return true;
            }
            source = err.source();
        }

        false
    }
}

pub trait SentryExclusion {
    fn is_sentry_excluded(&self) -> bool {
        false
    }
}

impl SentryExclusion for LauncherError {
    fn is_sentry_excluded(&self) -> bool {
        match self {
            LauncherError::StdIoError(e) => e.is_sentry_excluded(),
            LauncherError::IoError(e) => e.is_sentry_excluded(),
            LauncherError::RequestError(e) => e.is_transient(),
            LauncherError::JavaError(e) => e.is_transient(),
            _ => false,
        }
    }
}

impl SentryExclusion for std::io::Error {
    fn is_sentry_excluded(&self) -> bool {
        use std::io::ErrorKind;

        // `ErrorKind::StorageFull` is still unstable so match raw OS codes
        // Platform-gated 112 is the unrelated EHOSTDOWN on Linux
        if let Some(code) = self.raw_os_error() {
            #[cfg(unix)]
            if code == 28 {
                // ENOSPC
                return true;
            }
            #[cfg(windows)]
            if code == 112 || code == 39 {
                // ERROR_DISK_FULL / ERROR_HANDLE_DISK_FULL
                return true;
            }
            let _ = code;
        }

        matches!(
            self.kind(),
            ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionReset
                | ErrorKind::ConnectionAborted
                | ErrorKind::NotConnected
                | ErrorKind::TimedOut
        )
    }
}

impl SentryExclusion for reqwest::Error {
    fn is_sentry_excluded(&self) -> bool {
        // Connectivity problems are the user's network not a launcher bug
        self.is_timeout() || self.is_connect()
    }
}

impl SentryExclusion for csio::IOError {
    fn is_sentry_excluded(&self) -> bool {
        match self {
            csio::IOError::IOError(source) | csio::IOError::PathIOError { source, .. } => {
                source.is_sentry_excluded()
            }
            _ => false,
        }
    }
}

pub type LauncherResult<T> = Result<T, LauncherError>;

#[cfg(test)]
mod tests {
    use super::*;

    /// Raw OS error rather than a synthesized `ErrorKind` same kind but only
    /// this one renders the `(os error 2)` text users report
    fn not_found(path: &str) -> LauncherError {
        LauncherError::IoError(csio::IOError::PathIOError {
            source: std::io::Error::from_raw_os_error(2),
            path: path.to_string(),
        })
    }

    #[test]
    fn a_missing_asset_directory_asks_for_a_repair() {
        let path = "/Users/someone/Library/Application Support/org.CS.CSClient-dev/metadata/assets";
        let err = not_found(path);

        assert!(err.indicates_missing_files(), "{err}");
        assert!(err.to_string().contains(path), "{err}");
    }

    #[test]
    fn a_not_found_nested_several_layers_deep_is_still_found() {
        let err = LauncherError::McError(csclient_mc::McError::Io(csio::IOError::PathIOError {
            source: std::io::Error::from(std::io::ErrorKind::NotFound),
            path: "metadata/libraries/foo.jar".to_string(),
        }));

        assert!(err.indicates_missing_files(), "{err}");
    }

    #[test]
    fn dismissing_a_dialog_is_a_cancellation_not_a_failure() {
        assert!(LauncherError::JavaError(csclient_java::JavaError::Cancelled).is_cancelled());
        assert!(LauncherError::AuthError(csclient_auth::AuthError::LoginCancelled).is_cancelled());
    }

    /// The whole point of the split anything that is not a dismissal keeps
    /// reporting at `error`
    #[test]
    fn real_java_failures_are_not_cancellations() {
        for err in [
            csclient_java::JavaError::MissingJava,
            csclient_java::JavaError::PackageNotFound { major: 21 },
            csclient_java::JavaError::InvalidJavaPath {
                path: "/opt/not-java".to_string(),
            },
            csclient_java::JavaError::VersionMismatch {
                expected: 21,
                found: 8,
            },
        ] {
            let err = LauncherError::JavaError(err);
            assert!(!err.is_cancelled(), "{err}");
        }

        assert!(!LauncherError::NotInitialized.is_cancelled());
        assert!(!not_found("metadata/assets").is_cancelled());
    }

    #[test]
    fn failures_a_repair_could_not_fix_are_left_alone() {
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::ConnectionRefused,
            std::io::ErrorKind::TimedOut,
        ] {
            let err = LauncherError::IoError(csio::IOError::PathIOError {
                source: std::io::Error::from(kind),
                path: "metadata/assets".to_string(),
            });
            assert!(!err.indicates_missing_files(), "{kind:?} should not repair");
        }

        assert!(!LauncherError::NotInitialized.indicates_missing_files());
        assert!(!LauncherError::Minecraft("bad manifest".into()).indicates_missing_files());
    }
}
