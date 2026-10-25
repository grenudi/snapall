# The package repositories

`https://grenudi.github.io/snapall/` serves signed package repositories for apt, dnf, zypper and
pacman, so that snapall is installed and updated by your package manager like everything else. The
README has the lines to add one; this page says what you are trusting when you do.

## The key

```text
snapall package repository <17498075+grenudi@users.noreply.github.com>
RSA 4096, signing only, no expiry
79B4 B6BD 5766 6623 EAD4  00B2 0AE7 9BC1 E50D DD5A
```

It is the file `packaging/repo/snapall-repo.asc` in this repository, served as
`https://grenudi.github.io/snapall/key.asc`. A key fetched over HTTPS from the project's own site proves
nothing against someone who controls the site, so **compare the fingerprint** that apt, dnf, zypper or
pacman shows (or that `gpg --show-keys key.asc` prints) with the one above and the one in the README.
Both are served from this repository's history, which is the thing to check against.

The private half exists only as an encrypted GitHub Actions secret. It was generated for this purpose and
deleted from the machine that generated it. The workflow that uses it can deploy only from `main` (the
`github-pages` environment allows nothing else), and the steps that handle it run signing tools on
downloaded files and never build or run the project's code.

## What is signed, and what that protects

| | Signed | How a client checks |
|---|---|---|
| apt | the repository index (`InRelease`, `Release.gpg`), which lists every package's checksum | `signed-by=` the key |
| dnf, zypper | the repository metadata (`repomd.xml.asc`), which lists every package's checksum | `repo_gpgcheck=1` |
| pacman | each package (`.sig`) and the database (`snapall.db.sig`) | `SigLevel = Required` |

The apt and rpm packages are **the release's own files, byte for byte**, so they are the ones whose
build provenance GitHub attested ([docs/verifying.md](verifying.md)); the signature on the index makes
a changed package detectable. The rpm packages themselves are not individually signed (`gpgcheck=0`):
the signed metadata carries their checksums, which gives the same protection without re-signing and
changing the attested files.

Tests check this against real tools: a package changed after signing, a repository signed by another
key, and edited metadata are each refused by apt, and the signed rpm metadata is checked to contain
every package's true checksum. dnf, zypper and pacman install from the live repository in containers
after every release.

## How long versions are kept
apt and rpm repositories keep the three newest releases, so an older one can be installed by version
(`apt install snapall=VERSION`). The pacman repository holds only the newest, as most pacman
repositories do. Arch is x86_64 only, because Arch Linux has no official aarch64.

## Replacing the key
If the key is ever lost or exposed, a new one is generated, the secret and `snapall-repo.asc` are
replaced in one change, and this page and the README get the new fingerprint. Everyone who added the
repository then needs to trust the new key (apt: replace the file in `/etc/apt/keyrings`; dnf and zypper:
the next refresh asks; pacman: `pacman-key --add`, `--lsign-key`, and `--delete` the old one). That is a
real cost to users, which is why the key does not expire on a timer.

## How it is built
`.github/workflows/repos.yml` runs after a release has been verified. The scripts are in
`packaging/repo/`: `build-apt.sh`, `build-rpm.sh`, `build-arch.sh` and `site.sh` (the front page, whose
instructions are the README's own snippets, so they cannot differ). The Arch package is built from the
release's `PKGBUILD` in an Arch container with no access to the key; a separate step signs it.
