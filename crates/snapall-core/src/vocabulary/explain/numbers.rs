// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Saying how many: a number with its noun, in the right plural.

/// "1 snapshot" or "3 snapshots": the noun is given in the singular, and takes an `s` unless there is exactly one.
pub fn counted<N>(count: N, noun: &str) -> String
where
    N: PartialEq + From<u8> + std::fmt::Display,
{
    if count == N::from(1) {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// The word that agrees with a number that is not itself shown: `by_number(n, "includes", "include")`.
pub fn by_number<'a, N>(count: N, one: &'a str, many: &'a str) -> &'a str
where
    N: PartialEq + From<u8>,
{
    if count == N::from(1) {
        one
    } else {
        many
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

    #[test]
    fn a_word_agrees_with_a_number_that_is_not_shown() {
        assert_eq!(by_number(1_usize, "includes", "include"), "includes");
        assert_eq!(by_number(0_usize, "includes", "include"), "include");
        assert_eq!(by_number(3_u64, "it", "them"), "them");
    }

    #[test]
    fn a_count_of_any_whole_number_type_is_said_the_same() {
        assert_eq!(counted(1_u64, "day"), "1 day");
        assert_eq!(counted(90_u32, "minute"), "90 minutes");
        assert_eq!(counted(0_usize, "path"), "0 paths");
    }
}
