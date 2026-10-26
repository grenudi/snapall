# Changelog

All notable changes to snapall are documented here.
## [0.19.0]


### Features
- Stop protecting an area, after being told exactly what that deletes ([#55](https://github.com/grenudi/snapall/pull/55)) ([e92d61e])



## [0.18.0]


### Features
- Change how many snapshots an area keeps, after seeing what it would remove ([#53](https://github.com/grenudi/snapall/pull/53)) ([270af20])



## [0.17.0]


### Features
- Groups of areas, to snapshot together under a name ([#51](https://github.com/grenudi/snapall/pull/51)) ([64860d4])



## [0.16.0]


### Features
- Changes that outlive the window, with progress, a safe stop and a running item on the status bar ([#49](https://github.com/grenudi/snapall/pull/49)) ([81adcc4])



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



## [0.7.1]



## [0.7.0]


### Features
- Restore one file from your file manager, and make restore auditable ([#22](https://github.com/grenudi/snapall/pull/22)) ([f725b3e])



## [0.6.0]



## [0.5.0]


### Features
- Restore, the mouse in the TUI, and boot-menu detection ([#14](https://github.com/grenudi/snapall/pull/14)) ([ef27b84])



## [0.4.2]



## [0.4.1]



## [0.4.0]


### Features
- **tui:** Run snapper calls in the background so the screen never freezes ([f70b450])



## [0.3.0]


### Features
- **observe:** Observability port, audit log, graceful shutdown, typed install gate ([bda6dc5])


### Miscellaneous
- **license:** Relicense snapall under GPL-3.0-or-later **[BREAKING]** ([77bfa34])



## [0.2.0]


### Features
- **packaging:** Ship .deb, .rpm, a PKGBUILD and a flake, under MIT OR Apache-2.0 ([51217b7])


