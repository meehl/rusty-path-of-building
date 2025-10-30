<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added

- Add regular variant of Fontin font

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

[Unreleased]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.5...HEAD
[0.2.5]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/meehl/rusty-path-of-building/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/meehl/rusty-path-of-building/releases/tag/v0.1.0
