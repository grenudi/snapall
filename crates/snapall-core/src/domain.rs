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
    #[error("the area's name is empty")]
    EmptyConfigName,
    #[error("the area name `{name}` is not allowed: {reason}")]
    InvalidConfigName { name: String, reason: &'static str },
    #[error("description is empty")]
    EmptyDescription,
    #[error("description is too long ({len} characters, maximum {max})")]
    DescriptionTooLong { len: usize, max: usize },
    #[error("description contains a control character")]
    DescriptionControlChar,
    #[error("`{0}` is not a snapshot number")]
    InvalidSnapshotId(String),
    #[error("`{0}` is not a valid restore point name")]
    InvalidPoint(String),
}

/// A restore point: the snapshots that one action took together, one in every area. Restoring a point
/// puts every area back to how it was at that moment with a single action.
///
/// It is a short name made of letters, digits, `-` and `_` (snapall uses the time it was taken, such
/// as `20261025T181500Z`), so it can sit in a snapshot's tag and in a command line unquoted.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point(String);

impl Point {
    pub const MAX_LEN: usize = 64;

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let ok = !input.is_empty()
            && input.len() <= Self::MAX_LEN
            && input
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
        if ok {
            Ok(Self(input.to_owned()))
        } else {
            Err(ParseError::InvalidPoint(input.to_owned()))
        }
    }

    /// The point named for the moment `seconds` (since 1970, UTC) that one action started.
    pub fn at(seconds: u64) -> Self {
        Self(crate::calendar::compact_utc(seconds))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
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
    /// The restore point it belongs to, if one action made it together with snapshots of other areas.
    pub point: Option<Point>,
    /// The bytes only this snapshot holds, when Btrfs quotas make that knowable; `None` otherwise.
    pub used_bytes: Option<u64>,
}

/// How one path differs between a snapshot and the live system now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// It exists now and did not then: restoring would remove it.
    Added,
    /// It existed then and is gone now: restoring would bring it back.
    Removed,
    /// It exists in both and differs (content, type, permissions, owner...): restoring would revert it.
    Changed,
}

/// One path that differs between a snapshot and the live system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub kind: ChangeKind,
    /// An absolute path inside the config's subvolume.
    pub path: String,
}

/// What a restore did, as snapper counts it: files recreated, reverted and removed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RestoreSummary {
    pub created: u64,
    pub modified: u64,
    pub deleted: u64,
}

impl RestoreSummary {
    pub fn total(&self) -> u64 {
        self.created + self.modified + self.deleted
    }
}

/// How a file as one snapshot had it compares with the file as it is now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// The same content, type, permissions and owner: restoring would change nothing.
    Same,
    /// It exists in both and differs: restoring would revert it.
    Changed,
    /// It existed then and is gone now: restoring would bring it back.
    DeletedSince,
    /// It did not exist then: there is nothing in this snapshot to restore.
    AbsentThen,
}

impl FileState {
    /// What to tell a person: how the file in the snapshot compares with the file now.
    pub fn label(self) -> &'static str {
        match self {
            FileState::Same => "same as now",
            FileState::Changed => "differs from now",
            FileState::DeletedSince => "deleted since",
            FileState::AbsentThen => "did not exist",
        }
    }

    /// A stable token for machine-readable output.
    pub fn token(self) -> &'static str {
        match self {
            FileState::Same => "same",
            FileState::Changed => "changed",
            FileState::DeletedSince => "deleted_since",
            FileState::AbsentThen => "absent_then",
        }
    }

    /// Whether restoring from a snapshot in this state would put something back.
    pub fn restorable(self) -> bool {
        matches!(self, FileState::Changed | FileState::DeletedSince)
    }
}

/// One snapshot, and how the file in question stood in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileVersion {
    pub snapshot: SnapshotInfo,
    pub state: FileState,
}

/// How an area's snapshots are kept and cleaned up, as snapper has it set. Fields snapper does not
/// report, or reports in a form snapall does not understand, are `None`: nothing is guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retention {
    /// The newest snapshots kept (`NUMBER_LIMIT`).
    pub keep: Option<u32>,
    /// Snapshots marked important are kept this many (`NUMBER_LIMIT_IMPORTANT`).
    pub keep_important: Option<u32>,
    /// A snapshot younger than this many seconds is never cleaned up (`NUMBER_MIN_AGE`).
    pub min_age_seconds: Option<u64>,
    /// Whether cleaning up numbered snapshots is switched on for this area (`NUMBER_CLEANUP`).
    pub cleanup: bool,
    /// Whether snapper takes snapshots of this area by itself on a schedule (`TIMELINE_CREATE`).
    pub on_a_schedule: bool,
    /// The share of the disk the snapshots may fill before snapper cleans up (`SPACE_LIMIT`), in
    /// thousandths (500 is half), which only acts when Btrfs quotas are on.
    pub space_limit_permille: Option<u16>,
    /// Whether Btrfs quotas are set up for the area (`QGROUP` is set), which is what makes the space
    /// each snapshot holds knowable.
    pub quotas: bool,
}

impl Retention {
    /// From snapper's settings, as the `key -> value` text it reports them in.
    pub fn from_settings(settings: &std::collections::BTreeMap<String, String>) -> Self {
        let text = |key: &str| {
            settings
                .get(key)
                .map(|v| v.trim())
                .filter(|v| !v.is_empty())
        };
        let number = |key: &str| text(key).and_then(|v| v.parse::<u64>().ok());
        Self {
            keep: number("NUMBER_LIMIT").and_then(|n| u32::try_from(n).ok()),
            keep_important: number("NUMBER_LIMIT_IMPORTANT").and_then(|n| u32::try_from(n).ok()),
            min_age_seconds: number("NUMBER_MIN_AGE"),
            cleanup: text("NUMBER_CLEANUP") == Some("yes"),
            on_a_schedule: text("TIMELINE_CREATE") == Some("yes"),
            space_limit_permille: text("SPACE_LIMIT")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|share| (0.0..=1.0).contains(share))
                .map(|share| (share * 1000.0).round() as u16),
            quotas: text("QGROUP").is_some(),
        }
    }
}

/// A setting of a config that snapall chooses for the configs it creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigOption {
    /// Whether snapper takes snapshots by itself on a schedule (hourly, daily, ...). snapall turns
    /// it off for what it sets up: snapshots happen when you ask, so nothing piles up on its own.
    TimelineCreate(bool),
}

impl ConfigOption {
    /// What the setting means, for the audit log.
    pub fn describe(self) -> String {
        match self {
            ConfigOption::TimelineCreate(on) => {
                format!("snapshots on a schedule: {}", if on { "on" } else { "off" })
            }
        }
    }
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
