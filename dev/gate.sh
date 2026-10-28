#!/bin/bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Everything that has to pass before a change is shipped, in one command with a compact result.
#
#   dev/gate.sh          the fast gate, a minute or two: format, clippy, documentation, links, licences, and the tests that need
#                        no disk (which include the architecture checks and the findability test). Run this before every push.
#   dev/gate.sh --full   adds what needs the Btrfs fixture and root: golden output, the whole test suite on real snapper, the
#                        terminal journeys, the consequences, the completions and the audit trail. CI runs all of that on every
#                        pull request, so use this to reproduce a CI failure, not as a habit: push and let dev/ship.py wait.
#
# Each step writes its output to /tmp/gate/NAME.log and prints one line; a failing step prints the end of its log.
set -u
here=$(cd "$(dirname "$0")/.." && pwd)
cd "$here" || exit 2
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
mkdir -p /tmp/gate
failed=0

step() {
  local name=$1; shift
  local started=$SECONDS
  if "$@" > "/tmp/gate/$name.log" 2>&1; then
    printf 'ok    %-26s %3ss\n' "$name" $((SECONDS - started))
  else
    printf 'FAIL  %-26s %3ss  (/tmp/gate/%s.log)\n' "$name" $((SECONDS - started)) "$name"
    tail -n 14 "/tmp/gate/$name.log" | cut -c1-160 | sed 's/^/        /'
    failed=1
  fi
}

step format        cargo fmt --all -- --check
step clippy        cargo clippy --workspace --all-targets --all-features -- -D warnings
step docs          env RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
step links         python3 dev/check-links.py
step licences      reuse lint
step snippets      ./dev/check-snippets.sh
step tests         cargo test --workspace --all-features --locked

if [ "${1:-}" = "--full" ]; then
  cargo build -q -p snapall || failed=1
  bin=target/debug/snapall
  fresh() { for _ in 1 2; do ./dev/btrfs-fixture.sh down >/dev/null 2>&1; done; rm -f /etc/snapall/groups /var/lib/snapall/journal.jsonl*; ./dev/btrfs-fixture.sh up >/dev/null; }
  step golden        ./dev/golden-output.sh check "$bin"
  fresh
  step tests-on-btrfs env SNAPALL_IT=1 cargo test --workspace --all-features --no-fail-fast
  pkill -9 -f "[d]ebug/snapall --no-dbus tui" 2>/dev/null; fresh
  step tui-journeys  python3 dev/tui-e2e.py "$bin"
  fresh
  step consequences  python3 dev/cli-consequences.py "$bin"
  step real-snapper  env SNAPALL_IT=1 cargo test -p snapall-snapper --test real_snapper
  step completions   bash packaging/test/completions.sh "$bin"
  rm -f /etc/snapall/groups
fi

[ "$failed" = 0 ] && echo "gate: all passed" || echo "gate: FAILED"
exit "$failed"
