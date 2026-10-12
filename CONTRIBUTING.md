# Development

[Back to README](README.md)

Use stable Rust and the platform build tools listed in the README. Windows and
macOS are the supported desktop platforms. Dependency downloads are required for
the first build, not for playing built packages.

## Checks

Run before submitting a change:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --release --locked --bins
```

Include regression tests for rule and protocol changes. Keep changes focused and
document changes to controls, rules, or networking. For bug reports, include the
OS, CPU architecture, game version or commit, reproduction steps, and relevant
logs. Remove private invites, rejoin tokens, and personal addresses before sharing.

Tests cover the rules engine, card conservation, scoring, UNO and +4 decisions,
self-play, TCP framing and reconnect, private hands, and camera/card geometry.
The macOS native clipboard test is ignored by default because it changes the
system clipboard; run it explicitly only when appropriate:

```sh
cargo test --locked --bin uno clipboard::tests::native_clipboard_roundtrip -- --ignored
```

### Visual smoke tests

With a graphical desktop available, render GPU screenshots of menus, tables,
camera presets, large hands, dialogs, and connection states:

```sh
# macOS
UNO_SCREENSHOT_DIR=/tmp/uno-screens cargo run --locked --bin uno
```

```powershell
# Windows PowerShell
$env:UNO_SCREENSHOT_DIR = "$env:TEMP\uno-screens"
cargo run --locked --bin uno
Remove-Item Env:UNO_SCREENSHOT_DIR
```

Inspect the images for layout and rendering regressions. These complement, rather
than replace, interactive testing of input and multiplayer behavior.

## Packaging

```sh
# macOS: .app, console host, and documentation
bash scripts/package-macos.sh
```

```powershell
# Windows: desktop executable, console host, and documentation
.\scripts\package-windows.ps1
```

Packages are written to `dist/uno-<target>.zip`. The scripts rebuild their staging
directory on each run. macOS defaults to the current Rust host target; set `TARGET`
to `aarch64-apple-darwin` or `x86_64-apple-darwin` to select it explicitly. The Windows
script supports `x86_64-pc-windows-msvc`. Run each script on its corresponding OS.

The CI workflow (`.github/workflows/ci.yml`) checks and packages Windows x64, macOS
Apple Silicon, and macOS Intel, then uploads workflow artifacts. It does not
publish GitHub releases. Downloaded workflow artifacts contain the package ZIP.

Packages are **not distribution-signed or notarized**; macOS packaging applies
only a local ad-hoc signature. Do not disable platform security protections.
Public binary distribution should use appropriate signing and notarization.

When changing the version, update `Cargo.toml`, `Cargo.lock`, and the macOS version
fields in `packaging/macos/Info.plist`. Keep generated packages, screenshots,
credentials, and local build output out of Git.

## Source layout

| Path | Purpose |
| --- | --- |
| `src/game.rs` | Deterministic rules engine and per-player views; no rendering or sockets |
| `src/net.rs` | Authoritative host, bounded JSON framing, invites, reconnect, and client worker |
| `src/main.rs` | Desktop application, lobby, gameplay controls, and dialogs |
| `src/self_play.rs` | Offline multi-seat control and automatic hand switching |
| `src/ui.rs` | Procedural card art, table rendering, and immediate-mode UI |
| `src/table_view.rs` | Camera presets, tabletop geometry, ray picking, and layout |
| `src/card_motion.rs` | Card-flight animation and private-face handling |
| `src/clipboard.rs` | Platform clipboard access and text-field sanitization |
| `src/bin/server.rs` | Optional console host |
| `scripts/` | macOS and Windows packaging |

There is no public backend to deploy, and all game artwork is generated in code.
