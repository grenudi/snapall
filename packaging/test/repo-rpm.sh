#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The RPM repository format, as far as it can be checked without dnf: a directory per architecture,
# metadata signed by the key, and signed metadata that carries each package's real checksum (which
# is what makes a modified package detectable when repo_gpgcheck=1). dnf and zypper themselves are
# exercised in containers by the repositories workflow.
# usage: repo-rpm.sh PACKAGE.rpm...
set -euo pipefail
[ "$#" -ge 1 ] || { echo "usage: $0 PACKAGE.rpm..." >&2; exit 2; }
root="$(cd "$(dirname "$0")/../.." && pwd)"
work="$(mktemp -d)"; trap 'rm -rf "$work"' EXIT
fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }

mkdir -p "$work/gpg" && chmod 700 "$work/gpg"
export GNUPGHOME="$work/gpg"
gpg --batch --quiet --passphrase '' --quick-generate-key "snapall test repo <test@example.invalid>" rsa2048 sign never 2>/dev/null
fpr="$(gpg --batch --list-keys --with-colons | awk -F: '/^fpr/{print $10; exit}')"
SIGN_KEY="$fpr" bash "$root/packaging/repo/build-rpm.sh" "$work/rpm" "$@" >/dev/null

for package in "$@"; do
  arch="$(rpm -qp --queryformat '%{ARCH}' "$package" 2>/dev/null)"
  [ -f "$work/rpm/$arch/$(basename "$package")" ] || fail "$(basename "$package") is not under $arch/"
  cmp -s "$package" "$work/rpm/$arch/$(basename "$package")" || fail "$(basename "$package") was changed on its way into the repository"
done
ok "each package is under its own architecture, byte for byte unchanged"

python3 - "$work/rpm" "$fpr" <<'PYEOF'
import sys, gzip, hashlib, pathlib, subprocess, xml.etree.ElementTree as ET
repo, fpr = pathlib.Path(sys.argv[1]), sys.argv[2]
for arch_dir in sorted(p for p in repo.iterdir() if p.is_dir()):
    meta = arch_dir / "repodata"
    # 1. the metadata is signed by the key
    check = subprocess.run(["gpg", "--batch", "--verify", str(meta / "repomd.xml.asc"), str(meta / "repomd.xml")], capture_output=True, text=True)
    assert check.returncode == 0 and fpr[-16:] in check.stderr, f"{arch_dir.name}: bad signature: {check.stderr}"
    # 2. repomd.xml names the package list, whose checksum it gives
    ns = {"r": "http://linux.duke.edu/metadata/repo"}
    repomd = ET.parse(meta / "repomd.xml").getroot()
    primary = next(d for d in repomd.findall("r:data", ns) if d.get("type") == "primary")
    location = primary.find("r:location", ns).get("href")
    packed = (arch_dir / location).read_bytes()
    want = primary.find("r:checksum", ns)
    assert hashlib.new(want.get("type"), packed).hexdigest() == want.text, f"{arch_dir.name}: the signed repomd.xml does not match primary.xml"
    # 3. primary.xml gives every package's real checksum
    common = {"c": "http://linux.duke.edu/metadata/common"}
    rows = ET.fromstring(gzip.decompress(packed)).findall("c:package", common)
    assert rows, f"{arch_dir.name}: no packages listed"
    for row in rows:
        name = row.find("c:location", common).get("href")
        digest = row.find("c:checksum", common)
        real = hashlib.new(digest.get("type"), (arch_dir / name).read_bytes()).hexdigest()
        assert real == digest.text, f"{arch_dir.name}/{name}: the metadata's checksum is not the file's"
    print(f"  ok  {arch_dir.name}: metadata signed by the key; repomd -> primary -> {len(rows)} package checksum(s) all match")
PYEOF
echo "PASS: the RPM repository format"
