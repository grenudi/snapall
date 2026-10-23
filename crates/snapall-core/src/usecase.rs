//! The application use case, written against the port only.

use thiserror::Error;

use crate::domain::{ConfigName, Description, SnapshotId};
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
}
