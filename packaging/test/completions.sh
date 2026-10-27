#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

# The completions and the man page, tested in real shells against snapall's real contract.
#
#   packaging/test/completions.sh target/release/snapall
#
# Needs bash, fish, zsh, nu (nushell) and groff. CI installs them; a missing one is a failure,
# not a skip, so this cannot pass by testing nothing.
set -euo pipefail

bin="$(realpath "${1:?usage: $0 path/to/snapall}")"
here="$(cd "$(dirname "$0")" && pwd)"
for tool in bash fish zsh nu groff timeout; do
  command -v "$tool" >/dev/null 2>&1 || { echo "FAIL: $tool is required" >&2; exit 1; }
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cd "$work"
fail() { echo "FAIL: $*" >&2; exit 1; }
pass() { echo "  ok  $*"; }

"$bin" completions bash > snapall.bash
"$bin" completions zsh > _snapall
"$bin" completions fish > snapall.fish
"$bin" completions nushell > snapall.nu
"$bin" man > snapall.1
: > alpha.log   # something for file-name completion to find

# ---- bash: simulate the completion machinery word by word ---------------------------------
bash_complete() {
  local words="" w
  for w in "$@"; do words+=" '$w'"; done
  bash -c "source '$work/snapall.bash'; COMP_WORDS=($words); COMP_CWORD=$(($# - 1)); COMPREPLY=(); _snapall; printf '%s\n' \"\${COMPREPLY[@]}\""
}
expect_line() { # expect_line "output" word description
  grep -qx -- "$2" <<<"$1" || fail "$3: expected '$2' in:
$1"
}
refute_line() {
  if grep -qx -- "$2" <<<"$1"; then fail "$3: did not expect '$2'"; fi
}

out="$(bash_complete snapall "")"
for command in create list snapshots delete doctor completions man tui contract; do
  expect_line "$out" "$command" "bash offers the command $command"
done
pass "bash offers every command"
[ "$(bash_complete snapall cr)" = "create" ] || fail "bash completes a command by prefix"
pass "bash completes a command by prefix"
[ "$(bash_complete snapall create --d)" = "--dry-run" ] || fail "bash offers --dry-run for create"
pass "bash offers --dry-run after create"
# delete offers --destroy (a deletion is answered by it) and still never create's --dry-run
[ "$(bash_complete snapall delete --d)" = "--destroy" ] || fail "bash must offer --destroy, and not --dry-run, for delete"
pass "bash does NOT offer create's --dry-run after delete (the very mistake lint rejects), only --destroy"
out="$(bash_complete snapall delete -)"
expect_line "$out" "--log" "bash offers unscoped flags for delete"
refute_line "$out" "--only" "delete does not take --only"
expect_line "$out" "--destroy" "delete takes --destroy"
refute_line "$out" "--yes" "--yes cannot answer for a deletion, so delete does not offer it"
pass "bash offers the unscoped flags and only those for delete"
# restore shares --dry-run with create and --yes with doctor, and changes takes neither of them
out="$(bash_complete snapall restore -)"
expect_line "$out" "--dry-run" "bash offers --dry-run for restore"
expect_line "$out" "--yes" "bash offers --yes for restore"
expect_line "$out" "--json" "bash offers --json for restore"
refute_line "$out" "--only" "restore does not take --only"
out="$(bash_complete snapall changes -)"
refute_line "$out" "--yes" "changes only looks, so it takes no --yes"
refute_line "$out" "--dry-run" "changes only looks, so it takes no --dry-run"
expect_line "$out" "--json" "bash offers --json for changes"
# tips belong to create and the TUI, and can be switched off by flag
expect_line "$(bash_complete snapall create --no-)" "--no-hints" "bash offers --no-hints for create"
[ -z "$(bash_complete snapall list --no-h)" ] || fail "bash must not offer --no-hints for list"
pass "bash scopes restore, changes and --no-hints correctly"
# history is read-only and takes a file; --file belongs to the TUI alone
out="$(bash_complete snapall history -)"
expect_line "$out" "--json" "bash offers --json for history"
refute_line "$out" "--dry-run" "history only reads, so it takes no --dry-run"
refute_line "$out" "--file" "--file is the TUI's, not history's"
expect_line "$(bash_complete snapall tui --f)" "--file" "bash offers --file for tui"
[ -z "$(bash_complete snapall create --fi)" ] || fail "bash must not offer --file for create"
pass "bash scopes history and tui --file correctly"
out="$(bash_complete snapall create --log "")"
for value in off text json; do expect_line "$out" "$value" "bash offers the --log value $value"; done
pass "bash completes the values of --log after a command"
# --log belongs to the commands that log, so before any command its values are not in play
# (fish and zsh agree; bash used to complete them anyway, fixed in argenv 0.3.2).
out="$(bash_complete snapall --log "")"
refute_line "$out" "text" "bash must not offer --log values before a command"
pass "bash does not offer a scoped flag's values before a command"
out="$(bash_complete snapall completions "")"
for value in bash zsh fish nushell; do expect_line "$out" "$value" "bash offers the shell $value"; done
pass "bash completes the shell for completions"
[ -z "$(bash_complete snapall create --only "")" ] || fail "bash must not invent values for --only"
pass "bash suggests nothing for a free-text value"
[ "$(bash_complete snapall create --log-file al)" = "alpha.log" ] || fail "bash completes a file for --log-file"
pass "bash completes file names for --log-file"

# ---- fish ---------------------------------------------------------------------------------
fish_complete() { fish --no-config -c "source '$work/snapall.fish'; complete -C '$1'"; }
grep -q -- "--dry-run" <<<"$(fish_complete 'snapall create --d')" || fail "fish offers --dry-run for create"
if grep -q -- "dry-run" <<<"$(fish_complete 'snapall delete --d')"; then fail "fish must not offer --dry-run for delete"; fi
out="$(fish_complete 'snapall create --log ')"
for value in off text json; do grep -q "$value" <<<"$out" || fail "fish offers the --log value $value"; done
grep -q nushell <<<"$(fish_complete 'snapall completions ')" || fail "fish completes the shell"
out="$(fish_complete 'snapall ')"
for command in create list snapshots delete doctor completions man tui contract; do grep -q "$command" <<<"$out" || fail "fish offers $command"; done
pass "fish: scoping, values and commands"

# ---- zsh: a real interactive session, pressing Tab ----------------------------------------
zsh_complete() {
  timeout 60 zsh -f "$here/zsh-complete.zsh" "$work" "$1" | tr -d '\r'
}
grep -q -- "--dry-run" <<<"$(zsh_complete 'snapall create --d')" || fail "zsh offers --dry-run for create"
if grep -q -- "dry-run" <<<"$(zsh_complete 'snapall delete --d')"; then fail "zsh must not offer --dry-run for delete"; fi
grep -q nushell <<<"$(zsh_complete 'snapall completions ')" || fail "zsh completes the shell"
grep -q "snapall create" <<<"$(zsh_complete 'snapall cr')" || fail "zsh completes a command"
pass "zsh: scoping, values and commands, in a real terminal"

# ---- nushell ------------------------------------------------------------------------------
names="$(nu --no-config-file -c "source '$work/snapall.nu'; scope externs | where name =~ '^snapall' | get name | to json")"
for command in create list snapshots delete doctor completions man tui contract; do
  grep -q "\"snapall $command\"" <<<"$names" || fail "nushell defines no extern for $command: $names"
done
pass "nushell: an extern for every command"

# ---- the man page -------------------------------------------------------------------------
warnings="$(groff -man -z -ww snapall.1 2>&1 || true)"
[ -z "$warnings" ] || fail "groff complains about the man page: $warnings"
grep -q '^\.SH EXIT STATUS' snapall.1 || fail "the man page has no EXIT STATUS"
grep -q 'SNAPALL_ONLY' snapall.1 || fail "the man page does not list the environment"
pass "the man page is clean roff with its sections"

echo "PASS: completions and man page, in real shells"
