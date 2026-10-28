# Changelog

All notable changes to snapall are documented here.
## [0.26.2]


### Refactoring
- The three tabs are three folders named as the tab bar names them ([#91](https://github.com/grenudi/snapall/pull/91)) ([fee6a40])



## [0.26.1]


### Refactoring
- The window's questions are a module of their own ([#90](https://github.com/grenudi/snapall/pull/90)) ([e3b8ff3])



## [0.26.0]


### Refactoring
- A question holds the answers that belong to it ([#86](https://github.com/grenudi/snapall/pull/86)) ([97194c5])



## [0.25.1]


### Refactoring
- The program's entry point reads as the program, and the outline's sentences say what things are for ([#84](https://github.com/grenudi/snapall/pull/84)) ([61578f7])



## [0.25.0]


### Refactoring
- The core is folders named for what snapall is for ([#82](https://github.com/grenudi/snapall/pull/82)) ([6ec1a44])



## [0.24.3]



## [0.24.2]



## [0.24.1]


### Refactoring
- The window's update loop, the health tab and the command line's text are modules by what they do ([#76](https://github.com/grenudi/snapall/pull/76)) ([7bdbf37])



## [0.24.0]


### Refactoring
- The core speaks of areas and locations, and says so in one language ([#74](https://github.com/grenudi/snapall/pull/74)) ([0ead631])



## [0.23.4]



## [0.23.3]



## [0.23.2]



## [0.23.1]



## [0.23.0]


### Refactoring
- The app's vocabulary is declared once and the contract, the questions and the buttons are derived from it ([#63](https://github.com/grenudi/snapall/pull/63)) ([a861b18])



## [0.22.0]


### Features
- The window asks what the command line asks, and the rules are tied to their sources ([#61](https://github.com/grenudi/snapall/pull/61)) ([eda018a])



## [0.21.0]



## [0.20.0]


### Features
- **tui:** A coloured button bar, groups you can see, and buttons that say when they do nothing ([#57](https://github.com/grenudi/snapall/pull/57)) ([82faab9])



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


