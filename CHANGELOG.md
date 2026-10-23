<!-- next-header -->

## [Unreleased] - ReleaseDate

### Fixed

- Fix error in built-in updater with new `bundled-lua-modules` feature

## [0.3.2] - 2026-10-22

### Changed

- Refactor surface format selection

### Fixed

- Render tab characters as 4 spaces to match SimpleGraphic (#53)

## [0.3.1] - 2026-10-21

### Fixed

- Strip `#@` script headers before loading Lua files to fix syntax error with recent LuaJIT versions

## [0.3.0] - 2026-10-21

### Added

- Add `bundled-lua-modules` feature (enabled by default) for registering Rust implementations of required C libaries. Enabling this features removes the runtime dependencies on `Lua-cURLv3`, `luautf8`, `luasocket`, and `lzip`. Disable it use the original C libraries like before.
- Add `vendored-lua` feature for building LuaJIT from source
- Add support for gzip compression to `inflate`/`deflate` API functions

### Changed

- Rework renderer for significantly improved render performance
- Rework context handling
- Improve performance of image handle access in draw API
- Remove blend mode and mask index from draw API
- Bump dependencies

### Fixed

- Fix color escape regex
- Fix Windows canonical path handling

## [0.2.18] - 2026-05-29

### Changed

- Raise `max_texture_array_layers` GPU requirement to support new gem tooltips

### Fixed

- Fix doubleclick handling

## [0.2.17] - 2026-05-24

### Changed

- Improve macOS compatibility

### Fixed

- Fix broken selection and display of colored text
- Fix black screen during installation on nixOS

## [0.2.16] - 2026-03-23

### Fixed

- Fix bug that causes tarball extraction to fail

## [0.2.15] - 2026-03-22

### Changed

- Replace `ureq` crate with `reqwest` for better IPv4/IPv6 dual stack support
- Refactor installer
- Bump dependencies

## [0.2.14] - 2026-03-14

### Fixed

- Fix deletion of character build directories
- Prevent installer from downloading beta versions

## [0.2.13] - 2026-03-09

### Fixed

- Fix size of ascendancy flavour texts

## [0.2.12] - 2026-03-09

### Changed

- Add better logging and UI feedback for initial install
- Set global timeout for network requests

### Fixed

- Fix confirmation dialog not showing on close with unsaved changes

## [0.2.11] - 2026-03-05

### Fixed

- Clear pressed keys on focus loss to avoid stuck keys
- Use logical keys for input handling to fix unexpected behavior with different keyboard layouts

## [0.2.10] - 2025-12-15

### Added

- Implement GetDrawColor

### Fixed

- Avoid redundant update after initial download

## [0.2.9] - 2025-12-03

### Added

- Implement UI scale overriding

### Changed

- Reduce CPU usage while window is unfocused and not hovered over
- Download highest compatible PoB version instead of latest version

### Fixed

- Register `AbortSubscript` function under right name

## [0.2.8] - 2025-11-07

### Changed

- Replace non-ASCII characters in text input with '?'
- Use `smithay-clipboard` crate for clipboard support on Wayland

## [0.2.7] - 2025-11-02

### Added

- Add missing window icon. Note for package maintainers: On Wayland, the name of the `.desktop` file entries needs to match RustyPoB's `app_id`. `app_id` is set to either `rusty-path-of-building-1` or `rusty-path-of-building-2`, depending on the game argument.

### Fixed

- Fix missing window title during installation step
- Fix UI scaling factor not being applied at startup on X11

## [0.2.6] - 2025-10-31

### Added

- Add regular variant of Fontin font
- Add stubs for DPI scale overrides

## [0.2.5] - 2025-10-30

### Added

- Add support for "faux italics"

### Fixed

- Fix crash that occurs due to surface format selection not filtering out formats that require unrequested GPU features

## [0.2.4] - 2025-10-30

### Added

- Add support for loading webp images
- Add support for new 'fontin' fonts

### Changed

- Use `~/.local/share/RustyPathOfBuilding{1,2}/userdata` as default location for settings and build files. Builds files created prior to this change need to be manually copied from the old location in `~/Documents/`. Sorry about the inconvenience.

### Fixed

- Fix scrolling of gem selection dropdown

## [0.2.3] - 2025-10-29

### Added

- Add exponential backoff for github requests

## [0.2.2] - 2025-10-29

### Fixed

- Fix version file

## [0.2.1] - 2025-10-28

### Added

- Add stub for `TakeScreenshot` api function

### Fixed

- Fix crash caused by missing `SetForeground` api function

## [0.2.0] - 2025-10-28

### Added

- Add visual indicator for download progress in installer
- Handle dpi awareness feature flag

### Changed

- Remove global context
- Notify windowing system before presenting
- Cleanup image loading

## [0.1.2] - 2025-10-20

### Fixed

- Fix problem that causes non-identical frames to be elided.
- Correctly align layout origins with physical pixel grid for sharper text rendering.

## [0.1.1] - 2025-10-19

### Added

- Add install target to lzip Makefile
- Setup `cargo-release`

### Fixed

- Fix problem caused by wrong buffer type in inflate and deflate (#1)

### Changed

- Move manifests into separate repo
- Change script directory name to avoid conflicts with official PoB
- Remove modification of lua `package.cpath`. Libraries installed outside of the default path can be specified with the `LUA_CPATH` env variable.

## [0.1.0] - 2025-10-18

### Added

- First release

<!-- next-url -->

[Unreleased]: https://github.com/meehl/rusty-path-of-building/compare/v0.3.2...HEAD
[0.3.2]: https://github.com/meehl/rusty-path-of-building/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/meehl/rusty-path-of-building/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.18...v0.3.0
[0.2.18]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.17...v0.2.18
[0.2.17]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.16...v0.2.17
[0.2.16]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.15...v0.2.16
[0.2.15]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.14...v0.2.15
[0.2.14]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.13...v0.2.14
[0.2.13]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.12...v0.2.13
[0.2.12]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.11...v0.2.12
[0.2.11]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.10...v0.2.11
[0.2.10]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.9...v0.2.10
[0.2.9]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.8...v0.2.9
[0.2.8]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.7...v0.2.8
[0.2.7]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.6...v0.2.7
[0.2.6]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.5...v0.2.6
[0.2.5]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/meehl/rusty-path-of-building/releases/tag/v0.1.0
