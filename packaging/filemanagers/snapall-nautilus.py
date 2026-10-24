# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""A "Restore from snapshots" entry in the Nautilus (GNOME Files) right-click menu.

Needs the Nautilus Python bindings: python3-nautilus (Debian, Ubuntu), nautilus-python (Fedora),
python-nautilus (Arch). Works with Nautilus 43 and later (API 4.0) and with older ones (API 3.0).
Installed in /usr/share/nautilus-python/extensions/; restart Nautilus once (`nautilus -q`).
"""

import shutil
import subprocess

import gi

try:
    gi.require_version("Nautilus", "4.0")
except ValueError:
    gi.require_version("Nautilus", "3.0")

from gi.repository import GObject, Nautilus  # noqa: E402

LAUNCHER = "snapall-file-menu"


class SnapallMenu(GObject.GObject, Nautilus.MenuProvider):
    def get_file_items(self, *args):
        # API 4.0 passes (files); API 3.0 passes (window, files).
        files = args[-1]
        if len(files) != 1:
            return []
        selected = files[0]
        if selected.get_uri_scheme() != "file" or selected.is_directory():
            return []
        path = selected.get_location().get_path()
        if not path or shutil.which(LAUNCHER) is None:
            return []
        item = Nautilus.MenuItem(
            name="Snapall::RestoreFromSnapshots",
            label="Restore from snapshots",
            tip="Show earlier versions of this file in snapshots and restore one",
        )
        item.connect("activate", self._open, path)
        return [item]

    @staticmethod
    def _open(_item, path):
        subprocess.Popen([LAUNCHER, path], start_new_session=True)
