# jurnalis-cli

A standalone command-line REPL for the Jurnalis text-based CRPG engine.

## Installation

### From a release archive

Download the appropriate archive for your platform from the
[GitHub Releases](https://github.com/yanekyuk/jurnalis/releases) page.
CLI releases use tags prefixed with `cli-v` (for example `cli-v0.1.0`).

| Platform            | Archive                                       |
|---------------------|-----------------------------------------------|
| Linux (x86_64)      | `jurnalis-cli-cli-v<version>-x86_64-unknown-linux-gnu.tar.gz` |
| macOS (Intel)       | `jurnalis-cli-cli-v<version>-x86_64-apple-darwin.tar.gz`      |
| macOS (Apple Silicon)| `jurnalis-cli-cli-v<version>-aarch64-apple-darwin.tar.gz`     |
| Windows (x86_64)    | `jurnalis-cli-cli-v<version>-x86_64-pc-windows-msvc.zip`     |

Extract the archive and place the `jurnalis-cli` binary somewhere on your `PATH`:

```bash
tar xzf jurnalis-cli-cli-v0.1.0-x86_64-unknown-linux-gnu.tar.gz
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

CLI releases are independent of engine releases. The naming conventions are:

| Subproject | Tag pattern   | Example        |
|------------|---------------|----------------|
| Engine     | `engine-v*`   | `engine-v0.9.0`|
| CLI        | `cli-v*`      | `cli-v0.1.0`  |

To cut a CLI release:

1. Ensure the version in `jurnalis-cli/Cargo.toml` reflects the intended release.
2. Push a tag matching `cli-v<semver>`:
   ```bash
   git tag cli-v0.1.0
   git push origin cli-v0.1.0
   ```
3. The `release-cli.yml` workflow builds cross-platform binaries and creates a
   GitHub Release with the archives attached.

## License

See the repository root for license information.
