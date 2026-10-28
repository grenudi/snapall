// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! A change that takes several steps (a snapshot of every area, a restore point put back area by area)
//! follows a [`Course`]: before each step it asks whether to begin it. That is both how progress is
//! reported and how a change is stopped safely: the answer "no" stops it *between* steps, never inside
//! one, so the steps already done stay done and nothing is left half-way.

/// Told before each step of a change; says whether to go on.
pub trait Course {
    /// About to begin step `done + 1` of `of`, on `area`. `false` stops the change here: the steps
    /// already done stay done, and the rest are not started.
    fn proceed(&self, done: usize, of: usize, area: &str) -> bool;
}

/// Nobody is watching and nobody can stop it: every step goes ahead.
#[derive(Debug, Default, Clone, Copy)]
pub struct Unattended;

impl Course for Unattended {
    fn proceed(&self, _done: usize, _of: usize, _area: &str) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::going_back::points::{restore_point_along, restore_points, MemberResult};
    use crate::looking::snapshots::all_snapshots;
    use crate::ports::testing::InMemoryBackend;
    use crate::taking::run::{plan_snapshots, run_plan_along};
    use crate::vocabulary::domain::Description;
    use crate::SnapshotBackend;
    use std::cell::RefCell;

    /// Says yes to the first `allowed` steps, then no, and remembers every question it was asked.
    struct Stopper {
        allowed: usize,
        asked: RefCell<Vec<(usize, usize, String)>>,
    }

    impl Stopper {
        fn after(allowed: usize) -> Self {
            Self {
                allowed,
                asked: RefCell::new(Vec::new()),
            }
        }
    }

    impl Course for Stopper {
        fn proceed(&self, done: usize, of: usize, area: &str) -> bool {
            self.asked.borrow_mut().push((done, of, area.to_owned()));
            done < self.allowed
        }
    }

    fn description() -> Description {
        Description::parse("test").unwrap()
    }

    #[test]
    fn a_change_is_asked_before_each_step_what_it_is_about_and_how_far_it_has_come() {
        let backend = InMemoryBackend::with_areas(&["home", "docs", "root"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        let course = Stopper::after(usize::MAX);
        let (report, stopped) = run_plan_along(&backend, &plan, &description(), None, &course);
        assert!(!stopped && report.outcomes.len() == 3);
        let expected: Vec<(usize, usize, String)> = [(0, "docs"), (1, "home"), (2, "root")]
            .into_iter()
            .map(|(done, area)| (done, 3, area.to_owned()))
            .collect();
        assert_eq!(
            *course.asked.borrow(),
            expected,
            "asked once per step, in order, with the step number and the total"
        );
    }

    #[test]
    fn a_stopped_snapshot_of_everything_leaves_the_taken_ones_and_never_starts_the_rest() {
        let backend = InMemoryBackend::with_areas(&["home", "docs"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        let (report, stopped) =
            run_plan_along(&backend, &plan, &description(), None, &Stopper::after(1));
        assert!(stopped, "it says it was stopped");
        assert_eq!(
            report.outcomes.len(),
            1,
            "only the step that was begun has an outcome"
        );
        let counts: Vec<usize> = plan
            .targets()
            .iter()
            .map(|config| backend.list_snapshots(config).unwrap().len())
            .collect();
        assert_eq!(
            counts,
            [1, 0],
            "the first area has its snapshot, the second was never begun"
        );
    }

    #[test]
    fn a_stopped_restore_keeps_the_areas_already_put_back_and_leaves_the_rest_exactly_as_they_are()
    {
        let backend = InMemoryBackend::with_areas(&["docs", "home"]);
        backend.write_file("docs", "/mnt/docs/a.txt", "before");
        backend.write_file("home", "/mnt/home/b.txt", "before");
        let plan = plan_snapshots(&backend, None).unwrap();
        let point = crate::vocabulary::domain::Point::parse("20261025T181500Z").unwrap();
        run_plan_along(&backend, &plan, &description(), Some(&point), &Unattended);
        backend.write_file("docs", "/mnt/docs/a.txt", "after");
        backend.write_file("home", "/mnt/home/b.txt", "after");

        let points = restore_points(&all_snapshots(&backend).unwrap());
        let safety = crate::vocabulary::domain::Point::parse("20261025T190000Z").unwrap();
        let done = restore_point_along(&backend, &points[0], safety, &Stopper::after(1));

        assert!(done.stopped);
        assert_eq!(done.members.len(), 1, "one area had its turn");
        assert!(matches!(done.members[0].result, MemberResult::Restored(_)));
        assert_eq!(
            backend.read_file("docs", "/mnt/docs/a.txt").as_deref(),
            Some("before"),
            "put back"
        );
        assert_eq!(
            backend.read_file("home", "/mnt/home/b.txt").as_deref(),
            Some("after"),
            "untouched, not half-restored"
        );
    }

    #[test]
    fn nobody_watching_means_every_step_goes_ahead() {
        let backend = InMemoryBackend::with_areas(&["home"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        let (report, stopped) = run_plan_along(&backend, &plan, &description(), None, &Unattended);
        assert!(!stopped && report.is_success());
    }
}
