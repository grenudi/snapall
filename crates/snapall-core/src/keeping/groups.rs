// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Groups: a name for a set of protected areas, so that "take a snapshot of my theming" is one thing to
//! say. `all` is always there and means every protected area; it is not stored and cannot be changed.
//! The others are the person's own.
//!
//! They are kept as plain text a person can read and edit, one group a line:
//!
//! ```text
//! # snapall groups: a name, then the areas it covers
//! theming: home, data
//! ```
//!
//! A group names areas, not paths: whether an area is protected is snapper's to say, and a group that
//! names one that no longer is gets refused with the name of the missing one, never silently shortened.

use std::collections::BTreeMap;

use thiserror::Error;

use crate::vocabulary::domain::AreaName;

/// The group that is every protected area.
pub const ALL: &str = "all";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupName(String);

impl GroupName {
    pub const MAX_LEN: usize = 32;

    /// Lower-case letters, digits, `-` and `_`, starting with a letter; and not `all`.
    pub fn parse(text: &str) -> Result<Self, GroupsError> {
        let text = text.trim();
        let mut chars = text.chars();
        let fine = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
        if !fine || text.chars().count() > Self::MAX_LEN {
            return Err(GroupsError::BadName(text.to_owned()));
        }
        if text == ALL {
            return Err(GroupsError::Reserved);
        }
        Ok(Self(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GroupName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which areas a person asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Wanted {
    /// Every protected area.
    All,
    Named(GroupName),
}

impl Wanted {
    pub fn parse(text: &str) -> Result<Self, GroupsError> {
        if text.trim() == ALL {
            Ok(Wanted::All)
        } else {
            GroupName::parse(text).map(Wanted::Named)
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GroupsError {
    #[error("`{0}` cannot be a group name: use lower-case letters, digits, - and _, starting with a letter, at most 32 long")]
    BadName(String),
    #[error(
        "`all` is the group of every protected area: it is always there and cannot be changed"
    )]
    Reserved,
    #[error("a group needs at least one area")]
    Empty,
    #[error("there is no group called `{name}`{}", which_exist(.known))]
    Unknown { name: String, known: Vec<String> },
    #[error("the group `{group}` names {}, which {} no longer protected: put {} back with `snapall setup`, or change the group with `snapall groups`", .missing.join(", "), if .missing.len() == 1 { "is" } else { "are" }, if .missing.len() == 1 { "it" } else { "them" })]
    Missing { group: String, missing: Vec<String> },
    #[error("line {line}: {why}")]
    Syntax { line: usize, why: String },
}

fn which_exist(names: &[String]) -> String {
    if names.is_empty() {
        ", and none have been made yet".to_owned()
    } else {
        format!(" (there are: {})", names.join(", "))
    }
}

/// The groups a person made.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Groups {
    named: BTreeMap<GroupName, Vec<AreaName>>,
}

impl Groups {
    pub fn names(&self) -> impl Iterator<Item = &GroupName> {
        self.named.keys()
    }

    pub fn is_empty(&self) -> bool {
        self.named.is_empty()
    }

    pub fn get(&self, name: &GroupName) -> Option<&[AreaName]> {
        self.named.get(name).map(Vec::as_slice)
    }

    /// Make or replace a group. An area named twice counts once; a group needs an area.
    pub fn set(&mut self, name: GroupName, areas: Vec<AreaName>) -> Result<(), GroupsError> {
        let mut seen = Vec::new();
        for area in areas {
            if !seen.contains(&area) {
                seen.push(area);
            }
        }
        if seen.is_empty() {
            return Err(GroupsError::Empty);
        }
        self.named.insert(name, seen);
        Ok(())
    }

    /// Whether there was such a group.
    pub fn remove(&mut self, name: &GroupName) -> bool {
        self.named.remove(name).is_some()
    }

    /// The areas of what was asked for, out of the ones that are protected now.
    pub fn resolve(
        &self,
        wanted: &Wanted,
        protected: &[AreaName],
    ) -> Result<Vec<AreaName>, GroupsError> {
        let Wanted::Named(name) = wanted else {
            return Ok(protected.to_vec());
        };
        let areas = self.get(name).ok_or_else(|| GroupsError::Unknown {
            name: name.to_string(),
            known: self.names().map(ToString::to_string).collect(),
        })?;
        let missing: Vec<String> = areas
            .iter()
            .filter(|area| !protected.contains(area))
            .map(ToString::to_string)
            .collect();
        if missing.is_empty() {
            Ok(areas.to_vec())
        } else {
            Err(GroupsError::Missing {
                group: name.to_string(),
                missing,
            })
        }
    }

    /// Read the file. A line that cannot be read is an error that says which line and why, not a group
    /// quietly missing.
    pub fn parse(text: &str) -> Result<Groups, GroupsError> {
        let mut groups = Groups::default();
        for (index, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let at = index + 1;
            let syntax = |why: String| GroupsError::Syntax { line: at, why };
            let (name, areas) = line
                .split_once(':')
                .ok_or_else(|| syntax("expected `name: area, area`".to_owned()))?;
            let name = GroupName::parse(name).map_err(|e| syntax(e.to_string()))?;
            if groups.named.contains_key(&name) {
                return Err(syntax(format!("the group `{name}` is defined twice")));
            }
            let areas = areas
                .split(',')
                .map(str::trim)
                .filter(|area| !area.is_empty())
                .map(|area| AreaName::parse(area).map_err(|e| syntax(format!("`{area}`: {e}"))))
                .collect::<Result<Vec<_>, _>>()?;
            groups.set(name, areas).map_err(|e| syntax(e.to_string()))?;
        }
        Ok(groups)
    }

    pub fn to_text(&self) -> String {
        let mut text = String::from("# snapall groups: a name, then the areas it covers\n");
        for (name, areas) in &self.named {
            let list: Vec<&str> = areas.iter().map(AreaName::as_str).collect();
            text.push_str(&format!("{name}: {}\n", list.join(", ")));
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(name: &str) -> AreaName {
        AreaName::parse(name).unwrap()
    }

    fn name(text: &str) -> GroupName {
        GroupName::parse(text).unwrap()
    }

    fn protected() -> Vec<AreaName> {
        ["data", "home", "root"].map(area).to_vec()
    }

    #[test]
    fn a_group_name_is_plain_lower_case_and_never_all() {
        for good in ["theming", "my-games", "work_2", "a"] {
            assert!(GroupName::parse(good).is_ok(), "{good}");
        }
        for bad in [
            "",
            "Theming",
            "2fast",
            "has space",
            "a/b",
            "ünï",
            &"x".repeat(33),
        ] {
            assert!(
                matches!(GroupName::parse(bad), Err(GroupsError::BadName(_))),
                "{bad:?}"
            );
        }
        assert_eq!(GroupName::parse("all"), Err(GroupsError::Reserved));
        assert_eq!(
            Wanted::parse("all"),
            Ok(Wanted::All),
            "but all can be asked for"
        );
        assert_eq!(Wanted::parse("theming"), Ok(Wanted::Named(name("theming"))));
    }

    #[test]
    fn all_is_every_protected_area_whatever_groups_exist() {
        let mut groups = Groups::default();
        groups.set(name("theming"), vec![area("home")]).unwrap();
        assert_eq!(
            groups.resolve(&Wanted::All, &protected()).unwrap(),
            protected()
        );
    }

    #[test]
    fn a_group_is_its_areas_in_the_order_it_names_them_each_once() {
        let mut groups = Groups::default();
        groups
            .set(name("theming"), ["home", "data", "home"].map(area).to_vec())
            .unwrap();
        assert_eq!(
            groups.get(&name("theming")).unwrap(),
            ["home", "data"].map(area)
        );
        assert_eq!(
            groups
                .resolve(&Wanted::Named(name("theming")), &protected())
                .unwrap(),
            ["home", "data"].map(area)
        );
        assert_eq!(groups.set(name("empty"), vec![]), Err(GroupsError::Empty));
    }

    #[test]
    fn a_group_that_names_an_area_no_longer_protected_is_refused_naming_it_not_shortened() {
        let mut groups = Groups::default();
        groups
            .set(name("theming"), ["home", "gone", "data"].map(area).to_vec())
            .unwrap();
        let error = groups
            .resolve(&Wanted::Named(name("theming")), &protected())
            .unwrap_err();
        assert_eq!(
            error,
            GroupsError::Missing {
                group: "theming".into(),
                missing: vec!["gone".into()]
            }
        );
        let said = error.to_string();
        assert!(
            said.contains("`theming` names gone, which is no longer protected"),
            "{said}"
        );
        assert!(
            said.contains("snapall setup") && said.contains("snapall groups"),
            "it says what to do: {said}"
        );
    }

    #[test]
    fn an_unknown_group_says_which_exist() {
        let mut groups = Groups::default();
        let said = groups
            .resolve(&Wanted::Named(name("nope")), &protected())
            .unwrap_err()
            .to_string();
        assert!(said.contains("none have been made yet"), "{said}");
        groups.set(name("theming"), vec![area("home")]).unwrap();
        groups.set(name("games"), vec![area("data")]).unwrap();
        let said = groups
            .resolve(&Wanted::Named(name("nope")), &protected())
            .unwrap_err()
            .to_string();
        assert!(said.contains("there are: games, theming"), "{said}");
    }

    #[test]
    fn the_file_is_plain_text_that_survives_a_round_trip_and_hand_editing() {
        let mut groups = Groups::default();
        groups
            .set(name("theming"), ["home", "data"].map(area).to_vec())
            .unwrap();
        groups.set(name("games"), vec![area("data")]).unwrap();
        let text = groups.to_text();
        assert_eq!(text, "# snapall groups: a name, then the areas it covers\ngames: data\ntheming: home, data\n");
        assert_eq!(Groups::parse(&text).unwrap(), groups);
        // what a person might write
        let by_hand = "\n# my groups\n  theming :  home ,data  \n\n# games last\ngames: data\n";
        assert_eq!(Groups::parse(by_hand).unwrap(), groups);
    }

    #[test]
    fn a_line_that_cannot_be_read_says_which_and_why_and_nothing_is_dropped_quietly() {
        for (text, line, why) in [
            ("theming home\n", 1, "expected `name: area, area`"),
            ("ok: home\nBad: home\n", 2, "cannot be a group name"),
            ("all: home\n", 1, "always there and cannot be changed"),
            ("a: home\na: data\n", 2, "defined twice"),
            ("a:\n", 1, "needs at least one area"),
            ("a: home, no way\n", 1, "`no way`"),
        ] {
            let error = Groups::parse(text).unwrap_err();
            let said = error.to_string();
            assert!(
                matches!(error, GroupsError::Syntax { line: l, .. } if l == line),
                "{text:?}: {said}"
            );
            assert!(said.contains(why), "{text:?}: {said}");
        }
    }

    #[test]
    fn a_group_can_be_removed_and_only_once() {
        let mut groups = Groups::default();
        groups.set(name("theming"), vec![area("home")]).unwrap();
        assert!(groups.remove(&name("theming")) && !groups.remove(&name("theming")));
        assert!(groups.is_empty());
    }
}
