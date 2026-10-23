//! The application use case, written against the port only.

use thiserror::Error;

use crate::domain::{ConfigName, Description, SnapshotId, SnapshotInfo};
use crate::plan::{plan, Plan, PlanError};
use crate::port::{BackendError, SnapshotBackend};

/// What happened for one config.
#[derive(Debug)]
pub struct Outcome {
    pub config: ConfigName,
    pub result: Result<SnapshotId, BackendError>,
}

/// One outcome per planned config, in plan order.
#[derive(Debug)]
pub struct Report {
    pub outcomes: Vec<Outcome>,
}

impl Report {
    pub fn failures(&self) -> usize {
        self.outcomes.iter().filter(|o| o.result.is_err()).count()
    }

    pub fn is_success(&self) -> bool {
        self.failures() == 0
    }
}

#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error(transparent)]
    Backend(#[from] BackendError),
    #[error(transparent)]
    Plan(#[from] PlanError),
}

/// Ask the backend what exists, then decide what to do. No snapshot is taken.
pub fn plan_snapshots(
    backend: &dyn SnapshotBackend,
    only: Option<&[ConfigName]>,
) -> Result<Plan, SnapshotError> {
    let available = backend.list_configs()?;
    Ok(plan(only, &available)?)
}

/// Execute a plan. One failing config does not stop the others: every config gets its
/// attempt and the caller sees every outcome.
pub fn run_plan(backend: &dyn SnapshotBackend, plan: &Plan, description: &Description) -> Report {
    let outcomes = plan
        .targets()
        .iter()
        .map(|config| Outcome {
            config: config.clone(),
            result: backend.create_snapshot(config, description),
        })
        .collect();
    Report { outcomes }
}

pub fn snapshot_all(
    backend: &dyn SnapshotBackend,
    only: Option<&[ConfigName]>,
    description: &Description,
) -> Result<Report, SnapshotError> {
    let plan = plan_snapshots(backend, only)?;
    Ok(run_plan(backend, &plan, description))
}

/// Fail fast, with our own message, when `config` is not one the backend manages.
fn ensure_config(backend: &dyn SnapshotBackend, config: &ConfigName) -> Result<(), SnapshotError> {
    if backend.list_configs()?.contains(config) {
        Ok(())
    } else {
        Err(PlanError::UnknownConfigs(vec![config.clone()]).into())
    }
}

/// The snapshots of one config, oldest first.
pub fn snapshots_of(
    backend: &dyn SnapshotBackend,
    config: &ConfigName,
) -> Result<Vec<SnapshotInfo>, SnapshotError> {
    ensure_config(backend, config)?;
    Ok(backend.list_snapshots(config)?)
}

/// The snapshots of every config, configs sorted by name. A failure for any config is an
/// error: a partial listing would look complete.
pub fn all_snapshots(
    backend: &dyn SnapshotBackend,
) -> Result<Vec<(ConfigName, Vec<SnapshotInfo>)>, SnapshotError> {
    let mut configs = backend.list_configs()?;
    configs.sort();
    configs.dedup();
    configs
        .into_iter()
        .map(|config| {
            let snapshots = backend.list_snapshots(&config)?;
            Ok((config, snapshots))
        })
        .collect()
}

/// Delete one snapshot of an existing config.
pub fn delete_snapshot(
    backend: &dyn SnapshotBackend,
    config: &ConfigName,
    id: SnapshotId,
) -> Result<(), SnapshotError> {
    ensure_config(backend, config)?;
    Ok(backend.delete_snapshot(config, id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::InMemoryBackend;

    fn desc() -> Description {
        Description::parse("test").unwrap()
    }

    #[test]
    fn snapshots_every_config_by_default() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        let report = snapshot_all(&backend, None, &desc()).unwrap();
        assert!(report.is_success());
        assert_eq!(report.outcomes.len(), 2);
        assert_eq!(backend.snapshots().len(), 2);
    }

    #[test]
    fn only_restricts_the_targets() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        let only = [ConfigName::parse("home").unwrap()];
        let report = snapshot_all(&backend, Some(&only), &desc()).unwrap();
        assert_eq!(report.outcomes.len(), 1);
        assert_eq!(backend.snapshots()[0].0.as_str(), "home");
    }

    #[test]
    fn one_failure_does_not_stop_the_rest_and_is_reported() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]).failing_on("home");
        let report = snapshot_all(&backend, None, &desc()).unwrap();
        assert_eq!(report.outcomes.len(), 2);
        assert_eq!(report.failures(), 1);
        assert!(!report.is_success());
        assert_eq!(backend.snapshots().len(), 1);
    }

    #[test]
    fn planning_alone_takes_no_snapshot() {
        let backend = InMemoryBackend::with_configs(&["root"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        assert_eq!(plan.targets().len(), 1);
        assert!(backend.snapshots().is_empty());
    }

    #[test]
    fn a_backend_without_configs_is_a_plan_error() {
        let backend = InMemoryBackend::with_configs(&[]);
        assert!(matches!(
            snapshot_all(&backend, None, &desc()),
            Err(SnapshotError::Plan(PlanError::NoConfigs))
        ));
    }

    #[test]
    fn listing_and_deleting_go_through_the_use_cases() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        snapshot_all(&backend, None, &desc()).unwrap();
        let root = ConfigName::parse("root").unwrap();

        let listed = snapshots_of(&backend, &root).unwrap();
        assert_eq!(listed.len(), 1);
        delete_snapshot(&backend, &root, listed[0].id).unwrap();
        assert!(snapshots_of(&backend, &root).unwrap().is_empty());

        let all = all_snapshots(&backend).unwrap();
        assert_eq!(
            all.iter()
                .map(|(c, s)| (c.as_str(), s.len()))
                .collect::<Vec<_>>(),
            [("home", 1), ("root", 0)]
        );
    }

    #[test]
    fn an_unknown_config_is_our_error_not_the_backends() {
        let backend = InMemoryBackend::with_configs(&["root"]);
        let nope = ConfigName::parse("nope").unwrap();
        assert!(matches!(
            snapshots_of(&backend, &nope),
            Err(SnapshotError::Plan(PlanError::UnknownConfigs(_)))
        ));
        assert!(matches!(
            delete_snapshot(&backend, &nope, SnapshotId::new(1).unwrap()),
            Err(SnapshotError::Plan(PlanError::UnknownConfigs(_)))
        ));
    }
}
