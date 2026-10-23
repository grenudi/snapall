// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Validated domain types. Untrusted text is parsed into these once, at the boundary;
//! everything downstream receives a value that cannot be invalid.

use std::fmt;
use std::num::NonZeroU64;

use thiserror::Error;

/// Why a piece of input could not become a domain value.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("config name is empty")]
    EmptyConfigName,
    #[error("config name `{name}` is not allowed: {reason}")]
    InvalidConfigName { name: String, reason: &'static str },
    #[error("description is empty")]
    EmptyDescription,
    #[error("description is too long ({len} characters, maximum {max})")]
    DescriptionTooLong { len: usize, max: usize },
    #[error("description contains a control character")]
    DescriptionControlChar,
    #[error("`{0}` is not a snapshot number")]
    InvalidSnapshotId(String),
}

/// The name of a snapper config (`root`, `home`, ...).
///
/// The charset is deliberately narrow and the name can never start with `-`, so it is
/// always safe to hand to a command line as a single argument.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConfigName(String);

impl ConfigName {
    pub const MAX_LEN: usize = 128;

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let invalid = |reason| ParseError::InvalidConfigName {
            name: input.to_owned(),
            reason,
        };
        if input.is_empty() {
            return Err(ParseError::EmptyConfigName);
        }
        if input.len() > Self::MAX_LEN {
            return Err(invalid("longer than 128 bytes"));
        }
        if input.starts_with('-') {
            return Err(invalid("must not start with `-`"));
        }
        let allowed = |b: u8| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.');
        if !input.bytes().all(allowed) {
            return Err(invalid(
                "only ASCII letters, digits, `_`, `-` and `.` are allowed",
            ));
        }
        Ok(Self(input.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ConfigName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The text stored with a snapshot: non-empty, single-line, bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Description(String);

impl Description {
    pub const MAX_LEN: usize = 200;
    pub const DEFAULT: &'static str = "snapall";

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let text = input.trim();
        if text.is_empty() {
            return Err(ParseError::EmptyDescription);
        }
        if text.chars().any(char::is_control) {
            return Err(ParseError::DescriptionControlChar);
        }
        let len = text.chars().count();
        if len > Self::MAX_LEN {
            return Err(ParseError::DescriptionTooLong {
                len,
                max: Self::MAX_LEN,
            });
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Description {
    fn default() -> Self {
        Self(Self::DEFAULT.to_owned())
    }
}

impl fmt::Display for Description {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A snapshot number as reported by the backend.
///
/// Never zero: snapper uses 0 for "the current system", which is not a snapshot and must
/// never be handed to `delete`. Making 0 unrepresentable removes that whole class of
/// mistake (snapper itself prints an error for `delete 0` yet exits successfully).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SnapshotId(NonZeroU64);

impl SnapshotId {
    /// `None` for 0, which is not a snapshot.
    pub fn new(number: u64) -> Option<Self> {
        NonZeroU64::new(number).map(Self)
    }

    pub fn get(self) -> u64 {
        self.0.get()
    }

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let text = input.trim();
        text.parse::<u64>()
            .ok()
            .and_then(Self::new)
            .ok_or_else(|| ParseError::InvalidSnapshotId(text.to_owned()))
    }
}

impl fmt::Display for SnapshotId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// How a snapshot relates to others. A `Post` always knows its `Pre`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind {
    Single,
    Pre,
    Post { pre: SnapshotId },
}

/// One existing snapshot, as the backend reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotInfo {
    pub id: SnapshotId,
    pub kind: SnapshotKind,
    /// When it was taken, in the backend's own format (for snapper `2026-10-23 01:26:30`).
    pub date: Option<String>,
    pub user: String,
    /// The cleanup algorithm that may prune it (`number`, `timeline`, ...), if any.
    pub cleanup: Option<String>,
    /// Free text stored with the snapshot. Not a `Description`: it may have been written by
    /// anything, so it is shown, never trusted.
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_names_accept_ordinary_names() {
        for name in ["root", "home", "home_user", "data-2", "a.b"] {
            assert_eq!(ConfigName::parse(name).unwrap().as_str(), name);
        }
    }

    #[test]
    fn config_names_reject_anything_unsafe_for_an_argv() {
        assert_eq!(ConfigName::parse(""), Err(ParseError::EmptyConfigName));
        for bad in ["-rf", "--help", "a b", "a/b", "a;b", "ro\not", "naïve"] {
            assert!(
                matches!(
                    ConfigName::parse(bad),
                    Err(ParseError::InvalidConfigName { .. })
                ),
                "{bad:?} should be rejected"
            );
        }
        assert!(ConfigName::parse(&"x".repeat(ConfigName::MAX_LEN + 1)).is_err());
        assert!(ConfigName::parse(&"x".repeat(ConfigName::MAX_LEN)).is_ok());
    }

    #[test]
    fn descriptions_are_trimmed_and_bounded() {
        assert_eq!(
            Description::parse("  before theming ").unwrap().as_str(),
            "before theming"
        );
        assert_eq!(Description::parse("   "), Err(ParseError::EmptyDescription));
        assert_eq!(
            Description::parse("a\nb"),
            Err(ParseError::DescriptionControlChar)
        );
        assert!(matches!(
            Description::parse(&"é".repeat(Description::MAX_LEN + 1)),
            Err(ParseError::DescriptionTooLong { .. })
        ));
        assert!(Description::parse(&"é".repeat(Description::MAX_LEN)).is_ok());
        assert_eq!(Description::default().as_str(), "snapall");
    }

    #[test]
    fn snapshot_ids_parse_numbers_only() {
        assert_eq!(SnapshotId::parse(" 42\n").unwrap().get(), 42);
        assert!(SnapshotId::parse("").is_err());
        assert!(SnapshotId::parse("-1").is_err());
        assert!(SnapshotId::parse("4x").is_err());
    }

    #[test]
    fn zero_is_the_current_system_not_a_snapshot() {
        assert!(SnapshotId::new(0).is_none());
        assert!(SnapshotId::parse("0").is_err());
        assert_eq!(SnapshotId::new(1).unwrap().get(), 1);
    }
}
