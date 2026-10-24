# 7. Distro packages from one static binary, tested on real containers

Status: accepted

## Context
The goal is "install snapall on every major distribution". Distributions package software differently, and a package that was never installed on its target distribution is a guess.

## Decision
- **One binary.** The release binary is a static musl build per architecture, so `.deb`, `.rpm`, the tarball and the Nix wrapper all carry the same executable, and there is no per-distro build matrix.
- **Declare the dependency.** The `.deb` and `.rpm` depend on `snapper`, so a package manager installs it. The `PKGBUILD` has `depends=('snapper')`; the flake wraps snapper into the program's `PATH`.
- **One script builds the packages** (`packaging/build-packages.sh`) and both CI and the release workflow call it: what is tested is what ships.
- **Test on the real thing.** For Debian, Ubuntu, Fedora, openSUSE Tumbleweed, Arch and Alpine, CI starts a fresh container that has no snapper, and (1) runs `snapall doctor --install --yes` with the real package manager, (2) installs the real package and checks that snapper was pulled in. A `nix build` job checks the flake.
- **`doctor --install` is the fallback**, for systems with no package (Alpine, Void, Gentoo): it installs the dependency, not snapall itself.
- **Do not run a command that would be wrong.** RHEL and its rebuilds (snapper is only in EPEL), NixOS, immutable systems, and stable Alpine (snapper is only in the `testing` repository of `edge`) get an explanation instead of a command. Arch never runs a bare `pacman -Sy`.
- **The tests found what the research missed.** The first CI run showed that `apk add snapper` fails on Alpine edge: Repology lists the package, but it lives in `testing`. That is why the plan is verified in a real container instead of trusted.

## Considered
- Native packages built per distribution inside distribution containers: more faithful to each policy, but a build matrix to maintain for a single static binary with one dependency.
- Having `doctor` install snapall's own updates: out of scope; that is the package manager's job.

## Verified after publishing

CI proves what was *built*; users get what was *published*. So `release-verify.yml` runs after every Release workflow (and on demand for a tag): it downloads the published assets, verifies them against the published checksums, asserts the tarball carries the program, the license and the third-party notices, installs the published `.deb` and `.rpm` on fresh containers of each distribution and architecture, builds the published PKGBUILD from the tag's real source tarball (so makepkg checks the sha256 the release published), and asserts the installed program reports the tag's version. aarch64 is built and tested natively on GitHub's arm runners rather than assumed from x86_64; Arch has no official aarch64 image and stays x86_64 only. The first run of this found nothing wrong with v0.3.0, but the earlier defect (a checksum file listing a file that was never published) is exactly the kind of thing it exists to catch.

## Provenance, and one script builds everything that ships

The release files are no longer produced by a third-party upload action. `packaging/build-packages.sh` builds the tarball (with the completions and the man page), the `.deb`, the `.rpm` and their checksums, reproducibly (every timestamp is the commit's, tar entries are sorted and owned by root, gzip records no name or time). CI runs that same script on both architectures and tests its output; the release workflow runs it again and attaches exactly those files, so what is attested and attached is what was tested, and one fewer third-party action runs with write access to the release.

Each file is attested with `actions/attest-build-provenance` (SLSA build provenance, Sigstore, keyless). Keyless was chosen over a signing key because a key would have to be generated somewhere, stored as a secret and rotated, which moves trust from the build to whoever holds the key; the attestation is bound to the workflow's identity instead, and `gh attestation verify --signer-workflow` checks that identity. `release-verify.yml` verifies every attestation of every release, so a release without them fails loudly. Releases before this change (up to 0.4.0) have none, and a manual verification of an old tag turns the requirement off explicitly.

Signed apt/rpm *repositories* (so that `apt install snapall` works) still need a long-lived key and a hosting decision; they remain in the backlog.

## Consequences
The packages are unsigned and not in any distribution's repository; both are in the backlog. The Void and Gentoo install plans are unit-tested but not run in CI.
