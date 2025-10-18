# Rusty Path of Building

Rusty Path of Building is a cross-platform runtime for [Path of Building](https://github.com/PathOfBuildingCommunity/PathOfBuilding) and [Path of Building 2](https://github.com/PathOfBuildingCommunity/PathOfBuilding-PoE2). It implements the API used by PoB and handles rendering, window management, and input, essentially serving as a drop-in replacement for the [official runtime](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic).

This project was primarily created to allow Path of Building to run natively on Linux. It is written in Rust (in case you couldn't already tell from the unimaginative name) and is designed to work across all platforms, though it has only been tested on Linux so far.

## Usage

```bash
rusty-path-of-building [poe1|poe2]
```

NOTE: The first run takes a bit longer because Path of Building's assets need to be downloaded.

## Installation

### Arch (AUR)

- [![Stable version badge](https://img.shields.io/aur/version/rusty-path-of-building?style=flat&label=rusty-path-of-building)](https://aur.archlinux.org/packages/rusty-path-of-building)

Please check [the Arch Wiki](https://wiki.archlinux.org/title/Arch_User_Repository) on how to install packages from the AUR.

### Building from source

`LuaJIT` needs to be installed on your system for the `mlua` crate to compile.

```bash
cargo build --release
```

## Runtime Dependencies

Path of Building's Lua code requires the following dynamic c libraries:

- [Lua-cURLv3](https://github.com/Lua-cURL/Lua-cURLv3)
- [luautf8](https://github.com/starwing/luautf8)
- [zlib](https://www.zlib.net/) - The Lua module source is included in this repo under `lua/libs/lzip` and requires `zlib` to compile. Build it with `make LUA_IMPL=luajit` (or `lua51`).

These libraries (`.dll` on Windows, `.so` on Linux) should be placed either in the same directory as the executable or in a `lua` subdirectory alongside it.

## Known Issues

- Clipboard might not work on some Wayland compositors. Check this for compositor support and temporary workaround: https://github.com/1Password/arboard?tab=readme-ov-file#backend-support
