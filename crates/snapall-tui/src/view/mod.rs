// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Rendering: `render(state, frame)` draws, and does nothing else.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::model::{Button, Click, Modal, Notice, Pane, State};

use crate::tab::Tab;
use crate::theme::{ACCENT, DIM};
use crate::{status_bar, tabs};

mod footer;
mod header;
mod hit;
pub(crate) mod look;

pub(crate) use crate::file_history::drawing::render_file;
pub(crate) use crate::questions::drawing::*;
pub(crate) use crate::tabs::snapshots::{areas_split, pane_block, render_areas, render_snapshots};
pub use footer::*;
pub use header::*;
pub use hit::*;

/// Where everything is, for a terminal of this size. Drawing and clicking both start here, so a
/// click can only ever land on what was drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Areas {
    /// The tab bar.
    pub header: Rect,
    /// Everything between the tab bar and the status bar.
    pub body: Rect,
    pub area_list: Rect,
    pub snapshots: Rect,
    /// The status bar, under every view.
    pub status: Rect,
    pub notice: Rect,
    pub keys: Rect,
}

pub fn areas(size: Rect) -> Areas {
    let [header, body, status, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(3),
    ])
    .areas(size);
    let [area_list, snapshots] =
        Layout::horizontal([Constraint::Length(28), Constraint::Min(20)]).areas(body);
    let [notice, keys] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(2)]).areas(footer);
    Areas {
        header,
        body,
        area_list,
        snapshots,
        status,
        notice,
        keys,
    }
}

/// [`areas`], adjusted for what is on screen: when the UI is open on one file, its versions take the
/// whole body and there is no areas pane.
pub fn layout(state: &State, size: Rect) -> Areas {
    let mut areas = areas(size);
    if state.file.is_some() {
        areas.snapshots = Rect {
            x: areas.area_list.x,
            y: areas.area_list.y,
            width: areas.area_list.width.saturating_add(areas.snapshots.width),
            height: areas.area_list.height,
        };
        areas.area_list = Rect {
            width: 0,
            height: 0,
            ..areas.area_list
        };
    }
    areas
}

pub fn render(state: &State, frame: &mut Frame) {
    let areas = layout(state, frame.area());
    render_header(state, frame, areas.header);
    match (state.file.is_some(), state.tab) {
        (true, _) => render_file(state, frame, areas.snapshots),
        (false, Tab::Snapshots) => {
            render_areas(state, frame, areas.area_list);
            render_snapshots(state, frame, areas.snapshots);
        }
        (false, Tab::Health) => tabs::health::render(state, frame, areas.body),
        (false, Tab::Activity) => tabs::activity::render(state, frame, areas.body),
    }
    status_bar::render(state, frame, areas.status);
    render_footer(state, frame, &areas);

    if let Some(modal) = &state.modal {
        render_modal(modal, state, frame);
    }
}
