<!-- next-header -->

## [Unreleased] - ReleaseDate

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

[Unreleased]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/meehl/rusty-path-of-building/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/meehl/rusty-path-of-building/releases/tag/v0.1.0
