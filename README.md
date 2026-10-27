# snapall

**Try risky things on Linux, and go back if they go wrong.**

Before an Arch update, a new desktop theme or a config you are not sure about, add a restore point in one command, try the thing, and put your files back if you do not like it. snapall takes the snapshot of everything you have set up (system and home), lists them in a friendly terminal UI you can drive with the mouse, restores files from them, and deletes the ones you no longer need, so they do not pile up. It is a plug-and-play front end for [snapper](http://snapper.io/) on Btrfs: no configuration of its own, nothing to learn first.

- **One command, everything.** `sudo snapall "before theming"` snapshots every protected area at once (the System, your Home files, and anything else you protect).
- **A real interface.** `sudo snapall tui`, or the *snapall* icon in your application menu (it opens a terminal and asks for your password). Mouse or keyboard.
- **Go back.** Restore a file, a folder or your whole home from any snapshot; the state before is snapshotted first, so a restore can be undone. To go back with the *whole system*, boot an earlier snapshot from the boot menu: snapall checks whether your machine has that (Limine or GRUB) and tells you how to get it if not.
- **Honest about itself.** Snapshots live on the same disk: this is a safety net for experiments, not a backup. `snapall doctor` tells you what is missing before anything goes wrong.

```console
$ sudo snapall "before theming"            # shorthand for `snapall create "before theming"`
ok    home  #14
ok    root  #212

$ snapall list
home
root

$ snapall snapshots home
home
   #  type    date                 cleanup  description
  13  single  2026-10-22 18:04:11  number   after update
  14  single  2026-10-23 01:26:30  number   before theming

$ sudo snapall tui
```

## Install and upgrade

There are two ways. **Add the package repository** (below, the first block for your system): your package manager then installs snapall and updates it along with everything else, the way you are used to. Or **paste a download block** (the second one): it fetches the newest release by hand, and the same lines install and upgrade. Either way snapall drives [snapper](http://snapper.io/), so every package depends on it and installing snapall installs snapper too. The blocks are written for bash or zsh and need `curl`. Packages are attached to each [release](https://github.com/grenudi/snapall/releases) and are not signed yet; every release carries signed build provenance, see [docs/verifying.md](docs/verifying.md).

### Debian, Ubuntu, Mint, Pop!_OS, ...

As a repository, so that `sudo apt upgrade` keeps it current:

<!-- snippet:repo-deb -->
```sh
command -v curl >/dev/null || sudo apt install -y curl
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://grenudi.github.io/snapall/key.asc | sudo tee /etc/apt/keyrings/snapall.asc >/dev/null
echo "deb [signed-by=/etc/apt/keyrings/snapall.asc] https://grenudi.github.io/snapall/apt ./" | sudo tee /etc/apt/sources.list.d/snapall.list >/dev/null
sudo apt update && sudo apt install snapall
```

Or as a one-off download (the same lines install and upgrade):

<!-- snippet:deb -->
```sh
command -v curl >/dev/null || sudo apt install -y curl
cd /tmp
url=$(curl -s https://api.github.com/repos/grenudi/snapall/releases/latest | grep -o "https://[^\"]*_$(dpkg --print-architecture)\.deb")
curl -LO "$url" && sudo apt install "./$(basename "$url")"
```

### Fedora

As a repository, so that `sudo dnf upgrade` keeps it current:

<!-- snippet:repo-dnf -->
```sh
sudo curl -fsSLo /etc/yum.repos.d/snapall.repo https://grenudi.github.io/snapall/snapall.repo
sudo dnf install snapall
```

dnf shows the signing key's fingerprint and asks before importing it: compare it with the one [below](#the-package-repositories). Or as a one-off download:

<!-- snippet:rpm-dnf -->
```sh
cd /tmp
url=$(curl -s https://api.github.com/repos/grenudi/snapall/releases/latest | grep -o "https://[^\"]*\.$(uname -m)\.rpm")
curl -LO "$url" && sudo dnf install "./$(basename "$url")"
```

### openSUSE

As a repository, so that `sudo zypper update` keeps it current:

<!-- snippet:repo-zypper -->
```sh
sudo zypper addrepo --refresh https://grenudi.github.io/snapall/snapall.repo
sudo zypper install snapall
```

zypper shows the signing key and asks whether to trust it: compare the fingerprint with the one [below](#the-package-repositories). Or as a one-off download:

<!-- snippet:rpm-zypper -->
```sh
cd /tmp
url=$(curl -s https://api.github.com/repos/grenudi/snapall/releases/latest | grep -o "https://[^\"]*\.$(uname -m)\.rpm")
curl -LO "$url" && sudo zypper install --allow-unsigned-rpm "./$(basename "$url")"
```

### Arch, CachyOS, Manjaro, EndeavourOS, ...

As a repository, so that `sudo pacman -Syu` keeps it current (x86_64):

<!-- snippet:repo-pacman -->
```sh
curl -fsSL https://grenudi.github.io/snapall/key.asc | sudo pacman-key --add -
sudo pacman-key --lsign-key 79B4B6BD57666623EAD400B20AE79BC1E50DDD5A
grep -q '^\[snapall\]' /etc/pacman.conf || printf '\n[snapall]\nSigLevel = Required\nServer = https://grenudi.github.io/snapall/arch/$arch\n' | sudo tee -a /etc/pacman.conf >/dev/null
sudo pacman -Syu snapall
```

Or build it from the release's `PKGBUILD` (the same lines install and upgrade):

<!-- snippet:arch -->
```sh
rm -rf ~/.cache/snapall-build && mkdir -p ~/.cache/snapall-build && cd ~/.cache/snapall-build
curl -LO https://github.com/grenudi/snapall/releases/latest/download/PKGBUILD
makepkg -si
```

It builds from the release's source in a fresh directory every time, which is the point: `makepkg` reads only a file named exactly `PKGBUILD` and reuses any `src/` or built package it finds beside it, so building in a directory that still holds an older version can quietly reinstall that older version. Look at `pkgver` in the file if you are ever unsure.

### Alpine, Void, Gentoo, and anything else

<!-- snippet:tarball -->
```sh
cd "$(mktemp -d)"
url=$(curl -s https://api.github.com/repos/grenudi/snapall/releases/latest | grep -o "https://[^\"]*$(uname -m)-unknown-linux-musl\.tar\.gz")
curl -LO "$url" && tar xzf "$(basename "$url")" && cd snapall-v*/ && sudo ./install.sh
```

The tarball carries its own installer, so this snippet does not change when a release adds files: it installs the binary, the man page, completions, the application-menu entry with its icon and the file-manager entries under `/usr/local`. `sudo ./install.sh --uninstall` removes them again; `PREFIX=/usr` and `DESTDIR=...` are honoured. It is a static binary and does not install snapper: `sudo snapall doctor --install` does that where it can, and otherwise prints what to run on your system.

### The package repositories

The repositories at <https://grenudi.github.io/snapall/> hold the release's own packages, unchanged, and their metadata is signed with this key:

```text
snapall package repository <17498075+grenudi@users.noreply.github.com>
79B4 B6BD 5766 6623 EAD4 00B2 0AE7 9BC1 E50D DD5A
```

Check it against what your package manager shows before you trust it. [docs/repositories.md](docs/repositories.md) says what is signed, how long versions are kept, and how the key would be replaced.

### Nix, NixOS

<!-- snippet:nix -->
```sh
nix profile install github:grenudi/snapall   # install
nix profile upgrade snapall                  # upgrade later
```

The flake wraps snapper into the program's `PATH`. A program in your *user* profile is not on root's `PATH`, so run it as `sudo "$(command -v snapall)" tui`. On NixOS, putting the package in `environment.systemPackages` avoids that, and it is what makes the application-menu entry (which runs `sudo snapall tui`) work.

`snapall doctor --install` installs the one package snapall needs through the system's own package manager (`apt-get`, `dnf`, `pacman`, `zypper`, `apk`), showing the commands and asking first. Those are the systems it runs commands on because those are the systems CI installs on, for real, on every push. Everywhere else it prints what to run instead of running it: RHEL and its rebuilds (snapper is only in EPEL), Alpine releases other than `edge` (snapper is only in `edge`'s `testing`), Void and Gentoo (`xbps-install -S snapper`, `emerge --ask app-backup/snapper`), NixOS (declarative), and Fedora Atomic, MicroOS and SteamOS (read-only root).

## Commands

| Command | What it does |
|---|---|
| `snapall [create] [DESCRIPTION]` | One snapshot of every protected area (the default). `--only root,home` restricts it, `--dry-run` previews it. |
| `snapall list` | The protected areas snapall can see, i.e. exactly what `create` would touch. |
| `snapall health` | Whether protection is working: a verdict first, then what to do about anything that needs it (cleanup off, a disk nearly full, new snapshots not reaching the boot menu), then the numbers: snapshots per area, the room they hold (when measurable), how full the disk is, and which background jobs run. `--json` for scripts; exits 1 only for what needs doing now. |
| `snapall quotas [on\|off]` | How the disk counts the room snapshots take, what that gives and costs, and the switch for it. Switching changes how the whole disk works, so it explains and asks first (`--yes` to skip). |
| `snapall activity` | What snapall has done, newest first: from the window and from commands, in this and earlier sessions, and how each went. `--json` for scripts. It is kept in a small journal under `/var/lib/snapall`. |
| `snapall unprotect AREA [--dry-run] [--destroy]` | Stop protecting an area. **Deletes all its snapshots for good** (its own files are not touched). It always says first what that means: how many snapshots, which restore points and groups it affects. On a terminal you must type `destroy` and the area's name (`destroy home`); in a script, `--destroy`. The system area is never stopped. |
| `snapall limits AREA [--keep N] [--min-age MINUTES] [--cleanup on\|off]` | How many snapshots an area keeps and when its oldest go. Without options it shows them and what the next cleanup would remove; with options it changes them, and says first what that would remove (`--yes` to go ahead without being asked). |
| `snapall groups [add NAME AREAS\|remove NAME]` | Name a set of your areas, such as `theming`, so that one snapshot can take just those: `snapall create --group theming`. Without an action it lists them. `all` is always there and means every protected area. |
| `snapall cancel CHANGE` | Ask a running change (`snapall activity --json` gives its name) to stop. It finishes the area it is on and then stops, never in the middle of one, so nothing is left half-done. |
| `snapall points` | The restore points: the snapshots that one action took together, one in every area. |
| `snapall restore-all POINT` | Put **every** area back to one restore point with a single action. Shows what changes in each, asks first (`--yes` skips the question, `--dry-run` only shows), takes a safety snapshot per area, and prints the one command that undoes all of it. The running system is left alone. |
| `snapall setup [AREA]` | Find the parts of your disk that have no snapshots yet (such as your Home files), explain in plain words what protecting them means, ask, and do it. `--dry-run` only explains; `--yes` accepts what snapall recommends. Needs `sudo`. |
| `snapall snapshots [AREA]` | The snapshots of one protected area, or of all. |
| `snapall delete AREA ID [--destroy]` | Delete one snapshot **for good**. It says which one (date and description) and asks you to type `destroy`; in a script, `--destroy` (`--yes` is not enough). `0` is the current system and is refused. |
| `snapall undo CHANGE [--yes\|--destroy]` | Roll a change back to the state it recorded (`snapall activity` shows it under each change). It says what that state is and what it will run, checks that it still exists, and asks as heavily as what it does weighs. Where nothing can be undone, it says so and what protects you instead. |
| `snapall changes AREA ID [PATH]` | What restoring that snapshot would change: files it would revert, remove or bring back. |
| `snapall restore AREA ID [PATH]` | Put a protected area (or one path of it) back to how the snapshot had it. Shows what changes and asks first (`--yes` skips the question, `--dry-run` only shows). The state just before is snapshotted first, so a restore can be undone. The system root is refused: see [Restoring](#restoring). |
| `snapall doctor [--install]` | Checks that snapper works; with `--install`, installs it with this system's package manager (shows the commands, asks first, needs root). |
| `snapall tui` | The interactive terminal UI. |
| `snapall contract` | This program's invocation contract, as [argenv](https://github.com/argenv-opencommons/argenv) JSON. |

`--json` (on `create`, `list`, `snapshots`, `delete`, `changes`, `restore`, `history` and `doctor`) prints machine-readable output; `--no-dbus` talks to libsnapper directly (root only; for containers and CI). Every flag has a documented home in `snapall --help`, which is generated from the same declarations that parse the command line.

Nothing is assumed: an unknown config name is an error, never a silent skip; one failing config does not stop the others, every outcome is printed, and the exit status says whether any failed.

| Exit status | Meaning |
|---|---|
| `0` | done |
| `1` | a snapshot or snapper call failed (or there is no terminal for `tui`) |
| `128 + n` | `tui` was stopped by signal `n` (143 for SIGTERM); the terminal is restored first |
| `2` | the invocation is wrong (unknown flag, a flag that does not apply to the command, a bad value) |

A flag given to a command it does not belong to is **rejected**, not ignored, so `snapall delete home 3 --dry-run` can never delete for real while you believe it is a preview. A one-word typo of a command (`snapall lst`) is refused rather than becoming a snapshot named "lst"; write `snapall create lst` if you mean it.

### Environment

One setting, two doors: each of these is the same input as its flag, and an explicit flag wins.

| Variable | Same as |
|---|---|
| `SNAPALL_ONLY=root,home` | `--only root,home` |
| `SNAPALL_NO_DBUS=1` | `--no-dbus` |
| `SNAPALL_JSON=1` | `--json` |
| `SNAPALL_LOG=text\|json` | `--log text\|json` |
| `SNAPALL_LOG_FILE=/var/log/snapall.jsonl` | `--log-file /var/log/snapall.jsonl` |

A misspelled `SNAPALL_*` variable is an **error** with a suggestion (`SNAPALL_NO_DBU` stops the run and says it meant `SNAPALL_NO_DBUS`), not a setting that is silently not applied.

### Verifying what you downloaded

Every release file carries a signed statement of where it was built (SLSA provenance, Sigstore, no key to trust), and the binary is reproducible: rebuilding the tag gives the same bytes.

```console
$ gh attestation verify snapall-v0.4.2-x86_64-unknown-linux-musl.tar.gz --repo grenudi/snapall
```

How to check the checksums, rebuild a release yourself, and what is verified after every release: [docs/verifying.md](docs/verifying.md).

### Completions and the manual

Every package installs shell completions for **bash, zsh, fish and nushell** and the `snapall(1)` manual page, and the tarball carries them in `completions/` and `man/`. They are not written by hand: the contract that parses the command line also generates them (`snapall completions bash|zsh|fish|nushell`, `snapall man`), so they cannot disagree with what snapall accepts. A flag that belongs to one command, like `--dry-run` for `create`, is offered only after that command, which is the same rule `snapall delete --dry-run` is refused by.

```console
$ snapall completions fish > ~/.config/fish/completions/snapall.fish     # without a package
$ snapall man | gzip -9n > ~/.local/share/man/man1/snapall.1.gz
```

### Logging and the audit trail

snapall is quiet by default. Ask, and every operation it performs (listing, creating, deleting, and each package-manager step of `doctor --install`) produces one line with its outcome and duration:

```console
$ sudo snapall --log text create "before theming"
2026-10-23T08:30:14Z INFO  configs_listed count=2 took_ms=6
2026-10-23T08:30:14Z INFO  snapshot_created config=home snapshot=14 description="before theming" took_ms=29
2026-10-23T08:30:14Z INFO  snapshot_created config=root snapshot=212 description="before theming" took_ms=9

$ sudo snapall --log-file /var/log/snapall.jsonl delete home 14      # an audit trail: appended, mode 0600
$ jq -c 'select(.level == "error")' /var/log/snapall.jsonl
```

`--log text|json` writes to stderr: an event stream you route with your environment (a pipe, journald under systemd). `--log-file` appends JSON Lines to a private file instead, which is what you want from cron or a systemd timer. The `tui` owns the terminal, so it logs only to a file. If a log line cannot be written, the operation still happens: losing a log line is better than failing a snapshot.

### Getting protected

snapall keeps snapshots for **protected areas**: parts of your disk, such as *System* (`/`) and *Home* (`/home`). A snapshot of one area does not contain the others, so each needs protecting. On Arch and CachyOS your Home files are an area of their own, which means a computer that was only set up for System has **none of your own files protected**, however many snapshots it takes.

snapall finds this out and fixes it, with explanations, so you do not need to know anything beforehand:

```console
$ sudo snapall setup
A snapshot is a saved moment of your computer. If an update, a new setting or a mistake breaks
something, you can go back to how things were. ...

The parts of your disk, and whether snapshots are kept for them:

  System (/): protected
  Home (/home): NOT protected
      It holds your own files: documents, downloads, pictures, and the settings of your applications.
      Recommended. Snapshots of the System area do not contain these files, so without this a mistake
      or a bad change here could not be undone.

Protect Home (/home)?
  What snapall will do:
    - make a hidden folder called .snapshots inside /home, where its snapshots will be kept;
    - make sure snapshots of it are taken only when you ask, never on a schedule, so nothing is
      taken or piles up on its own.
  What it will not do: it does not copy, move or change any of your files, and it takes no snapshot yet.

Protect Home now? [Y/n]
```

It also tells you whether old snapshots are cleaned up automatically on your computer (a new area keeps the newest 50), and offers to switch that on if it is off. `sudo snapall doctor` warns when a part of your disk that matters has no snapshots, and points at `setup`; `snapall history` and the file-manager entry say the same about a file that no snapshot can contain. Snapshots are not a backup: they live on the same disk.

## Is it working?

`snapall health` answers the questions you have once protection is set up: *is it working, how much room do the snapshots take, will old ones be cleaned up or will they fill the disk?* It starts with a verdict (*All good*, or *2 things need attention*), then says what to do about each thing, most urgent first, and then shows the numbers behind it.

Some figures cannot be known, and it says so instead of guessing. The room each snapshot holds is only counted by the disk's filesystem when a feature called *quotas* is on; without it you see snapshot counts and how full the disk is overall, and the report says why. "Cleanup on" means snapper's cleanup job is switched on **and running**, not merely enabled. ([ADR 16](docs/adr/0016-health-and-activity.md) records what can be measured and how the screen is laid out.)

### What it asks, and the way back

How much snapall asks depends on what is at stake, because a question you answer without reading protects nothing:

* **Reading, and taking a snapshot: never asked.** A snapshot is the safety net itself. If making it took a ceremony, people would skip it.
* **A change that can be put back** (restoring, protecting, settings, groups, switching room counting, stopping a running change): type `y` or `yes`. Enter alone is never a yes. In a script, `--yes`.
* **What loses something for good** (deleting snapshots, stopping protection, a limit that makes cleanup remove snapshots): type `destroy`, and, where it is done to one thing, its name (`destroy home`). In a script, `--destroy`; `--yes` is deliberately not enough.

Every change records **how to get back from it**, worked out when it was made, so it names the state that was really there. `snapall activity` shows it under the change, and `snapall undo CHANGE` does it. A restore goes back to the snapshot it took of the state just before; a limit goes back to the numbers it had; a group to the areas it had; protecting an area goes back to not protected. Undoing the taking of a snapshot is deleting it, so it asks for `destroy`. And what cannot be undone says so: a deleted snapshot, or an area that stopped being protected, record that, and what protects you next time (a snapshot, or a group snapshot with `snapall create --group NAME`, before the risky step). A rollback is checked when you ask for it, not when it was recorded: if cleanup has removed the snapshot it would return to, it says so instead of pretending.

**The window asks the same questions**, through one prompt that every change goes through: it says what will happen, then waits for the typed answer (`y`, or `destroy`; stopping protection takes `destroy home`). A bare Enter, or a click on its button, does nothing until the answer is typed; Esc says nothing was changed. Taking a snapshot never asks. What a change recorded is shown in the **Activity** tab under *How it went*: the state it can go back to and the command that does it (`snapall undo CHANGE`), or that it cannot be undone and what protects you next time. (An undo key in the window is not built yet.)

### Stopping protection

`snapall unprotect AREA`, or **`U` on an area's row** in the Health tab, stops protecting an area. This is the one thing snapall does that cannot be taken back: **every snapshot of the area is deleted for good.** The area's own files are not touched, and protecting it again later (`p`) starts fresh, with no snapshots. So it says first, in plain words, how many snapshots go, how much room that frees (when it is measured), how many restore points stop including the area and which groups name it, and it will not go ahead until you have typed the area's name; a `y` or Enter is not enough.

**The system area is refused**, however you ask. Its snapshots are what the boot menu offers to start the computer from, and how you get back from a bad update.

### How many snapshots an area keeps

Automatic cleanup removes the oldest snapshots of an area once it has more than it is set to keep, and never one younger than a set age. In the Health tab, **Enter on an area** opens its settings (or `snapall limits AREA` on the command line): *keep the newest N*, *never remove one younger than M minutes*, and whether cleanup runs for the area at all.

Lowering how many are kept is the one change here that destroys something, since a removed snapshot cannot be brought back, so the editor **shows what it would remove before it does anything**: "at the next cleanup 5 of the 8 snapshots that cleanup looks after would be removed, the oldest first". A change that would remove something asks twice. The preview is checked against snapper's own cleanup (it said none would go and none did; then it said five and five did). It can only be as exact as the dates it is given, and says "up to" when it is not.

### Groups of areas

A group is a name for some of your areas (they are listed under the areas on the main screen, with a `g groups` button, or `g`): `theming` might be your home folder and the partition holding your game saves. `snapall create --group theming` (or `a`, then `Tab` in the window, which cycles through *all* and your groups) takes a snapshot of just those, as one restore point. In the window, `g` lists the groups and makes, changes and removes them; on the command line, `snapall groups add theming home,data`.

`all` is always there and is every protected area; it is not stored and cannot be changed. The others are kept in a plain file you can read and edit, `/etc/snapall/groups`, one group a line (`theming: home, data`). A group names *areas*, not folders, so it follows whatever snapper says is protected: a group that names an area that no longer is gets refused, with that area's name, and never quietly shortened. Taking a group away changes no area and no snapshot.

### Changes that outlive the window

A change of several areas (a snapshot of everything, a restore point put back) is handed to a `snapall` process of its own. **Closing the window does not stop it**, and reopening the window, or running `snapall activity`, shows it still running with how far it has come ("step 2 of 3: home"), and then how it ended. The status bar shows how many are running; click it to go to them. In the Activity tab, `x` on a running change asks it to stop: it finishes the area it is on, then stops, and says how far it got.

Where systemd is running the process is started through `systemd-run`, so the system keeps its log under the unit `snapall-job-NAME`; elsewhere it is a process of its own. A change of one area takes a moment and is done in the window.

### Background jobs

The Health tab also lists the background jobs that do the work in the dark (cleaning up old snapshots, taking them on a schedule, keeping the boot menu up to date), each with its real name on your computer, whether it is running, and **when it runs, as its own unit file says**. Press Enter on one for the commands that check on it and control it, each beside the key that does the same: its state, its log, run it now, switch it on or off, start, stop, restart.

### Counting the room snapshots take

Out of the box the disk does not count how much room each snapshot holds. Switching that on (`snapall quotas on`, or Enter on that row in the Health tab; the filesystem calls it *quotas*) lets snapall show what each snapshot costs, and lets cleanup act on room as well as on numbers. The price is a tally the disk keeps up to date, which is known to slow some operations down, deleting snapshots most of all, and building it once takes a while on a big disk. It applies to the whole disk and can be switched off again. With few snapshots, and no need to see their size, leaving it off is the better choice. snapall offers only the full kind: the newer *simple* kind charges room to whoever wrote it first, so it cannot say what a *snapshot* holds.

## Before you restore

Restoring an area puts back **everything in it**, including what programs keep there: settings, saved sessions, databases and logs. A program that is still open can write its own copy over what you restored. So close the programs that use those files first, and if one file or folder is all you need, restore just that.

Btrfs gives a structural remedy: **a snapshot stops at a subvolume boundary**. Data that should never roll back (caches, virtual machines, containers, logs) belongs in subvolumes of its own, and a restore of the area around them leaves them alone.

Only one snapall makes a change at a time, whether in the window or on the command line: a second is told who is making one and what it is doing, and reading is never blocked. This guards snapall from snapall; snapper and other tools do not know of it.

## Going back to one moment

Every time you run `sudo snapall` (or press `a` in the window) the snapshots it takes in each area are tagged as one **restore point**, named after the time it started. `snapall points` lists them, and one command puts everything back:

```console
$ snapall points
POINT             WHEN                 AREAS            NAME
20261025T181500Z  2026-10-25 18:15:00  docs, home, root  before theming

$ sudo snapall restore-all 20261025T181500Z
Restore point 20261025T181500Z (before theming):

  docs  put back (3 paths); the state before is snapshot #12
  home  put back (1 path); the state before is snapshot #31
  root  left alone: the running system is not put back while it runs

To undo all of this at once: sudo snapall restore-all 20261025T190200Z
```

- **Undo is the same action.** Each area takes a safety snapshot first, and those form a restore point of their own: the last line is the command that puts everything back as it was just before.
- **The running system is the one thing it will not put back** (see [ADR 12](docs/adr/0012-restore-and-the-boot-menu.md)): files, kernel and boot menu would fall out of step. Every other area is restored, and the output says how to restore the system itself.
- **Older snapshots** made before restore points existed are grouped when they have the same description and were taken within two minutes of each other, which is what pressing `a` did. They are marked *older: matched by name and time*, because that is a match, not a record.
- In the window, press `R` for the list of restore points: choose one with Up and Down, read what it would change in each area, and press Enter to put all of it back. (Or press `u` on a snapshot: if it belongs to a restore point the prompt says so, and `A` puts all of it back.)

## The terminal UI

Your protected areas on the left, the selected area's snapshots on the right, newest first. The buttons along the bottom are coloured blocks, in two rows (doing things first; finding your way and leaving second): the one under the mouse brightens and says what it does on the line above, a click flashes it, and one that does nothing where you are (taking, deleting and restoring act on the snapshot lists, so not on the Health tab) is dim and does not answer. The window has three views, chosen with `1`, `2`, `3` or a click on the tab bar: **Snapshots**, **Health** (is protection working, how much room it takes, what to do about anything that needs it) and **Activity** (what snapall has done, from the window and from commands, in this and earlier sessions, with the reason when something failed; a change that was cut short, because the program was closed or killed or the computer switched off, shows as STOPPED). Under every view a status bar shows the numbers that matter (snapshots, the room they hold, how full the disk is, whether old ones are cleaned up) and each item is clickable: it takes you to the row that explains it. The window explains itself: hover over any button and a sentence on the bottom line says what it does, and `?` opens a two-page help that starts with what a snapshot is. If part of your disk has no snapshots yet (your Home files, usually), the bottom line says so when the window opens, and `p` explains what protecting means and does it, with no command to type.

The packages install a *snapall* entry in your application menu that opens this in a terminal with `sudo`; from a terminal it is `sudo snapall tui`.

| Key | Action |
|---|---|
| `j` `k` / arrows | move in the focused pane |
| `h` `l` / `Tab` | switch pane |
| `space` | mark a snapshot, then move on |
| `c` | create a snapshot of the selected area (asks for a short note) |
| `a` | create a snapshot of every protected area |
| `R` | restore points: list them, see what each would change in every area, put all of one back (Enter) |
| `p` | protect a part of your disk that has no snapshots yet (explains first, asks, does it) |
| `d` | delete the marked snapshots, or the one under the cursor (asks to confirm) |
| `u` | restore the area to the snapshot under the cursor (shows what changes, asks to confirm); `A` in that prompt puts the snapshot's whole restore point back |
| `r` | look again at which snapshots exist |
| `?` / `q` | help (two pages; `Esc` closes) / quit |

The mouse works too: click an area or a snapshot to select it, the first column of a snapshot to mark it, the buttons along the bottom and in prompts, and use the wheel over a pane to scroll it. A click goes through exactly the code its key does, so a button that starts work is refused while snapper is busy, just as the key is. The terminal's mouse mode is handed back on every way out, including a signal.

Snapper calls run in the background: the header shows *working* while one is in flight, the screen stays responsive, and keys that would start more work are refused until it finishes. Quitting while work is in flight waits for it to complete.

> Snapshots live on the same disk. They are not backups.

### Restore one file from your file manager

Right-click a file and choose **Restore from snapshots**: snapall opens on that file, showing how it stood in every snapshot, and Enter or a click puts the version you pick back (after snapshotting the current state, so it can be undone). The packages install the entry for Dolphin (KDE), Nemo (Cinnamon) and PCManFM(-Qt); Nautilus (GNOME) needs its Python bindings, and Thunar (Xfce) one custom action. [docs/file-managers.md](docs/file-managers.md) has each, and the limits. From a terminal: `snapall history FILE`, or `sudo snapall tui --file FILE`.

### Restoring

`restore` and the TUI's `u` put back **files**: a non-root config such as `home`, or single paths of any config. It is `snapper undochange`, always preceded by a snapshot of the current state (`before restoring #N`), so restoring the restore's own snapshot undoes it.

**The system itself** (the root config) is not restored while it runs, and snapall does not edit your bootloader: the kernel and initramfs usually live outside Btrfs, and putting back only the snapshot can leave a system that does not start. What works depends on the machine, and `snapall doctor` reads it for you (a `boot-menu` row): with Limine, `limine-snapper-sync` puts snapshots in the boot menu and `limine-snapper-restore` restores the snapshot with its kernel; with GRUB, `grub-btrfs` adds a snapshots menu; otherwise restore from a live USB. [docs/boot-menu.md](docs/boot-menu.md) says what each setup needs and what switching bootloaders costs; [ADR 12](docs/adr/0012-restore-and-the-boot-menu.md) and [ADR 13](docs/adr/0013-no-automated-boot-menu-setup.md) have the reasoning and its sources. `create` and the TUI show a one-line tip when such a tool would help; `--no-hints` (or `SNAPALL_HINTS=off`) silences it.

## Status

Early. Scope today is **snapper-backed snapshots**, which in practice means Btrfs. Repository hosting and signing for the packages, and a few upstream argenv items are tracked in [docs/backlog.md](docs/backlog.md). Licensed under the GNU General Public License, version 3 or (at your option) any later version.

## Architecture

Hexagonal, with a functional core and an imperative shell, enforced by crate boundaries and a fitness test. See [ARCHITECTURE.md](ARCHITECTURE.md), which maps each decision back to the [software-engineering-canon](https://github.com/creation-guidelines/software-engineering-canon), and the [ADRs](docs/adr/).

## Contributing

Build, test and release instructions, the commit convention, and how the CI proves the packages are in [CONTRIBUTING.md](CONTRIBUTING.md); the rules the architecture depends on are in [AGENTS.md](AGENTS.md).

## License

snapall is free software: you can redistribute it and/or modify it under the terms of the [GNU General Public License](LICENSE) as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version. It comes with **no warranty**. See [docs/adr/0008-license-gpl-3-or-later.md](docs/adr/0008-license-gpl-3-or-later.md) for why, and `THIRD-PARTY-LICENSES.md` (in every package and release) for the licenses of the libraries it contains.

Releases up to and including v0.2.0 were published under MIT OR Apache-2.0; recipients of those versions keep those rights. Versions after v0.2.0 are GPL-3.0-or-later.
