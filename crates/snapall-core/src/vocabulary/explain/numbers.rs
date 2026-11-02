// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Saying how many: a number with its noun, in the right plural.

/// "1 snapshot" or "3 snapshots": the noun is given in the singular, and takes an `s` unless there is exactly one.
pub fn counted(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_is_singular_and_everything_else_is_plural() {
        assert_eq!(counted(1, "snapshot"), "1 snapshot");
        assert_eq!(counted(0, "snapshot"), "0 snapshots");
        assert_eq!(counted(2, "area"), "2 areas");
        assert_eq!(counted(12, "path"), "12 paths");
    }
}
