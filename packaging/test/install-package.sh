#!/bin/sh
# Run INSIDE a fresh distro container (as root), repo mounted at /w, packages in /w/dist.
# Installs the real package the way a user would and checks that the dependency on snapper
# was honoured (it pulled snapper in) and that the program runs.
#   usage: install-package.sh deb|rpm-dnf|rpm-zypper|arch
set -eu
kind="${1:?usage: $0 deb|rpm-dnf|rpm-zypper|arch}"
fail() { echo "FAIL: $*" >&2; exit 1; }
# shellcheck source=/dev/null
. /etc/os-release
echo "== $PRETTY_NAME ($kind)"
command -v snapper >/dev/null 2>&1 && fail "snapper is already installed in this image"

case "$kind" in
  deb)
    apt-get update -qq
    apt-get install -y -qq /w/dist/snapall_*.deb
    dpkg -s snapall | grep -E '^Depends:.*snapper' || fail "the deb does not depend on snapper"
    ;;
  rpm-dnf)
    dnf install -y -q /w/dist/snapall-*.rpm
    rpm -q --requires snapall | grep -qx snapper || fail "the rpm does not require snapper"
    ;;
  rpm-zypper)
    zypper --non-interactive --no-gpg-checks install --allow-unsigned-rpm /w/dist/snapall-*.rpm
    rpm -q --requires snapall | grep -qx snapper || fail "the rpm does not require snapper"
    ;;
  arch)
    pacman -Syu --noconfirm --needed base-devel git sudo >/dev/null
    useradd -m builder
    echo 'builder ALL=(ALL) NOPASSWD: ALL' >/etc/sudoers.d/builder
    mkdir -p /home/builder/pkg
    cp /w/packaging/arch/PKGBUILD /w/dist/snapall-src.tar.gz /home/builder/pkg/
    chown -R builder /home/builder/pkg
    # Build the PKGBUILD from this checkout instead of a release tag.
    su builder -c 'cd ~/pkg && _source=snapall-src.tar.gz _dir=snapall-src makepkg -s --noconfirm --nocheck'
    pacman -U --noconfirm /home/builder/pkg/snapall-*.pkg.tar.*
    pacman -Qi snapall | grep -E '^Depends On.*snapper' || fail "the package does not depend on snapper"
    ;;
  *) fail "unknown kind $kind" ;;
esac

command -v snapper >/dev/null 2>&1 || fail "installing snapall did not pull in snapper"
snapall --version
snapall --no-dbus doctor | tee /tmp/doctor.txt
grep -q '^ok    snapper' /tmp/doctor.txt || fail "doctor does not see snapper"
# Ask the package manager what the package contains: container images strip documentation
# from the filesystem on purpose, so looking for README on disk proves nothing.
case "$kind" in
  deb) files="$(dpkg -L snapall)" ;;
  rpm-dnf | rpm-zypper) files="$(rpm -ql snapall)" ;;
  arch) files="$(pacman -Ql snapall)" ;;
esac
echo "$files" | grep -q '/usr/bin/snapall$' || fail "the package does not contain /usr/bin/snapall"
echo "$files" | grep -q 'README.md' || fail "the package does not contain the README"
echo "$files" | grep -q 'LICENSE-MIT' || fail "the package does not contain the license"
echo "PASS: $kind on $ID"
