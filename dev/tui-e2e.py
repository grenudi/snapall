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

    def wait_until(self, condition, what, timeout=10.0):
        """Read the terminal until `condition()` holds. (`text()` alone only shows what was already read.)"""
        deadline = time.time() + timeout
        while time.time() < deadline:
            self.pump(0.1)
            if condition():
                return
        raise AssertionError(f"timed out waiting for {what}; screen:\n{self.text()}")

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


def words(tui):
    """The screen as plain words: box lines removed, so that a sentence that wraps inside a prompt reads
    as one."""
    cleaned = "".join(" " if c in "│┃┌┐└┘╭╮╰╯─━┏┓┗┛" else c for c in tui.text())
    return " ".join(cleaned.split())


def clear_journal():
    """Start a journey from a journal with nothing in it: what snapall has done is remembered across runs."""
    for leftover in ("/var/lib/snapall/journal.jsonl", "/var/lib/snapall/journal.jsonl.1"):
        if os.path.exists(leftover):
            os.remove(leftover)


def tabs_scenario():
    """The tab bar and the status bar in a real terminal: a click on a tab, a click on a status item,
    the Health tab's verdict, and the Activity tab recording a real snapshot."""
    clear_journal()
    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=6)
    screen = tui.text()
    check("1 Snapshots" in screen and "2 Health" in screen and "3 Activity" in screen, "the tab bar names every view")
    tui.wait_for("cleanup")  # the status bar fills in once the first screen has its data
    check(" snapshot" in tui.text().splitlines()[-3], "the status bar is under the view and counts the snapshots")
    tui.click_text("2 Health", offset=2)
    tui.wait_for("Health: ")
    tui.wait_for("What this means")
    check(" snapshot" in tui.text().splitlines()[-3], "the status bar is still there on the Health tab")
    check("Space" in tui.text() or "Needs a look" in tui.text() or "Good to know" in tui.text(), "the Health tab lists what it found")
    tui.send("1")
    tui.wait_for("Protected areas")
    col, row = tui.find("snapshots")
    check(row == len(tui.text().splitlines()) - 3, "the first 'snapshots' on the status line is the count")
    tui.click_text(tui.text().splitlines()[-3].strip().split(" | ")[0], offset=1)
    tui.wait_for("What this means")
    check(True, "clicking a status item goes to the Health tab")
    tui.send("3")
    tui.wait_for("What snapall did, from this window and from commands")
    check("Nothing yet" in tui.text(), "the Activity tab starts empty and says what fills it")
    tui.send("a")
    tui.send("enter")
    tui.wait_for("Took a snapshot of snapall_it_a, snapall_it_b", timeout=10)
    check("done" in tui.text(), "a real snapshot is recorded as done")
    tui.send("q")
    tui.finish()


def runner_scenario():
    """A change of several areas runs in a process of its own: the window can be closed while it runs, it
    carries on, its progress and its end are in the record, and a second one is stopped from the window."""
    clear_journal()
    shim = tempfile.mkdtemp(prefix="slow-snapper-")
    real = shutil.which("snapper")
    with open(f"{shim}/snapper", "w") as handle:
        handle.write("#!/bin/sh\ncase \" $* \" in *\" create \"*) sleep 2;; esac\n" + f'exec {real} "$@"\n')
    os.chmod(f"{shim}/snapper", 0o755)
    env = dict(os.environ, PATH=f"{shim}:{os.environ['PATH']}")

    def activity():
        return subprocess.run([BINARY, "--no-dbus", "activity"], capture_output=True, text=True, env=env).stdout

    # An earlier step killed a command in the middle of a slow snapshot; the slow stand-in for snapper it
    # was running goes on for a few seconds after that and then takes the snapshot. Wait for it, so that
    # it is not counted as this journey's.
    for _ in range(30):
        if subprocess.run(["pgrep", "-f", "slow-snapper-"], capture_output=True).returncode != 0:
            break
        time.sleep(0.5)
    before = (len(listing("snapall_it_a")), len(listing("snapall_it_b")))
    tui = Tui(extra_path=shim)
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=6)
    tui.send("a")
    tui.send("enter")
    tui.wait_for("started in the background", timeout=6)
    check("goes on if you close the window" in tui.text().replace("\n", " ") or "started in the background" in tui.text(),
          "the window says the change was handed to the background")
    check("working" not in tui.text().splitlines()[0], "handing it over is not 'working': the window stays free")
    tui.send("q")
    tui.finish()
    check(True, "the window closed while the change was still to be done")

    deadline = time.time() + 12
    seen_running = False
    while time.time() < deadline:
        out = activity()
        if "running" in out:
            seen_running = True
        if "done" in out and "running" not in out:
            break
        time.sleep(0.4)
    check(seen_running, "the change was seen running after the window was gone")
    out = activity()
    check("done" in out and "window" in out and "Took a snapshot of snapall_it_a, snapall_it_b" in out,
          "it finished by itself and the record says the window started it")
    after = (len(listing("snapall_it_a")), len(listing("snapall_it_b")))
    check(after == (before[0] + 1, before[1] + 1), f"both areas got their snapshot, without the window (before {before}, after {after})")

    # a second one, stopped from the window
    tui = Tui(extra_path=shim)
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=6)
    tui.send("a")
    tui.send("enter")
    tui.wait_for("started in the background", timeout=6)
    tui.wait_for("1 running", timeout=6)
    check(True, "the status bar says one change is running")
    tui.send("3")
    tui.wait_for("running", timeout=6)
    tui.send("x")
    tui.wait_for("Stop it after this area?", timeout=4)
    check("never stopped in the middle of an area" in words(tui), "stopping says what it means before it asks")
    tui.send("enter")
    tui.wait_for("CANCELLED", timeout=12)
    check("stopped on request after 1 of 2 areas" in words(tui), "it stopped between areas and says how far it got")
    tui.send("q")
    tui.finish()
    after_stop = (len(listing("snapall_it_a")), len(listing("snapall_it_b")))
    check(after_stop == (after[0] + 1, after[1]), "one area had its snapshot, the other was never begun")


def groups_scenario():
    """Groups of areas from the keyboard: make one, see it kept in a plain file, snapshot just it with Tab in
    the snapshot prompt, and take it away."""
    path = "/etc/snapall/groups"
    if os.path.exists(path):
        os.remove(path)
    before = (len(listing("snapall_it_a")), len(listing("snapall_it_b")))
    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=6)
    tui.send("g")
    tui.wait_for("Groups of areas")
    check("all every protected area (snapall_it_a, snapall_it_b)" in words(tui), "the groups start with all, which is every protected area")
    check("q quit" in tui.text().splitlines()[-1], "the prompt leaves the key line alone")
    tui.send("n")
    tui.wait_for("What should the group be called?")
    tui.type("theming")
    tui.send("enter")
    tui.wait_for("The areas of theming")
    tui.send(" ")  # snapall_it_a
    tui.send("enter")
    # the save is a change like any other: it is done on the worker, and the list shows the group once kept
    tui.wait_until(lambda: "theming snapall_it_a" in words(tui), "the new group in the list")
    check("theming snapall_it_a" in words(tui), "the new group is in the list with its area")
    check(os.path.exists(path) and "theming: snapall_it_a" in open(path).read(), "it is kept in a plain file a person can read")
    tui.send("esc")
    tui.wait_gone("Groups of areas")

    tui.send("a")
    tui.wait_for("New snapshot of ALL protected areas")
    check("Tab: all the areas, or one of your groups" in words(tui), "the prompt says how to choose a group")
    tui.send("tab")
    tui.wait_for("New snapshot of the group theming (snapall_it_a)")
    check(True, "Tab chooses the group and says which areas it covers")
    tui.send("enter")
    tui.pump(0.5)
    tui.wait_gone("working", timeout=10)
    after = (len(listing("snapall_it_a")), len(listing("snapall_it_b")))
    check(after == (before[0] + 1, before[1]), f"only the group's area got a snapshot (before {before}, after {after})")

    tui.send("g")
    tui.wait_for("Groups of areas")
    tui.send("down")
    tui.send("d")
    tui.wait_until(lambda: "theming snapall_it_a" not in words(tui), "the group to be taken away")
    check("theming snapall_it_a" not in words(tui), "the group is taken away")
    check("theming" not in open(path).read(), "and gone from the file")
    tui.send("esc")
    tui.send("q")
    tui.finish()


def limits_scenario():
    """How many an area keeps, from the Health tab: the editor shows what a change would remove, a change that
    removes something asks twice, and snapper then really holds the new numbers."""
    def config(name):
        out = subprocess.run(["snapper", "--no-dbus", "-c", "snapall_it_a", "get-config"], capture_output=True, text=True).stdout
        return next((line.split("|")[1].strip() for line in out.splitlines() if line.split("|")[0].strip() == name), None)

    check(config("NUMBER_LIMIT") == "50", "snapper starts with its own default")
    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=6)
    tui.wait_for("cleanup", timeout=8)
    tui.send("2")
    tui.wait_for("What this means")
    for _ in range(14):
        chosen = [line for line in tui.text().splitlines() if " > " in line]
        if chosen and "Snapall_it_a (" in chosen[0]:
            break
        tui.send("down")
        tui.pump(0.15)
    check("change how many it keeps" in words(tui), "the area's row says what Enter does")
    tui.send("enter")
    tui.wait_for("What Snapall_it_a keeps")
    tui.wait_until(lambda: "Nothing would be removed" in words(tui), "the preview")
    check("Keep the newest [ 50 ] snapshots" in words(tui), "the editor shows what snapper has now")
    check("q quit" in tui.text().splitlines()[-1], "the editor leaves the key line alone")
    tui.send("down")
    for _ in range(30):
        tui.send("left")  # 30 minutes of protection -> none
    tui.send("up")
    for _ in range(49):
        tui.send("left")  # keep 50 -> 1
    tui.wait_until(lambda: "removed, the oldest first" in words(tui), "what would go")
    check("cannot be brought back" in words(tui), "it says a removed snapshot cannot be brought back")
    tui.send("enter")
    tui.wait_for("Press Enter again to apply this")
    check(config("NUMBER_LIMIT") == "50", "the first Enter changes nothing")
    tui.send("enter")
    tui.wait_until(lambda: "takes effect at the next cleanup" in words(tui), "the change to be applied", timeout=12)
    check(config("NUMBER_LIMIT") == "1" and config("NUMBER_MIN_AGE") == "0", "snapper now holds the new numbers")
    tui.send("q")
    tui.finish()


def jobs_and_quotas_scenario():
    """The Health tab's two prompts in a real terminal: a background job's controls (its real name and
    every command made from it) and the question about counting room (its price, and a way out)."""
    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=6)
    tui.send("2")
    tui.wait_for("What this means")
    tui.send("G")
    tui.wait_for("Cleaning up old snapshots (snapper-cleanup.timer)")
    # choose the cleanup job by what the row says, not by where it is: other jobs may be installed too
    for _ in range(12):
        chosen = [line for line in tui.text().splitlines() if " > " in line]
        if chosen and "Cleaning up old snapshots" in chosen[0]:
            break
        tui.send("up")
        time.sleep(0.15)
    check("Background jobs" in tui.text(), "the jobs have a section of their own")
    check("the commands that check on it and control it" in tui.text(), "the row says what Enter does")
    tui.send("enter")
    tui.wait_for("sudo systemctl enable --now snapper-cleanup.timer")
    text = tui.text()
    check("sudo systemctl restart snapper-cleanup.timer" in text and "journalctl -u snapper-cleanup.service -n 50" in text,
          "the controls show every command, made from the unit name")
    check("q quit" in text.splitlines()[-1], "the prompt leaves the key line alone")
    tui.send("esc")
    tui.wait_gone("sudo systemctl enable --now")
    tui.send("up")  # the row above: how the disk counts room
    tui.wait_for("Counting the room snapshots take")
    tui.send("enter")
    tui.wait_for("snapall will switch counting on")
    text = tui.text()
    check("slow some operations down" in text and "leaving it off is the better choice" in text,
          "counting room is explained with its price and when to leave it off")
    tui.send("esc")
    tui.wait_gone("snapall will switch counting")  # the prompt's own sentence; the row's explanation stays
    check("Counting the room snapshots take: off" in tui.text(), "cancelling changed nothing")
    tui.send("q")
    tui.finish()


def journal_scenario():
    """What snapall did is remembered: a command's change shows in the window next to the window's own,
    both are in `snapall activity`, and a command killed mid-change shows as STOPPED."""
    clear_journal()
    made = subprocess.run([BINARY, "--no-dbus", "create", "from a command"], capture_output=True, text=True)
    check(made.returncode == 0, "a command takes a snapshot")

    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=6)
    tui.send("3")
    tui.wait_for("(command)")
    check("Took a snapshot of snapall_it_a, snapall_it_b (command)" in tui.text(),
          "the window shows what a command did, and says it was a command")
    tui.send("1")
    tui.send("a")
    tui.send("enter")
    time.sleep(0.6)  # let the snapshot be taken
    tui.wait_gone("working", timeout=10)
    tui.send("3")
    tui.wait_for("done")
    text = tui.text()
    check(text.count("Took a snapshot of snapall_it_a, snapall_it_b") >= 2, "the window's own change is listed beside the command's")
    tui.send("q")
    tui.finish()

    listed = subprocess.run([BINARY, "--no-dbus", "activity"], capture_output=True, text=True).stdout
    check("window" in listed and "command" in listed, "`snapall activity` lists both, whichever made them")

    # a command killed in the middle of a slow change
    shim = tempfile.mkdtemp(prefix="slow-snapper-")
    real = shutil.which("snapper")
    with open(f"{shim}/snapper", "w") as handle:
        handle.write("#!/bin/sh\ncase \" $* \" in *\" create \"*) sleep 4;; esac\n" + f'exec {real} "$@"\n')
    os.chmod(f"{shim}/snapper", 0o755)
    env = dict(os.environ, PATH=f"{shim}:{os.environ['PATH']}")
    killed = subprocess.Popen([BINARY, "--no-dbus", "create", "cut short"], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.2)
    killed.kill()
    killed.wait()
    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.send("3")
    tui.wait_for("STOPPED")
    check("stopped before it could say how it ended" in tui.text().replace("\n", " ") or "STOPPED" in tui.text(),
          "a command killed mid-change is shown as STOPPED, not forgotten and not still running")
    tui.send("q")
    tui.finish()


def points_screen_scenario():
    """The restore points screen, opened by clicking its footer button: it lists the newest point,
    says what it would change in each area, and Enter puts both areas back."""
    fixture = os.environ.get("SNAPALL_FIXTURE", "/tmp/snapall-fixture")
    a, b = f"{fixture}/mnt/snapall_it_a/screen-e2e.txt", f"{fixture}/mnt/snapall_it_b/screen-e2e.txt"
    for path, text in ((a, "a, then"), (b, "b, then")):
        with open(path, "w") as f:
            f.write(text)
    made = subprocess.run([BINARY, "--no-dbus", "create", "e2e points screen"], capture_output=True, text=True)
    check(made.returncode == 0, "one action snapshots every area")
    with open(a, "w") as f:
        f.write("a, ruined")
    os.remove(b)

    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=5)
    check("R points" in tui.text() and "q quit" in tui.text(), "all nine footer buttons are on screen, quit included")
    tui.click_text("R points", offset=0)
    tui.wait_for("Restore points  (1 of")
    tui.wait_for("e2e points screen")
    tui.wait_for("Putting every area back would:")
    tui.wait_for("snapall_it_a")
    check("[ Enter: Restore all 2 ]" in tui.text(), "it offers to put both areas back")
    tui.hover(*tui.find("Enter: Restore all 2"))
    tui.wait_for("Put every area of the chosen restore point back (not the system)")
    check(True, "hovering the button explains it, and the prompt leaves that line free")
    tui.send("enter")
    tui.wait_for("started in the background", timeout=6)
    tui.wait_for("To undo it: restore point", timeout=15)
    check(open(a).read() == "a, then" and open(b).read() == "b, then", "both areas are back on disk, a deleted file included")
    tui.send("q")
    tui.finish()
    os.remove(a)
    os.remove(b)


def restore_point_scenario():
    """One action snapshots both areas; both are ruined; in the window, `u` on a snapshot of that
    action offers the whole restore point and `A` puts both areas back."""
    fixture = os.environ.get("SNAPALL_FIXTURE", "/tmp/snapall-fixture")
    a, b = f"{fixture}/mnt/snapall_it_a/point-e2e.txt", f"{fixture}/mnt/snapall_it_b/point-e2e.txt"
    with open(a, "w") as f:
        f.write("a, then")
    with open(b, "w") as f:
        f.write("b, then")
    made = subprocess.run([BINARY, "--no-dbus", "create", "e2e restore point"], capture_output=True, text=True)
    check(made.returncode == 0, "one action snapshots every area")
    with open(a, "w") as f:
        f.write("a, ruined")
    os.remove(b)

    tui = Tui()
    tui.wait_for("Snapshots of snapall_it_a (")
    tui.wait_gone("working", timeout=5)
    tui.send("right")  # to the snapshots, newest first: the one just made
    tui.send("u")
    tui.wait_for("This snapshot is part of restore point")
    tui.wait_for("Press A to put all 2 that differ back at once.")
    check("The running system is left alone." in tui.text(), "the prompt says the running system is left alone")
    tui.hover(*tui.find("A: Restore all 2"))
    tui.wait_for("Put all areas of this restore point back at once (not the system)")
    check(True, "hovering the button explains it, and the prompt does not cover that line")
    tui.send("A")
    tui.wait_for("started in the background", timeout=6)
    tui.wait_for("To undo it: restore point", timeout=15)
    check(True, "it confirms in plain words and names the way back")
    check(open(a).read() == "a, then" and open(b).read() == "b, then", "both areas are really back on disk, a deleted file included")
    tui.send("q")
    tui.finish()
    os.remove(a)
    os.remove(b)


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
    # several areas: handed to a process of its own, which the window follows until it ends
    tui.wait_for("started in the background", timeout=6)
    tui.wait_for(": done", timeout=15)
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

    tabs_scenario()
    limits_scenario()
    groups_scenario()
    journal_scenario()
    runner_scenario()
    jobs_and_quotas_scenario()
    points_screen_scenario()
    restore_point_scenario()
    protect_scenario()

    print("\nTUI end-to-end: all checks passed")


if __name__ == "__main__":
    main()
