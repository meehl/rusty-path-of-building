# Rusty Path of Building

Rusty Path of Building is a cross-platform runtime for [Path of Building](https://github.com/PathOfBuildingCommunity/PathOfBuilding) and [Path of Building 2](https://github.com/PathOfBuildingCommunity/PathOfBuilding-PoE2). It implements the API used by PoB's Lua code and handles rendering, window management, and input. It essentially provides the same functionality as PoB's official runtime, [SimpleGraphic.dll](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic).

The main motivation behind this project was to enable Path of Building to run natively on Linux. It is written in Rust (in case you couldn't already tell from the unimaginative name) and should work on all platforms (though actual testing has only been done on Linux so far).

## Running

```bash
./rusty-path-of-building [poe1|poe2]
```

The first run takes a bit longer as PoB's assets need to be downloaded. (NOTE: The whole installation process was hacked together and needs a lot of improvement)

## Dependencies

Most runtime dependencies are handled automatically by `cargo`. [LuaJIT](https://github.com/LuaJIT/LuaJIT) needs to be installed on your system for `mlua` to work and to compile the libraries mentioned below.

Path of Building's Lua code also requires a few dynamic libraries:

- [Lua-cURLv3](https://github.com/Lua-cURL/Lua-cURLv3)
- [luautf8](https://github.com/starwing/luautf8)
- [zlib](https://www.zlib.net/) - The Lua module source is included in this repo under `lua/libs/lzip` and requires `zlib` to compile. Build it with `make LUA_IMPL=luajit`.

These libraries (`.dll` on Windows, `.so` on Linux) should be placed either in the same directory as the executable or in a `lua` subdirectory alongside it.

## Building

```bash
cargo build --release
```

COMING SOON: PKGBUILD for Arch Linux

## Known Issues

- Clipboard might not work on some Wayland compositors. Check this for compositor support and temporary workaround: https://github.com/1Password/arboard?tab=readme-ov-file#backend-support
