// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a restore of every area says about itself: how many areas were put back and where the way back is, why it failed,
//! or how far it got before it was stopped.

use super::putting_back::{MemberResult, PointRestore};

/// What to say when a restore was stopped between areas: to the person now, and for the record.
pub struct StoppedWords {
    pub now: String,
    pub record: String,
}

impl PointRestore {
    /// How it went, for the record: what came of it, or why it failed.
    pub fn how_it_went(&self) -> Result<String, String> {
        if !self.succeeded() {
            return Err("some areas could not be put back: the output says which".to_owned());
        }
        if !self.changed_anything() {
            return Ok(String::new());
        }
        let put_back = self
            .members
            .iter()
            .filter(|member| matches!(member.result, MemberResult::Restored(_)))
            .count();
        Ok(format!(
            "{put_back} area{} put back. To undo it: restore point {}",
            if put_back == 1 { "" } else { "s" },
            self.safety.as_str()
        ))
    }

    /// What to say when it was stopped before every area of a point of `total` areas was put back.
    pub fn stopped_words(&self, total: usize) -> StoppedWords {
        let put_back = self.members.len();
        StoppedWords {
            now: format!("snapall: stopped on request after {put_back} of {total} areas; the rest were left as they are"),
            record: format!(
                "stopped on request after {put_back} of {total} areas: the others were left as they are. To undo what was done: restore point {}",
                self.safety.as_str()
            ),
        }
    }
}
