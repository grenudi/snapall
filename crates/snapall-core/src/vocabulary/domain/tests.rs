// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The domain types, parsed once at the boundary.

#![cfg(test)]

use super::*;

#[test]
fn area_names_accept_ordinary_names() {
    for name in ["root", "home", "home_user", "data-2", "a.b"] {
        assert_eq!(AreaName::parse(name).unwrap().as_str(), name);
    }
}

#[test]
fn area_names_reject_anything_unsafe_for_an_argv() {
    assert_eq!(AreaName::parse(""), Err(ParseError::EmptyAreaName));
    for bad in ["-rf", "--help", "a b", "a/b", "a;b", "ro\not", "naïve"] {
        assert!(
            matches!(
                AreaName::parse(bad),
                Err(ParseError::InvalidAreaName { .. })
            ),
            "{bad:?} should be rejected"
        );
    }
    assert!(AreaName::parse(&"x".repeat(AreaName::MAX_LEN + 1)).is_err());
    assert!(AreaName::parse(&"x".repeat(AreaName::MAX_LEN)).is_ok());
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
