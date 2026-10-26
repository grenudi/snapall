# Changelog

All notable changes to snapall are documented here.
## [0.15.0]


### Features
- Remember what snapall did across sessions, from the window and from commands ([#47](https://github.com/grenudi/snapall/pull/47)) ([d07c993])



## [0.14.0]


### Features
- Background jobs from one catalogue, room counting and one change at a time ([#46](https://github.com/grenudi/snapall/pull/46)) ([669e86d])


### Refactoring
- **tui:** Split the window by concern, and keep it that way ([#44](https://github.com/grenudi/snapall/pull/44)) ([8bc8e23])



## [0.13.0]


### Features
- **tui:** Health and activity tabs, and a status bar under every view ([#42](https://github.com/grenudi/snapall/pull/42)) ([0a2770a])



## [0.12.0]


### Features
- Snapall health: whether protection is working, and how much room it takes ([#40](https://github.com/grenudi/snapall/pull/40)) ([5cf2fd7])



## [0.11.0]


### Features
- **tui:** A restore points screen: list them, see what each would change, put all of one back ([#38](https://github.com/grenudi/snapall/pull/38)) ([632ef18])



## [0.10.0]


### Features
- Restore every area to one restore point with a single action ([#35](https://github.com/grenudi/snapall/pull/35)) ([66641d4])



## [0.9.1]


### Bug Fixes
- No message tells a person to run a raw snapper command or calls an area a config ([#33](https://github.com/grenudi/snapall/pull/33)) ([8b32740])



## [0.9.0]


### Features
- Snapall sets up protection itself and explains every step ([#31](https://github.com/grenudi/snapall/pull/31)) ([4c09b39])


### Refactoring
- Main reads as a list of commands; prompts' rules read one by one ([#29](https://github.com/grenudi/snapall/pull/29)) ([85a38b5])



## [0.8.0]


### Bug Fixes
- A file on a separate subvolume is not attributed to the config above it ([#27](https://github.com/grenudi/snapall/pull/27)) ([84550b7])



## [0.7.2]


### Features
- Signed package repositories for apt, dnf, zypper and pacman ([#25](https://github.com/grenudi/snapall/pull/25)) ([4b45346])



## [0.7.1]


### Bug Fixes
- The tarball installs itself, and the snippet test runs only for a release ([#23](https://github.com/grenudi/snapall/pull/23)) ([be4e335])



## [0.7.0]


### Documentation
- How to upgrade on Arch without uninstalling ([#17](https://github.com/grenudi/snapall/pull/17)) ([eac3396])
- Install-and-upgrade snippets for each major system, tested as written ([#19](https://github.com/grenudi/snapall/pull/19)) ([37402b2])


### Features
- Restore one file from your file manager, and make restore auditable ([#22](https://github.com/grenudi/snapall/pull/22)) ([f725b3e])



## [0.6.0]


### Features
- A desktop launcher and icon, a truthful boot-menu check, and a boot-menu guide ([#15](https://github.com/grenudi/snapall/pull/15)) ([13a1401])



## [0.5.0]


### Bug Fixes
- **core:** Doctor only runs commands as root on systems CI installs on for real ([901347c])


### Documentation
- The verification examples use a real, verifiable release (0.4.2) ([0e7fd41])


### Features
- Restore, the mouse in the TUI, and boot-menu detection ([#14](https://github.com/grenudi/snapall/pull/14)) ([ef27b84])



## [0.4.2]


### CI/CD
- Pin the toolchain and make the release binary reproducible, and prove it ([1baacfa])



## [0.4.1]


### CI/CD
- Build, attest and verify releases through one script, and test the shells for real ([e731b5c])


### Features
- **cli:** Shell completions and a man page, generated from the contract and shipped in every package ([0b27b54])



## [0.4.0]


### Documentation
- Record the arm64 testing and the post-release verification ([34c8798])


### Features
- **tui:** Run snapper calls in the background so the screen never freezes ([f70b450])
- **cli:** Adopt argenv 0.3 and delete the workarounds it makes unnecessary **[BREAKING]** ([5433eb8])



## [0.3.0]


### Documentation
- Glossary, ADR 9, and the architecture map for observability ([c027ed4])


### Features
- **observe:** Observability port, audit log, graceful shutdown, typed install gate ([bda6dc5])


### Miscellaneous
- **license:** Relicense snapall under GPL-3.0-or-later **[BREAKING]** ([77bfa34])



## [0.2.0]


### CI/CD
- Check package contents through the package manager ([fba75ed])


### Documentation
- Document the tui, the argenv contract and the decisions ([adf7713])
- Document install, doctor and the packaging decisions ([417888c])


### Features
- **cli:** Declare the invocation surface with argenv ([600ca98])
- **cli:** Add snapall doctor [--install [--yes]] ([0651755])
- **packaging:** Ship .deb, .rpm, a PKGBUILD and a flake, under MIT OR Apache-2.0 ([51217b7])


