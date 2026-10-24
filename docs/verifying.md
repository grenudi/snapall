# Verifying what you downloaded

Every release file (the tarball, the `.deb`, the `.rpm`) comes with a **signed statement of where it was built**: this repository's release workflow, from the tagged source, on GitHub's runners. It is a [SLSA build provenance attestation](https://slsa.dev/spec/v1.0/provenance), signed keylessly through Sigstore, so there is no signing key to trust, steal or lose; the signature is bound to the workflow's identity.

```console
$ gh attestation verify snapall-v0.4.2-x86_64-unknown-linux-musl.tar.gz --repo grenudi/snapall
$ sha256sum --check --ignore-missing SHA256SUMS-x86_64-unknown-linux-musl
```

The checksums only prove that a file arrived intact; the attestation proves who built it. And you do not have to take either on trust: the binary is **reproducible**. The compiler version is pinned (`rust-toolchain.toml`) and the build paths are removed from the binary (`packaging/build-binary.sh`), so the same source gives the same bytes whatever the machine or directory:

```console
$ git clone --branch snapall-v0.4.2 https://github.com/grenudi/snapall && cd snapall
$ packaging/build-binary.sh x86_64-unknown-linux-musl
$ sha256sum target/x86_64-unknown-linux-musl/release/snapall      # the same as `snapall` inside the release tarball
```

CI builds one commit in two different directories and fails if the binaries differ. After every release, `release-verify.yml` rebuilds the tag on another machine and requires the identical binary, checks the checksums and the attestations, and installs the packages on fresh containers of each distribution and architecture. (Releases up to 0.4.1 were built before this and cannot be checked this way.)

## What is checked, and when

| Check | Where | Proves |
|---|---|---|
| `sha256sum --check` against `SHA256SUMS-<target>` | you, and `release-verify.yml` | the file arrived intact |
| `gh attestation verify --signer-workflow .../release-plz.yml` | you, and `release-verify.yml` | this repository's release workflow built it, from the tagged source |
| rebuild the tag with `packaging/build-binary.sh` and compare | you, and `release-verify.yml` (both architectures, another machine) | the binary is what the source says, not just who built it |
| install the package on a fresh container of each distribution and architecture | `release-verify.yml` | the published package works, not only the one CI built |

The same checks run after every release, on what was published rather than on what was meant to be published. A failure there is reported as an unverified release.
