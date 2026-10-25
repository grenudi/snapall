#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The repository's front page and the files beside the package directories. The instructions on the
# page are the README's own repo snippets, cut out by their markers, so the two cannot differ.
#
# usage: site.sh SITEDIR README.md VERSION
set -euo pipefail
site="${1:?usage: $0 SITEDIR README.md VERSION}"; readme="${2:?}"; version="${3:?}"
here="$(cd "$(dirname "$0")" && pwd)"
fingerprint="$(gpg --show-keys --with-colons "$here/snapall-repo.asc" | awk -F: '/^fpr/{print $10; exit}')"

snippet() { awk -v id="$1" '
  $0 == "<!-- snippet:" id " -->" { want = 1; next }
  want && /^```/ { if (inblock) exit; inblock = 1; next }
  inblock { print }' "$readme"; }
escape() { sed -e 's/&/\&amp;/g' -e 's/</\&lt;/g' -e 's/>/\&gt;/g'; }
section() { # section TITLE SNIPPET-ID
  local code; code="$(snippet "$2")"
  [ -n "$code" ] || { echo "the README has no snippet marked $2" >&2; exit 1; }
  printf '<h2>%s</h2>\n<pre>%s</pre>\n' "$1" "$(printf '%s\n' "$code" | escape)"
}

mkdir -p "$site"
cp "$here/snapall-repo.asc" "$site/key.asc"
cp "$here/snapall.repo" "$site/snapall.repo"
touch "$site/.nojekyll"
cat > "$site/index.html" <<HTML
<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>snapall packages</title>
<style>body{font:16px/1.5 system-ui,sans-serif;max-width:46rem;margin:2rem auto;padding:0 1rem}pre{background:#f4f4f4;padding:.8rem;overflow:auto;border-radius:6px}code{font-family:ui-monospace,monospace}@media(prefers-color-scheme:dark){body{background:#161616;color:#ddd}pre{background:#242424}a{color:#8cf}}</style>
</head><body>
<h1>snapall packages</h1>
<p>Package repositories for <a href="https://github.com/grenudi/snapall">snapall</a>, so that your package manager updates it with the rest of your system. Latest release: <b>${version}</b>. Packages are the release's own files, unchanged; the repositories' metadata is signed.</p>
<p>Signing key fingerprint, to compare with <a href="https://github.com/grenudi/snapall/blob/main/docs/repositories.md">the one in the repository</a>:<br><code>${fingerprint}</code> (<a href="key.asc">key.asc</a>)</p>
$(section "Debian, Ubuntu, Mint, ..." repo-deb)
$(section "Fedora" repo-dnf)
$(section "openSUSE" repo-zypper)
$(section "Arch, CachyOS, Manjaro, ..." repo-pacman)
</body></html>
HTML
echo "site written to $site (version $version, key $fingerprint)"
