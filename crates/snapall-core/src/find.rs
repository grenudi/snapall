// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Finding what a person points at: one snapshot of an area, by its number.

use crate::domain::{AreaName, SnapshotId, SnapshotInfo};
use crate::port::SnapshotBackend;
use crate::usecase::{snapshots_of, SnapshotError};

pub fn find_snapshot(
    backend: &dyn SnapshotBackend,
    area: &AreaName,
    id: SnapshotId,
) -> Result<Option<SnapshotInfo>, SnapshotError> {
    Ok(snapshots_of(backend, area)?
        .into_iter()
        .find(|snapshot| snapshot.id == id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Description;
    use crate::testing::InMemoryBackend;
    use crate::usecase::{plan_snapshots, run_plan};

    #[test]
    fn a_snapshot_is_found_by_its_number_in_its_own_area_and_nowhere_else() {
        let backend = InMemoryBackend::with_areas(&["data", "home"]);
        run_plan(
            &backend,
            &plan_snapshots(&backend, None).unwrap(),
            &Description::parse("x").unwrap(),
            None,
        );
        let home = AreaName::parse("home").unwrap();
        let its_id = snapshots_of(&backend, &home).unwrap()[0].id;
        assert_eq!(
            find_snapshot(&backend, &home, its_id)
                .unwrap()
                .map(|s| s.id),
            Some(its_id)
        );
        assert!(
            find_snapshot(&backend, &home, SnapshotId::new(999).unwrap())
                .unwrap()
                .is_none(),
            "a number nobody has is not found"
        );
        let nowhere = AreaName::parse("nowhere").unwrap();
        assert!(
            find_snapshot(&backend, &nowhere, its_id).is_err(),
            "an area that is not protected is an error, not an empty answer"
        );
    }
}
