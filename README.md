# Rusty Path of Building

Rusty Path of Building is a cross-platform runtime for [Path of Building](https://github.com/PathOfBuildingCommunity/PathOfBuilding) and [Path of Building 2](https://github.com/PathOfBuildingCommunity/PathOfBuilding-PoE2). Like the [official runtime](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic), it serves as a host environment for PoB by implementing the API used by the application's Lua logic, as well as rendering, window management, and input handling.

This project was primarily created to allow Path of Building to run natively on Linux. It is written in Rust (in case you couldn't already tell from the unimaginative name) and is designed to work across all platforms, though only Linux has been tested so far.

## Usage

```bash
rusty-path-of-building [poe1|poe2]
```

NOTE: The first run takes a bit longer because Path of Building's assets need to be downloaded.

## Installation

### Arch (AUR)

[![Stable version badge](https://img.shields.io/aur/version/rusty-path-of-building?style=flat&label=rusty-path-of-building)](https://aur.archlinux.org/packages/rusty-path-of-building)

Please check [the Arch Wiki](https://wiki.archlinux.org/title/Arch_User_Repository) on how to install packages from the AUR.

### Building from source

`LuaJIT` needs to be installed on your system for the `mlua` crate to compile.

```bash
cargo build --release
```

## Runtime Dependencies

Path of Building's Lua code requires the following C libraries:

- [Lua-cURLv3](https://github.com/Lua-cURL/Lua-cURLv3)
- [luautf8](https://github.com/starwing/luautf8)
- `lzip` - The source is included in this repo under `lua/libs/lzip` and requires [zlib](https://www.zlib.net/) to compile. Build it with `make LUA_IMPL=luajit`.

Please refer to the [Lua documentation](https://www.lua.org/manual/5.1/manual.html#pdf-package.cpath) to see how it locates libraries.

## Known Issues

- The clipboard doesn't work with Wayland compositors that don't support the data-control extension(s). It is recommended to fall back to `Xwayland` in these cases. This can be done by unsetting `WAYLAND_DISPLAY`. More info here: https://github.com/1Password/arboard?tab=readme-ov-file#backend-support
