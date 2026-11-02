// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! The absolute path of a file a person named.

use std::error::Error;

/// The absolute path of what the user named. A relative path is taken from the working directory,
/// and symlinks in the *directory* part are resolved, so that a path reached through `/home` being
/// a link to `/var/home` is found in the area that really holds it. The file's own name is
/// kept as given: a symlink is itself what is asked about, and a deleted file has nothing to resolve.
pub(super) fn absolute(path: &str) -> Result<String, Box<dyn Error>> {
    let given = std::path::Path::new(path);
    let joined = if given.is_absolute() {
        given.to_owned()
    } else {
        std::env::current_dir()?.join(given)
    };
    let (Some(parent), Some(name)) = (joined.parent(), joined.file_name()) else {
        return Err(format!("`{path}` does not name a file").into());
    };
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("{}: {e}", parent.display()))?;
    parent
        .join(name)
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| "a path must be valid UTF-8".into())
}
