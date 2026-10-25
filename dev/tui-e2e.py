#!/usr/bin/env python3

# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""End-to-end test of `snapall tui`: the real binary, in a real pseudo-terminal, against a
real snapper on the Btrfs fixture.

    sudo dev/btrfs-fixture.sh up
    sudo python3 dev/tui-e2e.py target/debug/snapall
    sudo dev/btrfs-fixture.sh down

Needs root (snapper), `pip install pyte`, and the fixture configs snapall_it_a and
snapall_it_b. Keys are sent as the bytes a terminal would send; the screen is read back
through a terminal emulator, so this checks what a user would actually see.
"""

import fcntl
import os
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

import pyte

COLS, ROWS = 110, 30
BINARY = sys.argv[1] if len(sys.argv) > 1 else "target/debug/snapall"

KEYS = {
    "enter": b"\r",
    "tab": b"\t",
    "esc": b"\x1b",
    "up": b"\x1b[A",
    "down": b"\x1b[B",
    "right": b"\x1b[C",
    "left": b"\x1b[D",
    "ctrl-c": b"\x03",
}


def snapper(*args):
    return subprocess.run(
        ["snapper", "--no-dbus", *args], capture_output=True, text=True, check=True
    ).stdout


def listing(config):
    """{number: description} for a config, straight from snapper (the ground truth)."""
    import json

    rows = json.loads(snapper("--jsonout", "-c", config, "list"))[config]
    return {r["number"]: r["description"] for r in rows if r["number"] != 0}


class Tui:
    def __init__(self, extra_path=None, args=("tui",)):
        master, slave = os.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        path = os.environ["PATH"] if not extra_path else f"{extra_path}:{os.environ['PATH']}"
        env = {"PATH": path, "TERM": "xterm-256color", "HOME": "/root"}
        self.process = subprocess.Popen(
            [BINARY, "--no-dbus", *args],
            stdin=slave,
            stdout=slave,
            stderr=slave,
            env=env,
            start_new_session=True,
        )
        os.close(slave)
        self.master = master
        self.screen = pyte.Screen(COLS, ROWS)
        self.stream = pyte.ByteStream(self.screen)
        self.raw = b""

    def text(self):
        return "\n".join(line.rstrip() for line in self.screen.display)

    def pump(self, seconds=0.1):
        deadline = time.time() + seconds
        while time.time() < deadline:
            ready, _, _ = select.select([self.master], [], [], 0.02)
            if ready:
                try:
                    data = os.read(self.master, 65536)
                except OSError:
                    return
                if not data:
                    return
                self.raw += data
                self.stream.feed(data)

    def wait_for(self, needle, timeout=6.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.pump(0.1)
            if needle in self.text():
                return
        raise AssertionError(f"timed out waiting for {needle!r}; screen:\n{self.text()}")

    def wait_gone(self, needle, timeout=6.0):
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.pump(0.1)
            if needle not in self.text():
                return
        raise AssertionError(f"{needle!r} never went away; screen:\n{self.text()}")

    def find(self, needle, nth=0):
        """(column, row) of the nth occurrence of `needle` on the screen, counting from 0."""
        seen = 0
        for row, line in enumerate(self.screen.display):
            start = 0
            while (col := line.find(needle, start)) >= 0:
                if seen == nth:
                    return col, row
                seen += 1
                start = col + 1
        raise AssertionError(f"{needle!r} is not on the screen:\n{self.text()}")

    def click(self, col, row):
        """A left click on a screen cell (0-based), as an xterm SGR mouse press and release."""
        os.write(self.master, f"\x1b[<0;{col + 1};{row + 1}M".encode())
        self.pump(0.1)
        os.write(self.master, f"\x1b[<0;{col + 1};{row + 1}m".encode())
        self.pump(0.3)

    def click_text(self, needle, offset=2, nth=0):
        col, row = self.find(needle, nth)
        self.click(col + offset, row)

    def hover(self, col, row):
        """Move the mouse over a cell (an xterm motion report: button code 35, no button held)."""
        os.write(self.master, f"\x1b[<35;{col + 1};{row + 1}M".encode())
        self.pump(0.3)

    def wheel(self, col, row, down=True):
        os.write(self.master, f"\x1b[<{65 if down else 64};{col + 1};{row + 1}M".encode())
        self.pump(0.3)

    def send(self, *keys):
        for key in keys:
            os.write(self.master, KEYS.get(key, key.encode() if isinstance(key, str) else key))
            self.pump(0.15)

    def type(self, text):
        for char in text:
            self.send(char)

    def stop_with(self, sig):
        """Send a signal; return the exit status once the process has gone."""
        os.kill(self.process.pid, sig)
        deadline = time.time() + 5
        while self.process.poll() is None and time.time() < deadline:
            self.pump(0.1)
        if self.process.poll() is None:
            self.process.kill()
            raise AssertionError(f"snapall did not exit after {signal.Signals(sig).name}")
        self.pump(0.2)
        return self.process.returncode

    def finish(self):
        self.send("q")
        deadline = time.time() + 5
        while self.process.poll() is None and time.time() < deadline:
            self.pump(0.1)
        if self.process.poll() is None:
            self.process.kill()
            raise AssertionError("snapall did not exit after q")
        self.pump(0.2)
        return self.process.returncode


def check(condition, message):
    if not condition:
        raise AssertionError(message)
    print(f"  ok  {message}")


def protect_scenario():
    """Protect a part of the disk from inside the window, on a real mounted Btrfs subvolume: the
    way /home is mounted on a real machine, with no snapper config of its own."""
    fixture = os.environ.get("SNAPALL_FIXTURE", "/tmp/snapall-fixture")
    loop = subprocess.run(["losetup", "-j", f"{fixture}/disk.img"], capture_output=True, text=True).stdout.split(":")[0]
    volume, area = f"{fixture}/mnt/notes-vol", f"{fixture}/notes"
    subprocess.run(["btrfs", "subvolume", "create", volume], check=True, capture_output=True)
    os.makedirs(area, exist_ok=True)
    subprocess.run(["mount", "-o", "subvol=notes-vol", loop, area], check=True)
    with open(f"{area}/letter.txt", "w") as f:
        f.write("a file in a part of the disk with no snapshots\n")
    try:
        tui = Tui()
        tui.wait_for("Snapshots of snapall_it_a (")
        tui.wait_gone("working", timeout=5)  # a person waits for it to finish loading, too
        tui.send("p")
        tui.wait_for("Protect  (1 of")
        # the fixture's own mount is offered too: go round with Tab until it is the one we want
        for _ in range(4):
            if "Protect Notes" in tui.text():
                break
            tui.send("tab")
            tui.pump(0.3)
        screen = tui.text()
        if "Protect Notes" not in screen:
            print("--- the screen when the offer was not found ---\n" + screen + "\n-----------------------------------------------")
        check("Protect Notes" in screen, "the window offers to protect the new part of the disk by name")
        for must in ("It holds whatever is kept in", "does not copy, move or change any of your files", "never on a schedule"):
            check(must in screen, f"the offer says: {must}")
        check("[ Enter: Protect it ]" in screen and "[ Tab: Next ]" in screen, "its buttons are shown")
        tui.hover(*tui.find("[ Enter: Protect it ]"))
        tui.wait_for("Set this part of the disk up for snapshots (nothing is copied or moved)")
        check(True, "hovering Protect it explains what it will do")
        tui.send("enter")
        tui.wait_for("Notes (")
        tui.wait_for("is protected now", timeout=10)
        check(True, "it confirms in plain words and the confirmation survives the reload")
        configs = snapper("--jsonout", "list-configs")
        check(f"{fixture}/notes" in configs, "snapper really has the new area")
        timeline = [l for l in snapper("-c", "notes", "get-config").splitlines() if "TIMELINE_CREATE" in l][0]
        check("no" in timeline, "snapshots on a schedule are off for it")
        check(os.path.exists(f"{area}/.snapshots") and open(f"{area}/letter.txt").read().startswith("a file"), "the hidden folder exists and the file is untouched")
        tui.send("q")
        tui.finish()
    finally:
        subprocess.run(["snapper", "--no-dbus", "-c", "notes", "delete-config"], capture_output=True)
        subprocess.run(["umount", area], capture_output=True)
        subprocess.run(["btrfs", "subvolume", "delete", volume], capture_output=True)


def main():
    # Ground truth we control: two snapshots in config a.
    snapper("-c", "snapall_it_a", "create", "--description", "seed one")
    snapper("-c", "snapall_it_a", "create", "--description", "seed two")
    before_b = listing("snapall_it_b")

    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (2)")
    screen = tui.text()
    print("\n--- first screen ---\n" + screen + "\n--------------------")
    check("snapall_it_a" in screen and "snapall_it_b" in screen, "both configs are listed")
    check("seed one" in screen and "seed two" in screen, "snapshots from real snapper are shown")
    check(screen.index("seed two") < screen.index("seed one"), "newest snapshot first")

    # --- create one snapshot in the selected config -------------------------------------
    tui.send("c")
    tui.wait_for("New snapshot of snapall_it_a")
    tui.type("made in the tui")
    tui.send("enter")
    tui.wait_for("created snapall_it_a #")
    check("made in the tui" in listing("snapall_it_a").values(), "snapper really has the new snapshot")
    # The list is refreshed in the background after the create, so the row appears a moment
    # after the notice; wait for it rather than assuming it is already there.
    tui.wait_for("made in the tui")
    check("made in the tui" in tui.text(), "the new snapshot appears on screen")

    # --- mark the two seeds and delete them ---------------------------------------------
    tui.send("right")
    tui.send("down", " ", " ")  # cursor: seed two (marked), then seed one (marked)
    tui.send("d")
    tui.wait_for("Delete 2 snapshot(s) of snapall_it_a")
    tui.send("y")
    tui.wait_for("deleted")
    remaining = set(listing("snapall_it_a").values())
    check("seed one" not in remaining and "seed two" not in remaining, "snapper deleted exactly the marked snapshots")
    check("made in the tui" in remaining, "the unmarked snapshot survived")

    # --- snapshot every config ----------------------------------------------------------
    tui.send("left")
    tui.send("a")
    tui.wait_for("ALL protected areas")
    tui.send("enter")  # empty description -> default
    tui.wait_for("created")
    check("snapall" in listing("snapall_it_a").values(), "config a got the default-described snapshot")
    after_b = listing("snapall_it_b")
    check(len(after_b) == len(before_b) + 1 and "snapall" in after_b.values(), "config b got one too")

    # --- clean exit restores the terminal -----------------------------------------------
    code = tui.finish()
    check(code == 0, "q exits with status 0")
    check(b"\x1b[?1049l" in tui.raw, "the alternate screen was left (terminal restored)")
    # --- disposability: a termination signal must leave the terminal usable ---------------
    for sig, status in ((signal.SIGTERM, 143), (signal.SIGHUP, 129), (signal.SIGINT, 130)):
        tui = Tui()
        tui.wait_for("Protected areas")
        code = tui.stop_with(sig)
        name = signal.Signals(sig).name
        check(code == status, f"{name} exits with status {status} (128 + signal), got {code}")
        check(b"\x1b[?1049l" in tui.raw, f"{name}: the alternate screen was left")
        check(b"\x1b[?25h" in tui.raw, f"{name}: the cursor was shown again")
        check(b"\x1b[?1000l" in tui.raw, f"{name}: mouse capture was switched off again")

    # --- a slow snapper must not freeze the screen -------------------------------------
    # A shim in front of the real snapper makes every create and delete take two seconds.
    shim = tempfile.mkdtemp(prefix="slow-snapper-")
    real = shutil.which("snapper")
    with open(f"{shim}/snapper", "w") as handle:
        handle.write(
            "#!/bin/sh\n"
            'case " $* " in *" create "*|*" delete "*) sleep 2;; esac\n'
            f'exec {real} "$@"\n'
        )
    os.chmod(f"{shim}/snapper", 0o755)

    tui = Tui(extra_path=shim)
    tui.wait_for("Snapshots of snapall_it_a")
    started = time.time()
    tui.send("c")
    tui.type("slow one")
    tui.send("enter")
    tui.wait_for("working", timeout=1.5)
    check(True, "the header says working while snapper runs")

    # The operation takes two seconds. The screen must answer within a fraction of that.
    asked = time.time()
    tui.send("?")
    tui.wait_for("What is this?", timeout=1.0)
    check(time.time() - asked < 1.5 and tui.process.poll() is None, "the UI answers a key while snapper is busy (help opened)")
    tui.send("\x1b")  # Esc closes the help
    tui.wait_gone("What is this?", timeout=2)

    tui.send("c")
    tui.wait_for("busy: wait for the current operation")
    check(True, "starting more work is refused while busy, with a reason")
    check("slow one" not in listing("snapall_it_a").values(), "the slow operation had not finished yet")

    tui.wait_for("created snapall_it_a #", timeout=8)
    check(time.time() - started >= 2, "the operation really took its two seconds")
    check("slow one" in listing("snapall_it_a").values(), "and it did complete")
    tui.wait_gone("working", timeout=3)  # the list refresh after the create also counts as work
    check("working" not in tui.text(), "the indicator is gone when the work is done")

    # Quitting while work is in flight waits for it instead of abandoning a half-finished call.
    tui.send("c")
    tui.type("quit during work")
    tui.send("enter")
    tui.wait_for("working", timeout=1.5)
    tui.send("q")
    tui.wait_for("finishing", timeout=1.5)
    tui.pump(0.5)
    check(tui.process.poll() is None, "q while busy does not exit at once")
    deadline = time.time() + 8
    while tui.process.poll() is None and time.time() < deadline:
        tui.pump(0.1)
    check(tui.process.poll() == 0, "it exits cleanly once the work is done")
    check("quit during work" in listing("snapall_it_a").values(), "and the operation was not abandoned half way")
    shutil.rmtree(shim, ignore_errors=True)


    # --- the mouse -----------------------------------------------------------------------------
    tui = Tui()
    tui.wait_for("Protected areas")
    check(b"\x1b[?1000h" in tui.raw, "mouse capture is switched on while the UI runs")

    # A click on a config in the list selects it (the first occurrence is the list, not a title).
    tui.wait_for("snapall_it_b")
    tui.click_text("snapall_it_b", offset=2)
    tui.wait_for("Snapshots of snapall_it_b")
    check(True, "clicking a config row selects it and shows its snapshots")

    # The wheel over the snapshots pane moves its cursor without taking the keyboard focus.
    col, row = tui.find("Snapshots of")
    for _ in range(3):
        tui.wheel(col + 5, row + 3, down=True)
    check(tui.process.poll() is None, "the wheel is handled")

    # A click on a footer button does what its key does: "?" opens the help, any click closes it.
    tui.click_text("? help", offset=0)
    tui.wait_for("What is this?")
    check("saved moment" in tui.text(), "the help starts by saying what a snapshot is")
    tui.click_text("[ Enter: Other page ]", offset=4)
    tui.wait_for("Keys and buttons")
    check("hover a button to read what it does" in tui.text(), "page two explains the keys and the mouse")
    tui.click_text("[ Esc: Close ]", offset=3)
    tui.wait_gone("Keys and buttons")
    check(True, "the prompt's Close button closes it")

    # Hovering a button (a real motion report) says in plain words what it does; leaving clears it.
    col, row = tui.find("d delete")
    tui.hover(col + 2, row)
    tui.wait_for("Delete the marked snapshots for good. Your files are not touched")
    check(True, "hovering a button explains it")
    tui.hover(2, 5)
    tui.wait_gone("for good. Your files are not touched")
    check(True, "moving away clears the explanation")

    # "q quit" quits, and gives the mouse back.
    tui.click_text("q quit", offset=0)
    deadline = time.time() + 5
    while tui.process.poll() is None and time.time() < deadline:
        tui.pump(0.1)
    tui.pump(0.2)
    check(tui.process.poll() == 0, "clicking quit exits cleanly")
    check(b"\x1b[?1000l" in tui.raw, "and mouse capture is switched off again")

    # --- restoring, on real Btrfs, with the mouse -----------------------------------------------
    import json

    cfg = "snapall_it_a"
    mnt = [c["subvolume"] for c in json.loads(snapper("--jsonout", "list-configs"))["configs"] if c["config"] == cfg][0]
    keep, gone, added = (os.path.join(mnt, f"tui-restore-{n}") for n in ("keep", "gone", "added"))
    open(keep, "w").write("version one")
    open(gone, "w").write("to be deleted")
    snapper("-c", cfg, "create", "-d", "e2e restore base")
    base = [n for n, d in listing(cfg).items() if d == "e2e restore base"][0]
    open(keep, "w").write("version TWO")
    os.remove(gone)
    open(added, "w").write("new")

    tui = Tui()
    tui.wait_for("Protected areas")
    tui.wait_for(cfg)
    tui.click_text(cfg, offset=2)
    tui.wait_for(f"Snapshots of {cfg}")
    tui.wait_for("e2e restore base")
    tui.click_text("e2e restore base", offset=0)          # selects that snapshot row
    tui.send("u")                                         # the keyboard path to the same prompt
    tui.wait_for("reverted")
    screen = tui.text()
    check("tui-restore-keep" in screen and "snapshotted first" in screen, "the prompt names what would change and says it is undoable")
    check(open(keep).read() == "version TWO", "nothing changed yet: the prompt only looked")

    tui.click_text("[ Enter: Restore ]", offset=4)        # the mouse path to confirming
    tui.wait_for(f"restored {cfg} to #{base}")
    check(open(keep).read() == "version one", "the edited file is back to the snapshot's version")
    check(open(gone).read() == "to be deleted", "the deleted file is back")
    check(not os.path.exists(added), "the file created since is gone")
    notes = listing(cfg)
    check(any(d == f"before restoring #{base}" for d in notes.values()), "snapper has the safety snapshot")
    tui.wait_for(f"before restoring #{base}")
    check(True, "and the TUI lists it")
    tui.finish()
    for path in (keep, gone, added):
        if os.path.exists(path):
            os.remove(path)

    # --- one file's history, as a file manager's right-click opens it ----------------------------
    cfg = "snapall_it_b"
    mnt = [c["subvolume"] for c in json.loads(snapper("--jsonout", "list-configs"))["configs"] if c["config"] == cfg][0]
    doc = os.path.join(mnt, "e2e-history.txt")
    open(doc, "w").write("first version")
    snapper("-c", cfg, "create", "-d", "e2e file one")
    open(doc, "w").write("second version")
    snapper("-c", cfg, "create", "-d", "e2e file two")
    open(doc, "w").write("current version")

    tui = Tui(args=("tui", "--file", doc))
    tui.wait_for("history of")
    tui.wait_for("e2e file two")
    screen = tui.text()
    check(doc in screen, "the header names the file")
    check("Protected areas" not in screen, "there is no areas pane when opened on a file")
    check(screen.count("differs from now") >= 2, "both snapshots say the file differs from now")
    check(screen.find("e2e file two") < screen.find("e2e file one"), "newest first")
    check(b"\x1b[?1000h" in tui.raw, "the mouse is captured on the file screen too")

    # the keyboard: restore the newest snapshot's version
    tui.send("u")
    tui.wait_for("1 reverted")
    check("snapshotted first" in tui.text(), "the prompt says the current state is saved first")
    check(open(doc).read() == "current version", "the prompt alone changed nothing")
    tui.send("\r")
    tui.wait_for("restored")
    check(open(doc).read() == "second version", "the file is back to the second version")
    check(any(d.startswith("before restoring") for d in listing(cfg).values()), "snapper has the safety snapshot")
    tui.wait_for("same as now")
    check(True, "the list caught up: the version just restored now says same as now")

    # the mouse: click the older version, click the restore button, confirm with the button
    tui.click_text("e2e file one", offset=2)
    tui.click_text("u restore this version", offset=0)
    tui.wait_for("Put")
    tui.click_text("[ Enter: Restore ]", offset=4)
    tui.wait_for("restored")
    check(open(doc).read() == "first version", "by mouse: the file is back to the first version")
    check(tui.finish() == 0, "q leaves cleanly")
    os.remove(doc)

    # a file no config covers is an error before the terminal is taken over
    refused = subprocess.run([BINARY, "--no-dbus", "history", "/etc/hostname"], capture_output=True, text=True)
    check(refused.returncode == 1 and "No snapshot contains /etc/hostname" in refused.stderr and "snapper" not in refused.stderr, "a file no snapshot can contain is refused in plain words")

    protect_scenario()

    print("\nTUI end-to-end: all checks passed")


if __name__ == "__main__":
    main()
