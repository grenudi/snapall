# Restore a file from your file manager

Right-click a file, choose **Restore from snapshots**, and snapall opens on that file: every
snapshot, how the file in it compares with the file now (*same as now*, *differs from now*,
*deleted since*, *did not exist*), and Enter (or a click) puts the version you pick back. A snapshot of
the current state is taken first, so a restore can be undone by restoring *that*.

It opens in a terminal and asks for your password there, because snapper needs root and a terminal is
where `sudo` can ask. From a terminal yourself: `sudo snapall tui --file ~/notes.txt`, or just look
with `snapall history ~/notes.txt` (add `--json` for scripts).

## Which file managers

The packages install the entry where each file manager looks for it. Nothing is edited in your home
directory.

| File manager | Desktop | What you need | Source for where it goes |
|---|---|---|---|
| Dolphin and other KDE apps | KDE Plasma | nothing | [KDE: service menus](https://develop.kde.org/docs/apps/dolphin/service-menus/): `/usr/share/kio/servicemenus/` |
| Nemo | Cinnamon | nothing | Nemo's own sample action: `/usr/share/nemo/actions/` |
| PCManFM, PCManFM-Qt | LXDE, LXQt | nothing; restart PCManFM-Qt once | [libfm actions](https://wiki.archlinux.org/title/PCManFM): `/usr/share/file-manager/actions/` |
| Nautilus (Files) | GNOME | the Python bindings, then `nautilus -q` | [GNOME: nautilus-python](https://gnome.pages.gitlab.gnome.org/nautilus-python/nautilus-python-migrating-to-4.html): `/usr/share/nautilus-python/extensions/` |
| Thunar | Xfce | add one custom action (below) | Thunar documents only a per-user setup |

Hyprland, sway, i3 and the like are window managers, not file managers: they use whichever of the
above you run, so Thunar, Nautilus, Dolphin and Nemo are the ones to set up there.

### Nautilus (GNOME)
Nautilus needs the Python extension support, which is a separate package:

```sh
sudo apt install python3-nautilus      # Debian, Ubuntu, Mint
sudo dnf install nautilus-python       # Fedora
sudo pacman -S python-nautilus         # Arch, CachyOS, Manjaro
nautilus -q                            # then open Files again
```

The extension works with Nautilus 43 and later and with older versions.

### Thunar (Xfce)
Thunar keeps custom actions in your own configuration, and its documentation describes no system-wide
location, so this one is a few clicks (once):

1. In Thunar: **Edit → Configure custom actions…**, then **+**.
2. **Name:** `Restore from snapshots`. **Command:** `snapall-file-menu %f`
   (Thunar's documentation says never to quote the field codes).
3. **Appearance Conditions:** file pattern `*`, and tick everything except *Directories*.

## What it can and cannot do
- **Files, not folders.** A folder has no single version to show; restore a whole config from the
  terminal UI, or one folder with `snapall restore CONFIG ID /path`.
- **The file must be under a snapper config.** The config with the longest matching mount point is used
  (a file in `/home` belongs to `home`, not `root`). Otherwise snapall says no config covers it.
- **The system itself** is not restored this way: a single file of it is fine, the whole root is not
  ([why](adr/0012-restore-and-the-boot-menu.md)).
- **The terminal.** The launcher uses `xdg-terminal-exec` if you have it, then `$TERMINAL`, then a
  terminal it knows how to run a command in (konsole, gnome-terminal, ptyxis, xfce4-terminal,
  alacritty, kitty, foot, wezterm, xterm). Anything else gets a message saying so; installing
  `xdg-terminal-exec` fixes it for any terminal.

## Not tested in a real desktop
The entries are checked against what each file manager's documentation requires, the launcher is run
against stand-ins for every terminal it knows (including a file name full of quotes and `$`), and the
Nautilus extension against a stand-in for GNOME's API, all in CI. They have not been clicked in a
running Plasma, Cinnamon, LXQt, GNOME or Xfce session. The PCManFM location comes from a distribution
wiki rather than the file manager's own documentation. If one does not appear for you, that is a bug
worth reporting.

## Related
KDE announced [KIO-Snapshot](https://www.phoronix.com/news/KDE-Btrfs-Snapshots) 1.0 in August 2026, a
Dolphin integration for browsing previous versions of files in Btrfs snapshots. It is new and not
something snapall has tried; it reads snapshots, while snapall's entry restores from them with a safety
snapshot first, so the two can sit side by side.
