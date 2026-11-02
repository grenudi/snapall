// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Reading the mount table: each place a disk is attached.

/// A filesystem mounted at a directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mount {
    pub point: String,
    pub fstype: String,
    /// The device (or other source) it is mounted from.
    pub source: String,
    /// Which part of that filesystem is mounted: `/@home` for the subvolume `@home`.
    pub root: String,
}

/// `\040` and friends: mountinfo writes a space, tab, newline or backslash in a path as an octal
/// escape.
pub(super) fn unescape(field: &str) -> String {
    let mut out = String::new();
    let bytes = field.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let octal = (bytes[i] == b'\\' && i + 3 < bytes.len())
            .then(|| field.get(i + 1..i + 4))
            .flatten()
            .and_then(|digits| u8::from_str_radix(digits, 8).ok());
        match octal {
            Some(byte) => {
                out.push(char::from(byte));
                i += 4;
            }
            None => {
                out.push(char::from(bytes[i]));
                i += 1;
            }
        }
    }
    out
}

/// The mounts in the text of `/proc/self/mountinfo`. Lines that are not in the documented shape
/// (`id parent dev root mount-point options ... - fstype source options`) are skipped.
pub fn mounts_from_mountinfo(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|line| {
            let (before, after) = line.split_once(" - ")?;
            let mut head = before.split_whitespace().skip(3);
            let root = unescape(head.next()?);
            let point = unescape(head.next()?);
            let mut tail = after.split_whitespace();
            let fstype = tail.next()?.to_owned();
            let source = tail.next().unwrap_or_default().to_owned();
            Some(Mount {
                point,
                fstype,
                source,
                root,
            })
        })
        .collect()
}
