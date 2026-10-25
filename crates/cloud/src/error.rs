//! What went wrong, sorted so the UI can say it in words (design system voice: the item,
//! the reason, the fix; no apologies).

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// rclone isn't installed (or the rcd isn't running).
    NotRunning,
    /// A call took longer than its budget.
    Timeout,
    /// Socket or process trouble on this computer.
    Io,
    /// Google is slowing requests (shared app limits).
    RateLimited,
    /// The sign-in no longer works (revoked, expired, wrong client).
    SignedOut,
    /// Google Drive is full.
    StorageFull,
    NotFound,
    PermissionDenied,
    /// Couldn't reach Google.
    Offline,
    /// Anything else rclone said.
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: Kind,
    /// rclone's own message (or ours), for the log and the "details" line.
    pub message: String,
}

impl Error {
    pub fn new(kind: Kind, message: impl Into<String>) -> Error {
        Error { kind, message: message.into() }
    }

    /// Sort an rclone error message.
    pub fn from_rclone(message: &str) -> Error {
        Error { kind: classify(message), message: message.to_string() }
    }

    /// A sentence for the UI.
    pub fn explain(&self) -> String {
        match self.kind {
            Kind::NotRunning => "Google Drive isn't running. rclone stopped or isn't installed.".into(),
            Kind::Timeout => "Google Drive didn't answer in time. Try again.".into(),
            Kind::Io => format!("Couldn't talk to rclone: {}", self.message),
            Kind::RateLimited => "Google Drive is slowing requests from rclone's shared app. Try again in a minute, or add your own client ID in Settings → Google Drive.".into(),
            Kind::SignedOut => "Google Drive needs you to sign in again.".into(),
            Kind::StorageFull => "Google Drive is full. Free up space on Drive, then try again.".into(),
            Kind::NotFound => "That isn't on Google Drive any more.".into(),
            Kind::PermissionDenied => "Google Drive doesn't allow that here. The file may be shared read-only with you.".into(),
            Kind::Offline => "Couldn't reach Google. Check the internet connection.".into(),
            Kind::Other => self.message.clone(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.explain())
    }
}

impl std::error::Error for Error {}

/// rclone passes Google's reasons through in its messages; match the stable parts.
pub fn classify(message: &str) -> Kind {
    let m = message.to_ascii_lowercase();
    let any = |xs: &[&str]| xs.iter().any(|x| m.contains(x));
    if any(&["storagequotaexceeded", "quota for this account", "drive storage quota"]) {
        Kind::StorageFull
    } else if any(&["invalid_grant", "token has been expired or revoked", "couldn't fetch token", "cannot fetch token", "unauthorized_client", "invalid_client", "empty token found"]) {
        Kind::SignedOut
    } else if any(&["ratelimitexceeded", "userratelimitexceeded", "rate limit exceeded", "too many requests", "error 429"]) {
        Kind::RateLimited
    } else if any(&["insufficientpermissions", "insufficient permission", "permission denied", "forbidden"]) {
        Kind::PermissionDenied
    } else if any(&["no such host", "network is unreachable", "dial tcp", "i/o timeout", "connection reset", "tls handshake timeout", "temporary failure in name resolution"]) {
        Kind::Offline
    } else if any(&["directory not found", "object not found", "file not found", "not found", "error 404"]) {
        Kind::NotFound
    } else {
        Kind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_google_reasons() {
        assert_eq!(classify("googleapi: Error 403: The user's Drive storage quota has been exceeded., storageQuotaExceeded"), Kind::StorageFull);
        assert_eq!(classify("couldn't list files: oauth2: cannot fetch token: 400 Bad Request Response: {\"error\": \"invalid_grant\"}"), Kind::SignedOut);
        assert_eq!(classify("googleapi: Error 403: Rate Limit Exceeded, rateLimitExceeded"), Kind::RateLimited);
        assert_eq!(classify("googleapi: Error 403: Insufficient Permission, insufficientPermissions"), Kind::PermissionDenied);
        assert_eq!(classify("Get \"https://www.googleapis.com/drive/v3/files\": dial tcp: lookup www.googleapis.com: no such host"), Kind::Offline);
        assert_eq!(classify("directory not found"), Kind::NotFound);
        assert_eq!(classify("something odd"), Kind::Other);
    }
}
