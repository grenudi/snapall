// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Restore points: listing them, what putting one back would do, and what it did.

use serde_json::{json, Value};

pub(super) fn areas_of(point: &snapall_core::RestorePoint) -> String {
    point
        .members
        .iter()
        .map(|m| m.area.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The restore points, newest first, each with what to type to go back to it.
pub fn points(points: &[snapall_core::RestorePoint]) -> String {
    if points.is_empty() {
        return "No restore points yet. `sudo snapall` takes a snapshot of every area at once, and that is a restore point.\n".to_owned();
    }
    let mut out = String::from(
        "Restore points: the snapshots that one action took together, one in every area.\nPut every area back to one with `sudo snapall restore-all POINT`.\n\n",
    );
    let width =
        |f: &dyn Fn(&snapall_core::RestorePoint) -> usize| points.iter().map(f).max().unwrap_or(0);
    let w_point = width(&|p| p.point.as_str().len()).max("POINT".len());
    let w_date = width(&|p| p.date.as_deref().unwrap_or("").len()).max("WHEN".len());
    let w_areas = width(&|p| areas_of(p).len()).max("AREAS".len());
    out.push_str(&format!(
        "{:<w_point$}  {:<w_date$}  {:<w_areas$}  NAME\n",
        "POINT", "WHEN", "AREAS"
    ));
    for p in points {
        let note = if p.tagged {
            ""
        } else {
            "   (older: matched by name and time)"
        };
        out.push_str(&format!(
            "{:<w_point$}  {:<w_date$}  {:<w_areas$}  {}{note}\n",
            p.point.as_str(),
            p.date.as_deref().unwrap_or(""),
            areas_of(p),
            p.description
        ));
    }
    out
}

pub fn points_json(points: &[snapall_core::RestorePoint]) -> Value {
    json!({ "points": points.iter().map(|p| json!({
        "point": p.point.as_str(),
        "description": p.description,
        "date": p.date,
        "tagged": p.tagged,
        "areas": p.members.iter().map(|m| json!({"area": m.area.as_str(), "snapshot": m.id.get()})).collect::<Vec<_>>(),
    })).collect::<Vec<_>>() })
}

pub(super) fn path_counts(changes: &[snapall_core::Change]) -> (usize, usize, usize) {
    use snapall_core::ChangeKind;
    let count = |kind: ChangeKind| changes.iter().filter(|c| c.kind == kind).count();
    (
        count(ChangeKind::Changed),
        count(ChangeKind::Added),
        count(ChangeKind::Removed),
    )
}

/// What putting every area of a point back would do, area by area, and why the system is left out.
pub fn point_preview(
    point: &snapall_core::RestorePoint,
    previews: &[snapall_core::MemberPreview],
    system_route: &str,
) -> String {
    use snapall_core::MemberView;
    let mut out = format!(
        "Restore point {} ({}), taken {}. Putting every area back would:\n\n",
        point.point,
        point.description,
        point.date.as_deref().unwrap_or("at an unknown time")
    );
    let width = previews
        .iter()
        .map(|p| p.area.as_str().len())
        .max()
        .unwrap_or(0);
    let mut system = false;
    for p in previews {
        let what = match &p.view {
            MemberView::Changes(c) if c.changes.is_empty() => {
                "nothing to change: it already matches".to_owned()
            }
            MemberView::Changes(c) => {
                let (changed, added, removed) = path_counts(&c.changes);
                format!(
                    "change {} path{}: {changed} reverted, {added} removed, {removed} brought back",
                    c.changes.len(),
                    if c.changes.len() == 1 { "" } else { "s" }
                )
            }
            MemberView::SystemRoot => {
                system = true;
                format!(
                    "not touched: the running system is not put back while it runs (its snapshot in this point is #{})",
                    p.id
                )
            }
            MemberView::Failed(why) => format!("could not be looked at: {why}"),
        };
        out.push_str(&format!("  {:<width$}  {what}\n", p.area.as_str()));
    }
    if system {
        out.push_str(&format!(
            "\nThe system itself: {system_route}\n\nTo bring the system back as well, do that first, then run restore-all with this point again: the system will already match, and the other areas follow.\n"
        ));
    }
    out
}

pub fn point_preview_json(
    point: &snapall_core::RestorePoint,
    previews: &[snapall_core::MemberPreview],
) -> Value {
    use snapall_core::MemberView;
    json!({
        "point": point.point.as_str(),
        "areas": previews.iter().map(|p| match &p.view {
            MemberView::Changes(c) => json!({"area": p.area.as_str(), "snapshot": p.id.get(), "action": if c.changes.is_empty() { "unchanged" } else { "change" }, "paths": c.changes.len()}),
            MemberView::SystemRoot => json!({"area": p.area.as_str(), "snapshot": p.id.get(), "action": "system"}),
            MemberView::Failed(why) => json!({"area": p.area.as_str(), "snapshot": p.id.get(), "action": "error", "error": why}),
        }).collect::<Vec<_>>(),
    })
}

/// What putting a point back did, and the one command that undoes all of it.
pub fn point_restored(
    point: &snapall_core::RestorePoint,
    done: &snapall_core::PointRestore,
) -> String {
    use snapall_core::MemberResult;
    let mut out = format!("Restore point {} ({}):\n\n", point.point, point.description);
    let width = done
        .members
        .iter()
        .map(|m| m.area.as_str().len())
        .max()
        .unwrap_or(0);
    for m in &done.members {
        let what = match &m.result {
            MemberResult::Restored(r) => format!(
                "put back ({} path{}); the state before is snapshot #{}",
                r.summary.created + r.summary.modified + r.summary.deleted,
                if r.summary.created + r.summary.modified + r.summary.deleted == 1 {
                    ""
                } else {
                    "s"
                },
                r.safety_snapshot
            ),
            MemberResult::AlreadyThere => "already matched: left alone".to_owned(),
            MemberResult::SystemRoot => {
                format!(
                "left alone: the running system is not put back while it runs (its snapshot in this point is #{})",
                m.id
            )
            }
            MemberResult::Failed(why) => format!("FAILED: {why}"),
        };
        out.push_str(&format!("  {:<width$}  {what}\n", m.area.as_str()));
    }
    if done.changed_anything() {
        out.push_str(&format!(
            "\nTo undo all of this at once: sudo snapall restore-all {}\n",
            done.safety
        ));
    } else {
        out.push_str("\nNothing was changed.\n");
    }
    out
}

pub fn point_restored_json(
    point: &snapall_core::RestorePoint,
    done: &snapall_core::PointRestore,
) -> Value {
    use snapall_core::MemberResult;
    json!({
        "point": point.point.as_str(),
        "undo_point": done.changed_anything().then(|| done.safety.as_str().to_owned()),
        "areas": done.members.iter().map(|m| match &m.result {
            MemberResult::Restored(r) => json!({"area": m.area.as_str(), "result": "restored", "snapshot": m.id.get(), "safety_snapshot": r.safety_snapshot.get(), "paths": r.summary.created + r.summary.modified + r.summary.deleted}),
            MemberResult::AlreadyThere => json!({"area": m.area.as_str(), "result": "unchanged", "snapshot": m.id.get()}),
            MemberResult::SystemRoot => json!({"area": m.area.as_str(), "result": "system", "snapshot": m.id.get()}),
            MemberResult::Failed(why) => json!({"area": m.area.as_str(), "result": "failed", "snapshot": m.id.get(), "error": why}),
        }).collect::<Vec<_>>(),
    })
}
