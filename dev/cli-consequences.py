#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
"""What the command line asks before it changes anything, and the way back it records, on real snapper.

Run on the Btrfs fixture (dev/btrfs-fixture.sh up), as root:  python3 dev/cli-consequences.py target/debug/snapall

* the answers are typed in a real pseudo-terminal: `y`/`yes` for a change, `destroy` for what loses something
  for good (and `destroy NAME` for stopping protection), never a bare Enter, never `y` for a deletion;
* a script has a flag of the same weight: `--yes` cannot answer for a deletion;
* each change records how to get back from it, and the undo really puts the files and settings back;
* a rollback is checked when it is used, and refused when what it would return to is gone.

It unprotects one of the fixture's two areas, so it puts that back, whatever happens.
"""
import json, os, pty, select, subprocess, sys, time

BIN = os.path.abspath(sys.argv[1]) if len(sys.argv) > 1 else "target/debug/snapall"
FIXTURE = os.environ.get("SNAPALL_FIXTURE", "/tmp/snapall-fixture")
B, A = "snapall_it_b", "snapall_it_a"
AREA = f"{FIXTURE}/mnt/{B}"
NOTE = f"{AREA}/note.txt"


def check(condition, message):
    if not condition:
        raise AssertionError(message)
    print(f"  ok  {message}")


def sh(args, **kw):
    return subprocess.run([BIN, "--no-dbus", *args], capture_output=True, text=True, stdin=subprocess.DEVNULL, **kw)


def snapper(*args):
    return subprocess.run(["snapper", "--no-dbus", *args], capture_output=True, text=True).stdout


def snapshots(config=B):
    return [line.split("|")[0].strip() for line in snapper("-c", config, "list", "--columns", "number").splitlines()
            if line.split("|")[0].strip().isdigit() and line.split("|")[0].strip() != "0"]


def setting(name, config=B):
    for line in snapper("-c", config, "get-config").splitlines():
        parts = [p.strip() for p in line.split("|")]
        if parts[0] == name:
            return parts[1]


def protected(config=B):
    return config in snapper("list-configs")


def typed(args, answers):
    """Run snapall in a real terminal, typing each answer when it waits. Returns (exit code, output)."""
    pid, fd = pty.fork()
    if pid == 0:
        os.execv(BIN, [BIN, "--no-dbus", *args])
    out, sent, deadline = b"", 0, time.time() + 40
    while time.time() < deadline:
        ready, _, _ = select.select([fd], [], [], 0.5)
        if ready:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
        elif sent < len(answers):
            os.write(fd, (answers[sent] + "\n").encode())
            sent += 1
    else:
        os.kill(pid, 9)
    _, status = os.waitpid(pid, 0)
    return os.waitstatus_to_exitcode(status), out.decode("utf-8", "replace")


def the_change(prefix):
    entries = json.loads(sh(["activity", "--json"]).stdout)
    return next(e for e in entries if e["what"].startswith(prefix))


def main():
    open(NOTE, "w").write("version 1\n")
    for note in ("one", "two", "three"):
        assert sh(["create", "--only", B, note]).returncode == 0

    print("a deletion is answered by the word destroy, and by nothing else")
    first = snapshots()[0]
    for wrong in ("", "y", "yes", "Destroy", "DESTROY", "delete", "destroy now"):
        code, _ = typed(["delete", B, first], [wrong])
        check(first in snapshots(), f"{wrong!r} deletes nothing")
    code, out = typed(["delete", B, first], ["destroy"])
    check(first not in snapshots() and code == 0, "`destroy` deletes it")
    check("Type destroy, then Enter, to go ahead" in out, "and it said what to type")
    second = snapshots()[0]
    check(sh(["delete", B, second]).returncode == 2, "with nobody to ask and no flag it does nothing and says so (exit 2)")
    check(sh(["delete", B, second, "--yes"]).returncode != 0 and second in snapshots(), "--yes is not accepted for a deletion")
    check(sh(["delete", B, second, "--destroy"]).returncode == 0 and second not in snapshots(), "--destroy is")
    entry = the_change("Deleted")
    check(entry["rollback"]["kind"] == "advice" and "snapall create --group" in entry["rollback"]["text"],
          "a deletion records that it cannot be undone, and what protects next time")

    print("a restore asks y or yes, and its undo really puts the file back")
    open(NOTE, "w").write("version 1\n")
    assert sh(["create", "--only", B, "base"]).returncode == 0
    base = snapshots()[-1]
    open(NOTE, "w").write("version 2\n")
    for wrong in ("", "yep", "no", "ok"):
        typed(["restore", B, base, NOTE], [wrong])
        check(open(NOTE).read() == "version 2\n", f"{wrong!r} restores nothing")
    code, _ = typed(["restore", B, base, NOTE], ["Yes"])
    check(open(NOTE).read() == "version 1\n" and code == 0, "`Yes` restores it")
    restore = the_change("Restored")
    check(restore["rollback"]["kind"] == "undo" and "just before: snapshot #" in restore["rollback"]["what"],
          "the record names the exact state it can return to")
    check(sh(["undo", restore["id"]]).returncode == 2, "undo with nobody to ask does nothing and says so")
    check(sh(["undo", restore["id"], "--yes"]).returncode == 0 and open(NOTE).read() == "version 2\n",
          "undo puts the file back to how it was before the restore")

    print("taking a snapshot never asks, and undoing it is a deletion")
    before = len(snapshots())
    assert sh(["create", "--only", B, "oops"]).returncode == 0
    check(len(snapshots()) == before + 1, "taking a snapshot asked nothing")
    entries = json.loads(sh(["activity", "--json"]).stdout)
    took = next(e for e in entries if e.get("rollback") and e["rollback"]["kind"] == "undo" and e["rollback"]["weight"] == "destroys")
    check(sh(["undo", took["id"], "--yes"]).returncode == 2 and len(snapshots()) == before + 1, "--yes cannot answer for it")
    check(sh(["undo", took["id"], "--destroy"]).returncode == 0 and len(snapshots()) == before, "--destroy can, and the snapshot is gone")

    print("settings and groups are put back to exactly what they were")
    check(setting("NUMBER_LIMIT") == "50", "snapper starts with its default")
    assert sh(["limits", B, "--keep", "7", "--yes"]).returncode == 0 and setting("NUMBER_LIMIT") == "7"
    limits = the_change("Changed how many")
    check("keeping the newest 50" in limits["rollback"]["what"] or "newest 50" in limits["rollback"]["what"], "the record says what it was: 50")
    check(sh(["undo", limits["id"], "--yes"]).returncode == 0 and setting("NUMBER_LIMIT") == "50", "undo sets it back to 50")
    assert sh(["groups", "add", "g1", A, "--yes"]).returncode == 0
    assert sh(["groups", "add", "g1", f"{A},{B}", "--yes"]).returncode == 0
    replaced = the_change("Made the group")
    assert sh(["undo", replaced["id"], "--yes"]).returncode == 0
    line = next(l for l in sh(["groups"]).stdout.splitlines() if l.startswith("g1"))
    check(A in line and B not in line, "undo puts the group back to its first areas")
    sh(["groups", "remove", "g1", "--yes"])

    print("a rollback is checked when it is used")
    open(NOTE, "w").write("version 2\n")
    typed(["restore", B, base, NOTE], ["yes"])
    restore = the_change("Restored")
    safety = restore["rollback"]["what"].split("snapshot #")[1].split(" ")[0]
    assert sh(["delete", B, safety, "--destroy"]).returncode == 0
    r = sh(["undo", restore["id"], "--yes"])
    check(r.returncode != 0 and "no longer exists" in r.stderr, "it is refused when what it would return to has been deleted")

    print("stopping protection needs the word and the name")
    for wrong in ("destroy", "destroy home", "yes", f"destroy {B} x", f"Destroy {B}"):
        typed(["unprotect", B], [wrong])
        check(protected(), f"{wrong!r} stops nothing")
    code, _ = typed(["unprotect", B], [f"destroy {B}"])
    check(not protected() and os.path.exists(NOTE), "`destroy NAME` does, and the files stay")
    gone = the_change("Stopped protecting")
    r = sh(["undo", gone["id"]])
    check(r.returncode != 0 and "cannot be undone" in r.stderr + r.stdout, "it says it cannot be undone")


if __name__ == "__main__":
    try:
        main()
    finally:
        # the fixture is shared with the tests that follow, which expect both areas protected
        if not protected():
            subprocess.run(["snapper", "--no-dbus", "-c", B, "create-config", AREA], capture_output=True)
        for path in (NOTE,):
            if os.path.exists(path):
                os.remove(path)
        subprocess.run(["rm", "-f", "/etc/snapall/groups"])
    print("\nCLI consequences: all checks passed")
