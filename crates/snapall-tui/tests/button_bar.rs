// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The buttons look like buttons (a coloured block, brighter under the mouse, a flash when clicked), the tabs
//! with them, and the groups can be seen on the main screen.

use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::Terminal;
use snapall_core::ports::testing::{FakeSystem, InMemoryBackend};
use snapall_core::{plan_snapshots, run_plan, AreaName, Description, GroupName, Groups};
use snapall_tui::model::{Button, Click, Key, Modal, Msg};
use snapall_tui::{view, Session, Tab, UiContext};

const SCREEN: Rect = Rect {
    x: 0,
    y: 0,
    width: 80,
    height: 24,
};

fn draw(session: &Session<'_>) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height)).unwrap();
    terminal
        .draw(|frame| view::render(&session.state, frame))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn line(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}

/// The first 28 columns of row `y`: the left pane.
fn left(buffer: &Buffer, y: u16) -> String {
    (0..28).map(|x| buffer[(x, y)].symbol()).collect()
}

/// Where `text` starts on row `y`.
fn at(buffer: &Buffer, y: u16, text: &str) -> u16 {
    u16::try_from(
        line(buffer, y)
            .find(text)
            .unwrap_or_else(|| panic!("`{text}` not on row {y}: {}", line(buffer, y))),
    )
    .unwrap()
}

fn machine() -> InMemoryBackend {
    let backend = InMemoryBackend::with_areas(&["data", "home", "root"]).with_root_area("root");
    let plan = plan_snapshots(&backend, None).unwrap();
    run_plan(&backend, &plan, &Description::parse("one").unwrap(), None);
    backend
}

fn session<'a>(backend: &'a InMemoryBackend, system: &'a FakeSystem) -> Session<'a> {
    Session::with_system(
        backend,
        system,
        UiContext {
            administrator: true,
            ..UiContext::default()
        },
    )
}

fn theming() -> Groups {
    let mut groups = Groups::default();
    groups
        .set(
            GroupName::parse("theming").unwrap(),
            vec![
                AreaName::parse("home").unwrap(),
                AreaName::parse("data").unwrap(),
            ],
        )
        .unwrap();
    groups
}

#[test]
fn a_button_is_a_bright_key_beside_a_block_of_colour_and_a_dangerous_one_is_red() {
    let (backend, system) = (machine(), FakeSystem::new());
    let session = session(&backend, &system);
    let screen = draw(&session);
    let create = at(&screen, 22, "c create");
    assert_eq!(
        screen[(create, 22)].bg,
        Color::Yellow,
        "the key is a bright chip"
    );
    assert_eq!(
        screen[(create + 3, 22)].bg,
        Color::DarkGray,
        "the label is a block of colour"
    );
    let delete = at(&screen, 22, "d delete");
    assert_eq!(
        screen[(delete, 22)].bg,
        Color::Red,
        "what removes something has a red key"
    );
    assert_eq!(
        screen[(delete + 3, 22)].bg,
        Color::DarkGray,
        "but is not shouting at rest"
    );
}

#[test]
fn the_button_under_the_mouse_brightens_and_only_that_one() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = session(&backend, &system);
    session.dispatch(Msg::Hover(Some(Click::Button(Button::Create))));
    let screen = draw(&session);
    let create = at(&screen, 22, "c create");
    assert_eq!(
        (screen[(create, 22)].bg, screen[(create + 3, 22)].bg),
        (Color::White, Color::Blue)
    );
    let all = at(&screen, 22, "a all");
    assert_eq!(
        screen[(all + 3, 22)].bg,
        Color::DarkGray,
        "the others stay as they were"
    );
    // a dangerous one goes red under the mouse
    session.dispatch(Msg::Hover(Some(Click::Button(Button::Delete))));
    let screen = draw(&session);
    let delete = at(&screen, 22, "d delete");
    assert_eq!(screen[(delete + 3, 22)].bg, Color::Red);
    session.dispatch(Msg::Hover(None));
    assert_eq!(
        draw(&session)[(delete + 3, 22)].bg,
        Color::DarkGray,
        "and it goes back when the mouse leaves"
    );
}

#[test]
fn a_click_flashes_the_button_for_a_moment_and_then_it_is_as_before() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = session(&backend, &system);
    session.dispatch(Msg::Click(Click::Button(Button::Reload)));
    let screen = draw(&session);
    let reload = at(&screen, 23, "r reload");
    assert_eq!(
        (screen[(reload, 23)].bg, screen[(reload + 3, 23)].bg),
        (Color::Green, Color::Green),
        "pressed"
    );
    session.dispatch(Msg::Tick);
    session.dispatch(Msg::Tick);
    assert_eq!(
        draw(&session)[(reload + 3, 23)].bg,
        Color::Green,
        "still lit after two ticks"
    );
    session.dispatch(Msg::Tick);
    assert_eq!(
        draw(&session)[(reload + 3, 23)].bg,
        Color::DarkGray,
        "and back to rest after three"
    );
    assert!(session.state.pressed.is_none());
}

#[test]
fn the_window_keeps_ticking_while_a_button_is_flashing_so_the_flash_ends() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = session(&backend, &system);
    session.dispatch(Msg::Click(Click::Button(Button::Reload)));
    assert!(
        session.state.is_following_something(),
        "the clock must run, or the flash would never end"
    );
    for _ in 0..3 {
        session.dispatch(Msg::Tick);
    }
    assert!(!session.state.is_following_something());
}

#[test]
fn tabs_are_buttons_too_the_open_one_a_white_block_and_the_hovered_one_lit() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = session(&backend, &system);
    let screen = draw(&session);
    let open = at(&screen, 0, "1 Snapshots");
    let other = at(&screen, 0, "2 Health");
    assert_eq!(
        (screen[(open + 2, 0)].bg, screen[(other + 2, 0)].bg),
        (Color::White, Color::DarkGray)
    );
    session.dispatch(Msg::Hover(Some(Click::Tab(Tab::Health))));
    let screen = draw(&session);
    assert_eq!(screen[(other + 2, 0)].bg, Color::Blue);
    assert!(
        line(&screen, 21).contains("how much room it takes"),
        "and the line below says what the tab is for: {}",
        line(&screen, 21)
    );
}

#[test]
fn a_prompts_answer_is_in_its_colour_the_way_out_is_grey_and_both_light_under_the_mouse() {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = session(&backend, &system);
    session.press(Key::Char('c'));
    assert!(matches!(session.state.modal, Some(Modal::Describe { .. })));
    let screen = draw(&session);
    let ask = (0..24)
        .find(|y| line(&screen, *y).contains("[ Enter: Create ]"))
        .expect("the prompt's button");
    let x = at(&screen, ask, "[ Enter: Create ]");
    assert_eq!(
        screen[(x, ask)].bg,
        Color::Cyan,
        "the answer wears the prompt's colour"
    );
    let cancel = at(&screen, ask, "[ Esc: Cancel ]");
    assert_eq!(screen[(cancel, ask)].bg, Color::DarkGray);
    session.dispatch(Msg::Hover(Some(Click::Button(Button::Cancel))));
    assert_eq!(draw(&session)[(cancel, ask)].bg, Color::Blue);
}

#[test]
fn groups_can_be_seen_under_the_areas_and_a_click_on_them_opens_them() {
    let (backend, system) = (machine(), FakeSystem::new().with_groups(theming()));
    let mut session = session(&backend, &system);
    let screen = draw(&session);
    let pane: String = (1..21)
        .map(|y| left(&screen, y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(pane.contains("Groups  g: manage"), "{pane}");
    assert!(pane.contains("theming  home, data"), "{pane}");
    let y = (1..21)
        .find(|y| left(&screen, *y).contains("theming"))
        .unwrap();
    let click = view::hit_test(&session.state, SCREEN, 5, y);
    assert_eq!(click, Some(Click::Button(Button::Groups)));
    session.dispatch(Msg::Click(click.unwrap()));
    assert!(matches!(session.state.modal, Some(Modal::Groups(_))));
}

#[test]
fn with_no_groups_the_pane_says_how_to_make_one_so_the_feature_can_be_found() {
    let (backend, system) = (machine(), FakeSystem::new());
    let session = session(&backend, &system);
    let pane: String = (1..21)
        .map(|y| left(&draw(&session), y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        pane.contains("Groups") && pane.contains("none yet: press g"),
        "{pane}"
    );
}

#[test]
fn the_g_groups_button_is_on_the_bar_and_opens_them_and_the_areas_are_never_squeezed_for_the_groups(
) {
    let names: Vec<String> = (0..16).map(|i| format!("area{i:02}")).collect();
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let backend = InMemoryBackend::with_areas(&refs);
    let system = FakeSystem::new().with_groups(theming());
    let mut session = session(&backend, &system);
    let screen = draw(&session);
    let pane: String = (1..21)
        .map(|y| left(&screen, y))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !pane.contains("Groups"),
        "with this many areas there is no room, and the areas win:\n{pane}"
    );
    // the button is always there
    let button = at(&screen, 22, "g groups");
    let click = view::hit_test(&session.state, SCREEN, button + 2, 22);
    assert_eq!(click, Some(Click::Button(Button::Groups)));
    session.dispatch(Msg::Click(click.unwrap()));
    assert!(matches!(session.state.modal, Some(Modal::Groups(_))));
}

#[test]
fn buttons_that_act_on_the_snapshot_lists_are_dim_and_dead_in_the_other_tabs_and_the_rest_stay_alive(
) {
    let (backend, system) = (machine(), FakeSystem::new());
    let mut session = session(&backend, &system);
    session.press(Key::Char('2'));
    let screen = draw(&session);
    for (label, row) in [("c create", 22), ("d delete", 22), ("u restore", 22)] {
        let x = at(&screen, row, label);
        assert_eq!(
            screen[(x + 2, row)].bg,
            Color::Reset,
            "`{label}` has no block of colour here"
        );
        assert_eq!(screen[(x + 2, row)].fg, Color::DarkGray, "`{label}` is dim");
        assert_eq!(
            view::hit_test(&session.state, SCREEN, x + 2, row),
            None,
            "`{label}` does not answer a click here"
        );
    }
    // what does act everywhere still looks and works like a button
    for (label, row, button) in [
        ("a all", 22, Button::CreateAll),
        ("g groups", 22, Button::Groups),
        ("r reload", 23, Button::Reload),
        ("q quit", 23, Button::Quit),
    ] {
        let x = at(&screen, row, label);
        assert_eq!(
            screen[(x + 2, row)].bg,
            Color::DarkGray,
            "`{label}` is alive"
        );
        assert_eq!(
            view::hit_test(&session.state, SCREEN, x + 2, row),
            Some(Click::Button(button))
        );
    }
    // and back on the snapshots they all are
    session.press(Key::Char('1'));
    let screen = draw(&session);
    let x = at(&screen, 22, "c create");
    assert_eq!(screen[(x + 3, 22)].bg, Color::DarkGray);
    assert_eq!(
        view::hit_test(&session.state, SCREEN, x + 2, 22),
        Some(Click::Button(Button::Create))
    );
}
