// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The file history screen: one file's versions, newest first, and putting one back. It is read in one place: what its
//! keys do, what happens when its versions arrive, and how it is drawn.

mod arriving;
pub(crate) mod drawing;
mod keys;

pub(crate) use arriving::on_file_versions_loaded;
pub(crate) use keys::on_file_key;
