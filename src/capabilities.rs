//! Runtime capability detection. `probe()` touches the environment; the
//! `contains` gating logic is pure and is what the registry filters against.

use crate::proc;
use bitflags::bitflags;

bitflags! {
    /// Runtime prerequisites a verb may require. A verb is shown/dispatchable
    /// only when every flag in its `requires` set is present.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Capabilities: u8 {
        const NIRI_SOCKET     = 0b0001;
        const FUZZEL          = 0b0010;
        const FORK            = 0b0100;
        const NIRI_ACTIVITIES = 0b1000;
    }
}

impl Capabilities {
    /// Probe the live environment. Each flag is independent; a failure of one
    /// probe never errors — it just leaves that flag unset.
    pub fn probe() -> Self {
        let mut caps = Capabilities::empty();

        if niri_socket_reachable() {
            caps |= Capabilities::NIRI_SOCKET;
        }
        if proc::which("fuzzel").is_some() {
            caps |= Capabilities::FUZZEL;
        }
        // Fork detection: the activities subcommand exists only on the fork.
        if caps.contains(Capabilities::NIRI_SOCKET)
            && proc::run_capture("niri", &["msg", "--json", "activities"]).is_ok()
        {
            caps |= Capabilities::FORK;
        }
        if jiji_activities_present() {
            caps |= Capabilities::NIRI_ACTIVITIES;
        }
        caps
    }
}

fn niri_socket_reachable() -> bool {
    // `$NIRI_SOCKET` set AND a trivial request succeeds.
    std::env::var_os("NIRI_SOCKET").is_some()
        && proc::run_capture("niri", &["msg", "--json", "workspaces"]).is_ok()
}

fn jiji_activities_present() -> bool {
    proc::which("jiji-activities").is_some()
        && proc::run_capture("jiji-activities", &["--version"]).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains_is_subset_check() {
        let caps = Capabilities::NIRI_SOCKET | Capabilities::FUZZEL;
        assert!(caps.contains(Capabilities::NIRI_SOCKET));
        assert!(caps.contains(Capabilities::NIRI_SOCKET | Capabilities::FUZZEL));
        assert!(!caps.contains(Capabilities::FORK));
        assert!(!caps.contains(Capabilities::NIRI_SOCKET | Capabilities::FORK));
    }

    #[test]
    fn empty_satisfies_nothing_but_empty() {
        let caps = Capabilities::empty();
        assert!(caps.contains(Capabilities::empty()));
        assert!(!caps.contains(Capabilities::FUZZEL));
    }
}
