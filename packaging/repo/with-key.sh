#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Run a command with the repository signing key available, and nothing else changed. The key is
# imported into a throwaway keyring, its fingerprint must be the expected one (so a wrong or replaced
# secret fails here, loudly, before anything is signed), and the secret is removed from the
# environment of the command, which gets SIGN_KEY and GNUPGHOME instead.
#
# usage: REPO_SIGNING_KEY=... FINGERPRINT=... with-key.sh COMMAND [ARGS...]
set -euo pipefail
: "${REPO_SIGNING_KEY:?REPO_SIGNING_KEY (the armoured private key) is required}"
: "${FINGERPRINT:?FINGERPRINT (the expected fingerprint) is required}"
[ "$#" -gt 0 ] || { echo "usage: $0 COMMAND [ARGS...]" >&2; exit 2; }

GNUPGHOME="$(mktemp -d)"; chmod 700 "$GNUPGHOME"; export GNUPGHOME
trap 'rm -rf "$GNUPGHOME"' EXIT
printf '%s\n' "$REPO_SIGNING_KEY" | gpg --batch --quiet --import 2>/dev/null
unset REPO_SIGNING_KEY

have="$(gpg --batch --list-secret-keys --with-colons | awk -F: '/^fpr/{print $10; exit}')"
[ -n "$have" ] || { echo "with-key: no secret key could be imported from REPO_SIGNING_KEY" >&2; exit 1; }
[ "$have" = "$FINGERPRINT" ] || { echo "with-key: the secret key is $have, not the expected $FINGERPRINT" >&2; exit 1; }
SIGN_KEY="$have"; export SIGN_KEY
"$@"
