// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Looking at a file as snapper's snapshots keep it, and comparing it with now.

use snapall_core::{BackendError, FileState};
use std::fs::{self, File};
use std::io;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;

/// The nearest ancestor of `path` that exists, which is where a file that is gone was kept.
pub(crate) fn nearest_existing(path: &Path) -> Option<&Path> {
    path.ancestors().find(|ancestor| ancestor.exists())
}

/// Whether the directory holding `path` is on the same subvolume as `subvolume`. Btrfs gives every
/// subvolume its own device number, so this is a comparison of those: a subvolume that is nested in
/// another, or mounted over a directory of it, is told apart from it. The directory is what is
/// compared (not the file) because that is where the file, a symlink or a deleted file lives.
pub(crate) fn same_subvolume(subvolume: &Path, path: &Path) -> Result<bool, BackendError> {
    // The subvolume's own root directory is the subvolume, whatever holds that directory.
    if path.components().eq(subvolume.components()) {
        return Ok(true);
    }
    let holder = path
        .parent()
        .and_then(nearest_existing)
        .unwrap_or(Path::new("/"));
    let theirs = fs::metadata(subvolume).map_err(|e| io_error(subvolume, &e))?;
    let ours = fs::metadata(holder).map_err(|e| io_error(holder, &e))?;
    Ok(theirs.dev() == ours.dev())
}

pub(crate) fn io_error(path: &Path, error: &io::Error) -> BackendError {
    BackendError::Unavailable {
        program: "filesystem".into(),
        detail: format!("{}: {error}", path.display()),
    }
}

/// Metadata of what is at `path` itself (not what a symlink points at), or `None` if nothing is.
pub(crate) fn lstat(path: &Path) -> Result<Option<fs::Metadata>, BackendError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        // A parent that is not a directory, or missing, means the file is not there either.
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(None)
        }
        Err(e) => Err(io_error(path, &e)),
    }
}

/// Whether two readers hold the same bytes.
pub(crate) fn same_bytes(a: &Path, b: &Path) -> Result<bool, BackendError> {
    let mut left = File::open(a).map_err(|e| io_error(a, &e))?;
    let mut right = File::open(b).map_err(|e| io_error(b, &e))?;
    let (mut x, mut y) = ([0u8; 64 * 1024], [0u8; 64 * 1024]);
    loop {
        let n = left.read(&mut x).map_err(|e| io_error(a, &e))?;
        let mut filled = 0;
        while filled < n {
            let got = right.read(&mut y[filled..n]).map_err(|e| io_error(b, &e))?;
            if got == 0 {
                return Ok(false); // the other ended first
            }
            filled += got;
        }
        if n == 0 {
            // `left` is exhausted: equal only if `right` is too
            return Ok(right.read(&mut y).map_err(|e| io_error(b, &e))? == 0);
        }
        if x[..n] != y[..n] {
            return Ok(false);
        }
    }
}

/// How the file as the snapshot had it (`then`) compares with the file now (`now`). Content, type,
/// permissions and owner all count, because restoring puts all of them back.
pub(crate) fn file_state(then: &Path, now: &Path) -> Result<FileState, BackendError> {
    let Some(before) = lstat(then)? else {
        return Ok(FileState::AbsentThen);
    };
    if before.is_dir() {
        return Err(BackendError::NotSupported(
            "directories are not supported yet: choose a file".into(),
        ));
    }
    let Some(after) = lstat(now)? else {
        return Ok(FileState::DeletedSince);
    };
    if after.is_dir() {
        return Err(BackendError::NotSupported(
            "directories are not supported yet: choose a file".into(),
        ));
    }
    let same_kind = before.file_type().is_symlink() == after.file_type().is_symlink();
    let same_owner = before.uid() == after.uid() && before.gid() == after.gid();
    let same_mode = before.permissions().mode() & 0o7777 == after.permissions().mode() & 0o7777;
    let same_content = if !same_kind {
        false
    } else if before.file_type().is_symlink() {
        fs::read_link(then).map_err(|e| io_error(then, &e))?
            == fs::read_link(now).map_err(|e| io_error(now, &e))?
    } else {
        before.len() == after.len() && same_bytes(then, now)?
    };
    Ok(
        if same_kind && same_content && same_owner && (same_mode || before.file_type().is_symlink())
        {
            FileState::Same
        } else {
            FileState::Changed
        },
    )
}
