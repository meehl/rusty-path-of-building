# Rusty Path of Building

Rusty Path of Building is a cross-platform runtime environment for [Path of Building](https://github.com/PathOfBuildingCommunity/PathOfBuilding) and [Path of Building 2](https://github.com/PathOfBuildingCommunity/PathOfBuilding-PoE2). It serves as a replacement for [SimpleGraphic](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic), the official runtime environment for PoB.

The main focus of this project is native Linux support for Path of Building. It also runs on Windows and macOS, though most of my testing and development is focused on Linux, with support for other platforms largely based on user reports. Contributions that improve support or fix bugs on other platforms are welcome.

## Usage

```bash
rusty-path-of-building [poe1|poe2]
```

## Installation

[![Packaging status](https://repology.org/badge/vertical-allrepos/rusty-path-of-building.svg)](https://repology.org/project/rusty-path-of-building/versions)

### Flathub

[![Stable version badge](https://img.shields.io/flathub/v/community.pathofbuilding.PathOfBuilding)](https://flathub.org/en/apps/community.pathofbuilding.PathOfBuilding)

### Homebrew

[Homebrew Tap](https://github.com/meehl/homebrew-rusty-path-of-building) (NOTE: This has been tested on macOS only)

### Building from source

```bash
cargo build --release
```

#### Build Dependencies

- `LuaJIT` - Required by the `mlua` crate. Alternatively, enable the `vendored-lua` feature to build LuaJIT from source instead of using a system installation.
- `libcurl` - Required when the `bundled-lua-modules` feature (enabled by default) is used.

## Runtime Dependencies

Path of Building's Lua code requires the following C libraries:

- [Lua-cURLv3](https://github.com/Lua-cURL/Lua-cURLv3)
- [luautf8](https://github.com/starwing/luautf8)
- [luasocket](https://github.com/lunarmodules/luasocket)
- [lzip](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic/tree/master/libs/LZip)

By default, the `bundled-lua-modules` feature is enabled and registers Rust implementations of these libraries with the Lua runtime. The original C libraries don't need to be installed if this feature is enabled.

To use the original C libraries instead, disable the feature and make sure Lua can locate them. Please refer to the [Lua documentation](https://www.lua.org/manual/5.1/manual.html#pdf-package.cpath) for details on that.

## Known Issues

- If automatic updates fail, navigate to `~/.local/share/RustyPathOfBuilding{1,2}/` and delete the `rpob.version` file and the `Update` directory. This will force a complete re-download of PoB's latest assets and Lua code on the next startup.
- Automatic updates for weekly beta builds are supported, but compatibility is not checked. If you run into problems with a beta version, please don't open an issue for it.
