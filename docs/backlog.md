# Backlog

Every known gap lives here, not in a prose TODO.

| # | Item | Status |
|---|---|---|
| 1 | Adopt `argenv` as the invocation contract (ADR 3) | open |
| 2 | Choose a license (`argenv` uses MIT OR Apache-2.0) | open |
| 3 | Distro packages: `.deb`, `.rpm`, AUR `PKGBUILD`, a Nix flake; consider the openSUSE Build Service for multi-distro builds | open |
| 4 | Per-distro backends and hooks (pacman, zypper, dnf, apt) behind the existing port | open |
| 5 | A TUI as a second driving adapter (ratatui), over the same use case | open |
| 6 | More use cases through the port: list and delete snapshots, restore a file | open |
| 7 | `snapall doctor`: report missing tools, show (and with confirmation run) the install command for the detected distro | open |
| 8 | Observability as a port (structured logs), once there is something to observe | open |
| 9 | Confirm snapper's allowed config-name characters; `ConfigName` is currently stricter than necessary | open |
| 10 | Branch protection on `main` requiring the CI checks | open |
| 11 | Shell completions (deferred by decision) | open |
