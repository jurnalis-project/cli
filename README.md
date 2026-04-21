# jurnalis-cli

A standalone command-line REPL for the Jurnalis text-based CRPG engine.

## Installation

### From a release archive

Download the appropriate archive for your platform from the
[GitHub Releases](https://github.com/jurnalis-project/cli/releases) page.

| Platform            | Archive                                       |
|---------------------|-----------------------------------------------|
| Linux (x86_64)      | `jurnalis-cli-<version>-linux-x86_64.tar.gz`       |
| macOS (Intel)       | `jurnalis-cli-<version>-macos-intel.tar.gz`         |
| macOS (Apple Silicon)| `jurnalis-cli-<version>-macos-apple-silicon.tar.gz`|
| Windows (x86_64)    | `jurnalis-cli-<version>-windows-x86_64.zip`        |

Extract the archive and place the `jurnalis-cli` binary somewhere on your `PATH`:

```bash
tar xzf jurnalis-cli-0.1.0-linux-x86_64.tar.gz
sudo mv jurnalis-cli /usr/local/bin/
```

### Building from source

Requires [Rust](https://rustup.rs/) (stable toolchain).

```bash
git clone https://github.com/yanekyuk/jurnalis.git
cd jurnalis
cargo build --release -p jurnalis-cli
```

The binary is placed at `target/release/jurnalis-cli`.

## Usage

### Play mode (interactive REPL)

Launch the CLI without arguments to start a new game:

```bash
jurnalis-cli
```

You will be guided through character creation, then dropped into the game
world. Type commands at the `>` prompt to explore, interact, and fight.

Built-in REPL commands:

| Command             | Description                              |
|---------------------|------------------------------------------|
| `save [slot]`       | Save current game state (default: `autosave`) |
| `load [slot]`       | Load a saved game (default: `autosave`)  |
| `quit` / `exit`     | Exit the game                            |

All other input is forwarded to the game engine for parsing.

### Dev mode

When compiled with the `dev` feature, you can inject a pre-crafted `GameState`
JSON file to skip character creation and jump directly into a specific scenario:

```bash
cargo run -p jurnalis-cli --features dev -- --dev-state path/to/state.json
```

### stdio-json mode (planned)

A future `--stdio-json` flag will run the CLI as a headless JSON-over-stdin/stdout
interface, suitable for scripting, testing harnesses, and external tool
integration. This mode is not yet implemented.

## Release convention

Releases are cut automatically on every push to `main`. The tag is the bare semver from `Cargo.toml` (e.g. `0.1.0`) and the release name is `jurnalis-cli v<version>`.

To cut a release:

1. Bump the version in `Cargo.toml`.
2. Push to `main` — the `cli-release.yml` workflow builds cross-platform binaries
   and creates a GitHub Release with the archives attached.

## License

See the repository root for license information.
