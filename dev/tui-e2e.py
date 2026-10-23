#!/usr/bin/env python3
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
import struct
import subprocess
import sys
import termios
import time

import pyte

COLS, ROWS = 110, 30
BINARY = sys.argv[1] if len(sys.argv) > 1 else "target/debug/snapall"

KEYS = {
    "enter": b"\r",
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
    def __init__(self):
        master, slave = os.openpty()
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", ROWS, COLS, 0, 0))
        env = {"PATH": os.environ["PATH"], "TERM": "xterm-256color", "HOME": "/root"}
        self.process = subprocess.Popen(
            [BINARY, "--no-dbus", "tui"],
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

    def send(self, *keys):
        for key in keys:
            os.write(self.master, KEYS.get(key, key.encode() if isinstance(key, str) else key))
            self.pump(0.15)

    def type(self, text):
        for char in text:
            self.send(char)

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
    tui.wait_for("ALL configs")
    tui.send("enter")  # empty description -> default
    tui.wait_for("created")
    check("snapall" in listing("snapall_it_a").values(), "config a got the default-described snapshot")
    after_b = listing("snapall_it_b")
    check(len(after_b) == len(before_b) + 1 and "snapall" in after_b.values(), "config b got one too")

    # --- clean exit restores the terminal -----------------------------------------------
    code = tui.finish()
    check(code == 0, "q exits with status 0")
    check(b"\x1b[?1049l" in tui.raw, "the alternate screen was left (terminal restored)")
    print("\nTUI end-to-end: all checks passed")


if __name__ == "__main__":
    main()
