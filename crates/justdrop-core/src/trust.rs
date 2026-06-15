//! Device trust state machine.
//!
//! Defines the trust levels for peer devices and valid transitions.
//! Trust relationships are persisted in SQLite and survive app restarts.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Trust level for a peer device.
///
/// State transitions:
/// ```text
/// Unknown ──► Trusted ──► Favorite
///    │            │           │
///    └────────────┴───────────┘
///                 │
///                 ▼
///             Blocked
/// ```
///
/// Any state can transition to `Blocked`.
/// `Blocked` can only transition back to `Unknown` (full reset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrustLevel {
    /// Device has never been seen or was explicitly reset.
    Unknown,
    /// Device was accepted at least once. May auto-connect with reduced prompts.
    Trusted,
    /// Explicitly marked by user. Auto-accept transfers. Persistent across reinstalls.
    Favorite,
    /// Device must never appear in discovery results or receive connections.
    Blocked,
}

impl TrustLevel {
    /// Serialize to a stable string for SQLite storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            TrustLevel::Unknown => "unknown",
            TrustLevel::Trusted => "trusted",
            TrustLevel::Favorite => "favorite",
            TrustLevel::Blocked => "blocked",
        }
    }

    /// Deserialize from SQLite string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "trusted" => TrustLevel::Trusted,
            "favorite" => TrustLevel::Favorite,
            "blocked" => TrustLevel::Blocked,
            _ => TrustLevel::Unknown,
        }
    }

    /// Whether this device should auto-accept incoming transfers.
    pub fn auto_accept(&self) -> bool {
        matches!(self, TrustLevel::Favorite)
    }

    /// Whether this device should be hidden from discovery.
    pub fn is_blocked(&self) -> bool {
        matches!(self, TrustLevel::Blocked)
    }

    /// Whether connections should proceed with reduced prompts.
    pub fn is_trusted(&self) -> bool {
        matches!(self, TrustLevel::Trusted | TrustLevel::Favorite)
    }

    /// Validate a trust level transition. Returns `None` if the transition is invalid.
    pub fn transition_to(self, target: TrustLevel) -> Option<TrustLevel> {
        match (self, target) {
            // Any → Blocked is always valid
            (_, TrustLevel::Blocked) => Some(TrustLevel::Blocked),
            // Blocked → Unknown (reset) is the only escape from Blocked
            (TrustLevel::Blocked, TrustLevel::Unknown) => Some(TrustLevel::Unknown),
            // Blocked → Trusted/Favorite is invalid (must go through Unknown first)
            (TrustLevel::Blocked, _) => None,
            // Normal upgrade path
            (TrustLevel::Unknown, TrustLevel::Trusted) => Some(TrustLevel::Trusted),
            (TrustLevel::Unknown, TrustLevel::Favorite) => Some(TrustLevel::Favorite),
            (TrustLevel::Trusted, TrustLevel::Favorite) => Some(TrustLevel::Favorite),
            // Downgrade path
            (TrustLevel::Favorite, TrustLevel::Trusted) => Some(TrustLevel::Trusted),
            (TrustLevel::Trusted, TrustLevel::Unknown) => Some(TrustLevel::Unknown),
            (TrustLevel::Favorite, TrustLevel::Unknown) => Some(TrustLevel::Unknown),
            // Identity transition
            (s, t) if s == t => Some(t),
            _ => None,
        }
    }
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for TrustLevel {
    fn default() -> Self {
        TrustLevel::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_upgrade_path() {
        assert_eq!(
            TrustLevel::Unknown.transition_to(TrustLevel::Trusted),
            Some(TrustLevel::Trusted)
        );
        assert_eq!(
            TrustLevel::Trusted.transition_to(TrustLevel::Favorite),
            Some(TrustLevel::Favorite)
        );
    }

    #[test]
    fn any_to_blocked() {
        assert_eq!(
            TrustLevel::Unknown.transition_to(TrustLevel::Blocked),
            Some(TrustLevel::Blocked)
        );
        assert_eq!(
            TrustLevel::Trusted.transition_to(TrustLevel::Blocked),
            Some(TrustLevel::Blocked)
        );
        assert_eq!(
            TrustLevel::Favorite.transition_to(TrustLevel::Blocked),
            Some(TrustLevel::Blocked)
        );
    }

    #[test]
    fn blocked_can_only_reset() {
        assert_eq!(
            TrustLevel::Blocked.transition_to(TrustLevel::Unknown),
            Some(TrustLevel::Unknown)
        );
        assert_eq!(
            TrustLevel::Blocked.transition_to(TrustLevel::Trusted),
            None
        );
        assert_eq!(
            TrustLevel::Blocked.transition_to(TrustLevel::Favorite),
            None
        );
    }

    #[test]
    fn downgrade_valid() {
        assert_eq!(
            TrustLevel::Favorite.transition_to(TrustLevel::Trusted),
            Some(TrustLevel::Trusted)
        );
        assert_eq!(
            TrustLevel::Trusted.transition_to(TrustLevel::Unknown),
            Some(TrustLevel::Unknown)
        );
    }

    #[test]
    fn identity_transition() {
        assert_eq!(
            TrustLevel::Trusted.transition_to(TrustLevel::Trusted),
            Some(TrustLevel::Trusted)
        );
    }

    #[test]
    fn auto_accept_only_favorite() {
        assert!(!TrustLevel::Unknown.auto_accept());
        assert!(!TrustLevel::Trusted.auto_accept());
        assert!(TrustLevel::Favorite.auto_accept());
        assert!(!TrustLevel::Blocked.auto_accept());
    }

    #[test]
    fn roundtrip_serialization() {
        for level in [
            TrustLevel::Unknown,
            TrustLevel::Trusted,
            TrustLevel::Favorite,
            TrustLevel::Blocked,
        ] {
            assert_eq!(TrustLevel::from_str(level.as_str()), level);
        }
    }
}
