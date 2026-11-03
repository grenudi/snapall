// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! What putting files back does to paths, in words: only what happens, never a row of zeros.

/// `1 removed`, `2 reverted, 1 brought back`, or `no change`: the parts that are not none, in the order a person reads them.
pub fn what_changes<N>(reverted: N, removed: N, brought_back: N) -> String
where
    N: Copy + PartialEq + From<u8> + std::fmt::Display,
{
    let parts: Vec<String> = [
        (reverted, "reverted"),
        (removed, "removed"),
        (brought_back, "brought back"),
    ]
    .into_iter()
    .filter(|(count, _)| *count != N::from(0))
    .map(|(count, what)| format!("{count} {what}"))
    .collect();
    if parts.is_empty() {
        "no change".to_owned()
    } else {
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_happens_is_said_and_nothing_is_said_as_no_change() {
        assert_eq!(what_changes(0_usize, 1, 0), "1 removed");
        assert_eq!(what_changes(2_usize, 0, 1), "2 reverted, 1 brought back");
        assert_eq!(
            what_changes(3_u64, 1, 2),
            "3 reverted, 1 removed, 2 brought back"
        );
        assert_eq!(what_changes(0_u64, 0, 0), "no change");
    }
}
