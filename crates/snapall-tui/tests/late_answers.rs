// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! An answer belongs to the question that asked it. The window looks things up in the background, so an answer can
//! arrive after its question was closed, or after the person has moved on to another: it must go nowhere, and must
//! never show up in the next question.

use snapall_core::ports::testing::InMemoryBackend;
use snapall_core::{plan_snapshots, run_plan, Description, Point};
use snapall_tui::model::{Key, Modal, Msg};
use snapall_tui::Session;

/// docs, home and the system, two actions a moment apart, so there are two restore points.
fn two_points() -> InMemoryBackend {
    let backend = InMemoryBackend::with_areas(&["docs", "home", "root"]).with_root_area("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    for (words, point) in [
        ("first moment", "20261025T100000Z"),
        ("second moment", "20261025T110000Z"),
    ] {
        backend.write_file("docs", "/mnt/docs/a.txt", words);
        backend.write_file("home", "/mnt/home/b.txt", words);
        run_plan(
            &backend,
            &plan,
            &Description::parse(words).unwrap(),
            Some(&Point::parse(point).unwrap()),
        );
    }
    backend.write_file("docs", "/mnt/docs/a.txt", "ruined");
    backend.write_file("home", "/mnt/home/b.txt", "ruined");
    backend
}

fn open_points(backend: &InMemoryBackend) -> Session<'_> {
    let mut session = Session::new(backend);
    session.press(Key::Char('R'));
    session
}

#[test]
fn restore_points_that_arrive_after_the_screen_was_closed_go_nowhere() {
    let backend = two_points();
    let mut session = open_points(&backend);
    let Some(Modal::Points {
        points: Some(points),
        ..
    }) = session.state.modal.clone()
    else {
        panic!("the points screen did not open with its points");
    };
    session.press(Key::Esc);
    assert!(session.state.modal.is_none());

    session.dispatch(Msg::PointsLoaded(Ok(points)));

    assert!(
        session.state.modal.is_none(),
        "late points must not reopen or fill anything"
    );
    assert_eq!(
        session.state.pending, 0,
        "late points must not start a look at the first one"
    );
}

#[test]
fn a_preview_for_the_point_the_cursor_has_left_is_not_shown_for_the_one_it_is_on() {
    let backend = two_points();
    let mut session = open_points(&backend);
    let Some(Modal::Points {
        preview: Some(first),
        ..
    }) = session.state.modal.clone()
    else {
        panic!("the first point has no preview");
    };
    session.press(Key::Down);

    session.dispatch(Msg::PointPreviewLoaded(first.clone()));

    let Some(Modal::Points {
        points: Some(points),
        index,
        preview,
        ..
    }) = &session.state.modal
    else {
        panic!("the points screen closed");
    };
    let chosen = &points[*index];
    assert_ne!(
        chosen.point, first.point.point,
        "the cursor is on the other point"
    );
    assert!(
        preview
            .as_ref()
            .is_none_or(|shown| shown.point.point == chosen.point),
        "the preview shown is for a point the cursor is not on"
    );
}

#[test]
fn an_offer_for_a_snapshot_the_person_has_moved_on_from_does_not_replace_the_open_questions_own() {
    let backend = two_points();
    let mut session = Session::new(&backend);
    session.press(Key::Right);
    session.press(Key::Char('u'));
    let Some(Modal::Restore {
        area,
        id: newest,
        offer: Some(offer_of_newest),
        ..
    }) = session.state.modal.clone()
    else {
        panic!("the restore question did not open with the offer of its restore point");
    };
    session.press(Key::Esc);
    session.press(Key::Down);
    session.press(Key::Char('u'));
    let Some(Modal::Restore {
        id: older,
        offer: Some(own),
        ..
    }) = session.state.modal.clone()
    else {
        panic!("the question for the older snapshot did not open with its offer");
    };
    assert_ne!(newest, older);
    assert_ne!(
        offer_of_newest.point.point, own.point.point,
        "the two snapshots belong to different points"
    );

    // the answer for the snapshot the person left arrives late
    session.dispatch(Msg::PointOfferLoaded {
        area,
        id: newest,
        offer: Some(offer_of_newest),
    });

    let Some(Modal::Restore {
        offer: Some(shown), ..
    }) = &session.state.modal
    else {
        panic!("the question closed");
    };
    assert_eq!(
        shown.point.point, own.point.point,
        "a late answer for another snapshot replaced this one's"
    );
}
