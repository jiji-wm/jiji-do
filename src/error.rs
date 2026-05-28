//! Typed errors with explicit exit codes. Capability misses map to 69
//! (EX_UNAVAILABLE) per the spec.

use std::fmt;

#[derive(Debug)]
pub enum DoError {
    /// A required capability is absent: `$NIRI_SOCKET` unreachable, or a gated
    /// verb invoked in an unsupported environment, or the menu requested with
    /// fuzzel missing.
    MissingCapability(String),
}

impl DoError {
    /// Process exit code. 69 = EX_UNAVAILABLE.
    pub fn exit_code(&self) -> i32 {
        match self {
            DoError::MissingCapability(_) => 69,
        }
    }
}

impl fmt::Display for DoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DoError::MissingCapability(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for DoError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_miss_is_69() {
        assert_eq!(DoError::MissingCapability("x".into()).exit_code(), 69);
    }

    #[test]
    fn display_is_the_raw_message() {
        let e = DoError::MissingCapability("niri socket unavailable".into());
        assert_eq!(e.to_string(), "niri socket unavailable");
    }
}
