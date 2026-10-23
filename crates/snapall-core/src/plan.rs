// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Functional core: decide *which* configs to snapshot. Pure and total.

use std::collections::BTreeSet;

use thiserror::Error;

use crate::domain::ConfigName;

/// The configs that will be snapshotted.
///
/// Invariant (enforced by `plan`, the only constructor): non-empty, sorted, unique, and
/// every entry is a config the backend reported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    targets: Vec<ConfigName>,
}

impl Plan {
    pub fn targets(&self) -> &[ConfigName] {
        &self.targets
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlanError {
    #[error("snapper reports no configs; create one first, for example `snapper -c home create-config /home`")]
    NoConfigs,
    #[error("an empty list of configs was requested")]
    NothingRequested,
    #[error(
        "unknown config(s): {}",
        .0.iter().map(ConfigName::as_str).collect::<Vec<_>>().join(", ")
    )]
    UnknownConfigs(Vec<ConfigName>),
}

/// Choose targets. `requested = None` means "everything that exists"; an explicit
/// request must name only configs that exist (fail fast, never silently skip).
pub fn plan(requested: Option<&[ConfigName]>, available: &[ConfigName]) -> Result<Plan, PlanError> {
    if available.is_empty() {
        return Err(PlanError::NoConfigs);
    }
    let available: BTreeSet<&ConfigName> = available.iter().collect();

    let targets: BTreeSet<ConfigName> = match requested {
        None => available.into_iter().cloned().collect(),
        Some([]) => return Err(PlanError::NothingRequested),
        Some(requested) => {
            let unknown: BTreeSet<ConfigName> = requested
                .iter()
                .filter(|name| !available.contains(name))
                .cloned()
                .collect();
            if !unknown.is_empty() {
                return Err(PlanError::UnknownConfigs(unknown.into_iter().collect()));
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

    fn names(list: &[&str]) -> Vec<ConfigName> {
        list.iter().map(|n| ConfigName::parse(n).unwrap()).collect()
    }

    #[test]
    fn default_is_every_available_config_sorted_and_unique() {
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
    fn unknown_configs_fail_fast_and_are_all_reported() {
        let req = names(&["root", "nope", "nada"]);
        assert_eq!(
            plan(Some(&req), &names(&["root"])),
            Err(PlanError::UnknownConfigs(names(&["nada", "nope"])))
        );
    }

    #[test]
    fn no_configs_and_empty_requests_are_errors_not_empty_plans() {
        assert_eq!(plan(None, &[]), Err(PlanError::NoConfigs));
        assert_eq!(
            plan(Some(&[]), &names(&["root"])),
            Err(PlanError::NothingRequested)
        );
    }
}
