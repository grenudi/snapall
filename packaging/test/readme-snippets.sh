#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Runs the install-and-upgrade snippets from the README, exactly as written, on this machine:
#
#   1. install the PREVIOUS release the plain way,
#   2. run the README snippet                      -> must now be the newest release (an upgrade),
#   3. remove snapall, run the snippet again       -> the newest release (a fresh install),
#   4. run the snippet once more                   -> still the newest, nothing breaks (a repeat).
#
# The snippet is cut out of README.md at the marker `<!-- snippet:ID -->`, so what is tested is what
# a user pastes. Nothing here is a second copy of it.
#
# usage: readme-snippets.sh deb|rpm-dnf|rpm-zypper|arch|tarball|nix
# env:   SNIPPET_REF  git ref whose README to test (default: main)
#        GITHUB_TOKEN raises the API rate limit, for CI only; users run the snippet without one
set -euo pipefail

family="${1:?usage: $0 deb|rpm-dnf|rpm-zypper|arch|tarball|nix}"
repo=grenudi/snapall
ref="${SNIPPET_REF:-main}"
work="$(mktemp -d)"
cd "$work"

fail() { echo "FAIL: $*" >&2; exit 1; }
ok() { echo "  ok  $*"; }

if [ -n "${GITHUB_TOKEN:-}" ]; then
  # curl drops the header when a download redirects to another host.
  printf 'header = "Authorization: Bearer %s"\n' "$GITHUB_TOKEN" > "$HOME/.curlrc"
  trap 'rm -f "$HOME/.curlrc"' EXIT
fi

# GitHub's releases API lists tags sorted as TEXT (0.9.1 comes before 0.10.0), not by version and not
# by date, so "the newest releases" are chosen here, by version. RELEASES_JSON_FILE lets a test hand in
# a list instead of asking GitHub.
releases_json() {
  if [ -n "${RELEASES_JSON_FILE:-}" ]; then cat "$RELEASES_JSON_FILE"
  else curl -fsS "https://api.github.com/repos/$repo/releases?per_page=100"; fi
}
newest_tags() { # newest_tags COUNT: the COUNT highest versions among the tags on stdin, highest first
  grep -o '"tag_name": *"snapall-v[0-9.]*"' | grep -o 'snapall-v[0-9.]*' | sed 's/^snapall-v//' \
    | sort -u -t. -k1,1nr -k2,2nr -k3,3nr | head -n "$1" | sed 's/^/snapall-v/'
}

mapfile -t tags < <(releases_json | newest_tags 2)
[ "${#tags[@]}" -ge 2 ] || fail "need two releases to test an upgrade, found ${#tags[@]}"
latest="${tags[0]}"; previous="${tags[1]}"
want="${latest#snapall-v}"; before="${previous#snapall-v}"
echo "family $family: $before -> $want"
[ -z "${SELECT_ONLY:-}" ] || { echo "$want $before"; exit 0; }

curl -fsSL "https://raw.githubusercontent.com/$repo/$ref/README.md" -o README.md
snippet="$(awk -v id="$family" '
  $0 == "<!-- snippet:" id " -->" { want = 1; next }
  want && /^```/ { if (inblock) exit; inblock = 1; next }
  inblock { print }' README.md)"
[ -n "$snippet" ] || fail "no snippet marked <!-- snippet:$family --> in the README at $ref"



have() { # the installed version, or nothing
  PATH="$HOME/.nix-profile/bin:/usr/local/bin:$PATH" snapall --version 2>/dev/null | awk '{print $2}' || true
}
# Run the snippet as a user would paste it; answer its prompts the way they would (yes).
run_snippet() { bash -euo pipefail -c "$snippet" < <(yes); }
asset() { # URL of a release asset matching a regex, for a tag
  curl -fsS "https://api.github.com/repos/$repo/releases/tags/$1" | grep -o "https://[^\"]*$2" | head -n1
}

case "$family" in
  deb)
    install_previous() {
      url="$(asset "$previous" "_$(dpkg --print-architecture)\\.deb")"
      (cd /tmp && curl -fLO "$url" && sudo apt-get install -y "./$(basename "$url")")
    }
    remove() { sudo apt-get remove -y snapall; } ;;
  rpm-dnf)
    install_previous() {
      url="$(asset "$previous" "\\.$(uname -m)\\.rpm")"
      (cd /tmp && curl -fLO "$url" && sudo dnf install -y "./$(basename "$url")")
    }
    remove() { sudo dnf remove -y snapall; } ;;
  rpm-zypper)
    install_previous() {
      url="$(asset "$previous" "\\.$(uname -m)\\.rpm")"
      (cd /tmp && curl -fLO "$url" && sudo zypper --non-interactive install --allow-unsigned-rpm "./$(basename "$url")")
    }
    remove() { sudo zypper --non-interactive remove snapall; } ;;
  arch)
    install_previous() {
      d="$(mktemp -d)"
      (cd "$d" && curl -fL "https://github.com/$repo/releases/download/$previous/PKGBUILD" -o PKGBUILD && makepkg -si --noconfirm)
    }
    remove() { sudo pacman -R --noconfirm snapall; } ;;
  tarball)
    install_previous() {
      d="$(mktemp -d)"; url="$(asset "$previous" "$(uname -m)-unknown-linux-musl\\.tar\\.gz")"
      (cd "$d" && curl -fLO "$url" && tar xzf "$(basename "$url")" && sudo install -Dm755 snapall-v*/snapall /usr/local/bin/snapall)
    }
    remove() { sudo rm -f /usr/local/bin/snapall; } ;;
  nix)
    # A flake reference is not pinned to a release, so there is no "previous" to start from:
    # install, check, upgrade, check.
    install_previous() { :; }
    remove() { nix profile remove snapall; } ;;
  *) fail "unknown family $family" ;;
esac

if [ "$family" != nix ]; then
  install_previous
  [ "$(have)" = "$before" ] || fail "expected $before after installing the previous release, got '$(have)'"
  ok "the previous release ($before) is installed"
fi

run_snippet
[ "$(have)" = "$want" ] || fail "after the snippet expected $want, got '$(have)'"
ok "the snippet brought it to $want$([ "$family" = nix ] || echo " (an upgrade from $before)")"

if [ "$family" = nix ]; then
  # The upgrade line of the snippet, on its own, must run cleanly on what is installed.
  bash -euo pipefail -c "$(printf '%s\n' "$snippet" | grep -E '^nix profile upgrade')"
  [ "$(have)" = "$want" ] || fail "after nix profile upgrade expected $want, got '$(have)'"
  ok "nix profile upgrade runs and leaves $want"
else
  remove
  [ -z "$(have)" ] || fail "snapall is still there after removal"
  ok "removed"
  run_snippet
  [ "$(have)" = "$want" ] || fail "fresh install: expected $want, got '$(have)'"
  ok "a fresh install with the snippet gives $want"
  run_snippet
  [ "$(have)" = "$want" ] || fail "repeat: expected $want, got '$(have)'"
  ok "running it again changes nothing"
fi
echo "PASS: the README snippet for $family installs and upgrades"
