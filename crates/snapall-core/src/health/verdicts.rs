// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What judging produces: levels, topics, remedies, findings, chips and the verdict.

/// How much a finding matters, least first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Good,
    /// Worth knowing; nothing is wrong.
    Note,
    /// Something to do.
    Attention,
    /// Something to do now.
    Problem,
}

/// Which part of the Health screen a finding belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Space,
    Cleanup,
    BootMenu,
    Snapshots,
    /// Changes that are running now: not a finding, but an item of the status bar like the others.
    Jobs,
}

/// Something snapall can do about a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    SwitchOnCleanup,
    /// Switch on counting the room snapshots take.
    CountRoom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub topic: Topic,
    pub headline: String,
    /// What it is, why it matters and what to do, in words that assume nothing.
    pub why: String,
    pub remedy: Option<Remedy>,
}

/// One item of the status bar: what it says, how it stands, and where a click goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub label: String,
    pub level: Level,
    pub topic: Topic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub level: Level,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    pub verdict: Verdict,
    /// Most urgent first.
    pub findings: Vec<Finding>,
    pub chips: Vec<Chip>,
}
