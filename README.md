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

See the [tap available here](https://github.com/meehl/homebrew-rusty-path-of-building). NOTE: This has been tested on macOS only.

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

## AI Assistant

When a build is open, an **IA** tab is available next to *Party* and *Compare*. It lets you chat in Portuguese with an LLM ([Maritaca](https://docs.maritaca.ai/) Sabiá 4 or Sabiazinho 4) about the current build: ask for an analysis, improvements, or new passive tree paths. Every question is sent together with a summary of the build (stats, skills, equipment and allocated passives).

The feature needs a Maritaca API key. Set it with the `MARITACA_API_KEY` environment variable or create `~/.local/share/RustyPathOfBuilding{1,2}/ai.json`:

```json
{
  "api_key": "your-key",
  "base_url": "https://chat.maritaca.ai/api",
  "models": ["sabia-4", "sabiazinho-4"]
}
```

For a private build you can bake the key into the binary with `RPOB_EMBEDDED_API_KEY=your-key cargo build --release`. It is only used when neither the environment variable nor `ai.json` provides one. Anyone who has that binary can extract the key, so don't share it publicly.

Only `api_key` is required. `base_url` (also `MARITACA_BASE_URL`) can point to any OpenAI-compatible `/chat/completions` endpoint. Remote endpoints must use HTTPS; HTTP is accepted only for `localhost`, `127.0.0.1` and `::1`. Redirects are disabled so the API key cannot follow a redirect to another host.

The key is only used by the Rust side of the runtime and is never exposed to PoB's Lua code. Nothing is sent unless you press *Enviar* or one of the shortcut buttons. The request can contain the question, recent conversation, build statistics, skills, equipment, passive tree and relevant game-data references. The AI tab displays the configured endpoint and a privacy warning before the first question.

Requests are limited to 256 KiB and two concurrent calls, retry transient network/HTTP failures up to three times, and discard results cancelled when the conversation or build changes. Do not put API keys, personal information or other secrets in build names, item text or questions.

## Known Issues

- If automatic updates fail, navigate to `~/.local/share/RustyPathOfBuilding{1,2}/` and delete the `rpob.version` file and the `Update` directory. This will force a complete re-download of PoB's latest assets and Lua code on the next startup.
- Automatic updates for weekly beta builds are supported, but compatibility is not checked. If you run into problems with a beta version, please don't open an issue for it.

[^1]: The lzip source was copied from [SimpleGraphic's LZip library](https://github.com/PathOfBuildingCommunity/PathOfBuilding-SimpleGraphic/tree/master/libs/LZip).
