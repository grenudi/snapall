// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Deciding which areas to snapshot. Pure and total.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::vocabulary::domain::AreaName;

/// The areas that will be snapshotted.
///
/// Invariant (enforced by `plan`, the only constructor): non-empty, sorted, unique, and
/// every entry is an area the backend reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    targets: Vec<AreaName>,
}

impl Plan {
    pub fn targets(&self) -> &[AreaName] {
        &self.targets
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanError {
    #[error("There are no protected areas yet, so there is nothing to snapshot. Run `sudo snapall setup` to set one up.")]
    NoAreas,
    #[error("an empty list of areas was requested")]
    NothingRequested,
    #[error(
        "unknown area(s): {} (`snapall list` shows the ones that exist)",
        .0.iter().map(AreaName::as_str).collect::<Vec<_>>().join(", ")
    )]
    UnknownAreas(Vec<AreaName>),
}

/// Choose targets. `requested = None` means "everything that exists"; an explicit
/// request must name only areas that exist (fail fast, never silently skip).
pub fn plan(requested: Option<&[AreaName]>, available: &[AreaName]) -> Result<Plan, PlanError> {
    if available.is_empty() {
        return Err(PlanError::NoAreas);
    }
    let available: BTreeSet<&AreaName> = available.iter().collect();

    let targets: BTreeSet<AreaName> = match requested {
        None => available.into_iter().cloned().collect(),
        Some([]) => return Err(PlanError::NothingRequested),
        Some(requested) => {
            let unknown: BTreeSet<AreaName> = requested
                .iter()
                .filter(|name| !available.contains(name))
                .cloned()
                .collect();
            if !unknown.is_empty() {
                return Err(PlanError::UnknownAreas(unknown.into_iter().collect()));
            }
            requested.iter().cloned().collect()
        }
    };
    Ok(Plan {
        targets: targets.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<AreaName> {
        list.iter().map(|n| AreaName::parse(n).unwrap()).collect()
    }

    #[test]
    fn default_is_every_available_area_sorted_and_unique() {
        let p = plan(None, &names(&["root", "home", "home"])).unwrap();
        assert_eq!(p.targets(), names(&["home", "root"]).as_slice());
    }

    #[test]
    fn explicit_request_is_honoured_and_deduplicated() {
        let req = names(&["home", "home"]);
        let p = plan(Some(&req), &names(&["root", "home"])).unwrap();
        assert_eq!(p.targets(), names(&["home"]).as_slice());
    }

    #[test]
    fn unknown_areas_fail_fast_and_are_all_reported() {
        let req = names(&["root", "nope", "nada"]);
        assert_eq!(
            plan(Some(&req), &names(&["root"])),
            Err(PlanError::UnknownAreas(names(&["nada", "nope"])))
        );
    }

    #[test]
    fn no_areas_and_empty_requests_are_errors_not_empty_plans() {
        assert_eq!(plan(None, &[]), Err(PlanError::NoAreas));
        assert_eq!(
            plan(Some(&[]), &names(&["root"])),
            Err(PlanError::NothingRequested)
        );
    }
}
