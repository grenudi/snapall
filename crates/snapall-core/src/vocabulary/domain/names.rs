// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Names and numbers that are parsed once, at the boundary: an area, a description, a snapshot number, a restore point.

use std::fmt;
use std::num::NonZeroU64;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("the area's name is empty")]
    EmptyAreaName,
    #[error("the area name `{name}` is not allowed: {reason}")]
    InvalidAreaName { name: String, reason: &'static str },
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
        Self(crate::vocabulary::calendar::compact_utc(seconds))
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
pub struct AreaName(String);

impl AreaName {
    pub const MAX_LEN: usize = 128;

    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let invalid = |reason| ParseError::InvalidAreaName {
            name: input.to_owned(),
            reason,
        };
        if input.is_empty() {
            return Err(ParseError::EmptyAreaName);
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

impl fmt::Display for AreaName {
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
