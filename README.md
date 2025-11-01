# Rusty Path of Building

Rusty Path of Building is a cross-platform runtime environment for [Path of Building](https://github.com/PathOfBuildingCommunity/PathOfBuilding) and [Path of Building 2](https://github.com/PathOfBuildingCommunity/PathOfBuilding-PoE2). Like [SimpleGraphic](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic), PoB's official runtime environment, it implements the API functions required by PoB's Lua code, handles window management and input, and renders the UI.

The primary goal of this project is to provide native Linux support for Path of Building. It is written in Rust with cross-platform compatibility in mind and also runs on Windows, though testing there has been minimal.

## Usage

```bash
rusty-path-of-building [poe1|poe2]
```

## Installation

### Arch (AUR)

[![Stable version badge](https://img.shields.io/aur/version/rusty-path-of-building)](https://aur.archlinux.org/packages/rusty-path-of-building)

Please check [the Arch Wiki](https://wiki.archlinux.org/title/Arch_User_Repository) on how to install packages from the AUR.

### Flathub

[![Stable version badge](https://img.shields.io/flathub/v/community.pathofbuilding.PathOfBuilding)](https://flathub.org/en/apps/community.pathofbuilding.PathOfBuilding)

### Building from source

`LuaJIT` needs to be installed on your system for the `mlua` crate to compile.

```bash
cargo build --release
```

## Runtime Dependencies

Path of Building's Lua code requires the following C libraries:

- [Lua-cURLv3](https://github.com/Lua-cURL/Lua-cURLv3)
- [luautf8](https://github.com/starwing/luautf8)
- [luasocket](https://github.com/lunarmodules/luasocket)
- `lzip` - The source is included in this repo under `lua/libs/lzip` and requires [zlib](https://www.zlib.net/) to compile. Build it with `make LUA_IMPL=luajit`.

Please refer to the [Lua documentation](https://www.lua.org/manual/5.1/manual.html#pdf-package.cpath) to see how it locates libraries.

## Known Issues

- The clipboard doesn't work with Wayland compositors that don't support the data-control extension(s). It is recommended to fall back to `Xwayland` in these cases. This can be done by unsetting `WAYLAND_DISPLAY`. More info here: https://github.com/1Password/arboard?tab=readme-ov-file#backend-support
