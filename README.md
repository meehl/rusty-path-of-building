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

### Building from source

`LuaJIT` needs to be installed for the `mlua` crate to compile.

```bash
cargo build --release
```

## Runtime Dependencies

Path of Building's Lua code requires the following C libraries:

- [Lua-cURLv3](https://github.com/Lua-cURL/Lua-cURLv3)
- [luautf8](https://github.com/starwing/luautf8)
- [luasocket](https://github.com/lunarmodules/luasocket)
- `lzip` - The source is included in this repo under `lua/libs/lzip` and requires [zlib](https://www.zlib.net/) to compile. Build it with `make LUA_IMPL=luajit`. [^1]

Please refer to the [Lua documentation](https://www.lua.org/manual/5.1/manual.html#pdf-package.cpath) for details on how it locates libraries.

## Known Issues

- If automatic updates fail, navigate to `~/.local/share/RustyPathOfBuilding{1,2}/` and delete the `rpob.version` file and the `Update` directory. This will force a complete re-download of PoB's latest assets and Lua code on the next startup.
- Automatic updates for weekly beta builds are supported, but compatibility is not checked. If you run into problems with a beta version, please don't open an issue for it.

[^1]: The lzip source was copied from [SimpleGraphic's LZip library](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic/tree/master/libs/LZip).
