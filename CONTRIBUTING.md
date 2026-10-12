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

### README gameplay GIF

The preview in `docs/media/gameplay.gif` uses scripted legal moves through the
real rules engine and renderer. It is not a bot mode or a multiplayer recording.
With a graphical desktop available, capture 180 frames (12 seconds at 15 fps):

```sh
# macOS; keep the game window unobscured and don't interact during capture.
UNO_GAMEPLAY_DIR=/tmp/uno-gameplay cargo run --release --locked --bin uno
python3 -m venv /tmp/uno-gif-venv
/tmp/uno-gif-venv/bin/pip install Pillow
/tmp/uno-gif-venv/bin/python scripts/make-gameplay-gif.py /tmp/uno-gameplay docs/media/gameplay.gif
```

On Windows, set `$env:UNO_GAMEPLAY_DIR` as for the screenshot hook above, then
remove it afterward. The encoder takes the frame directory and output GIF path;
it requires Python with Pillow installed. It scales to 960×600 and uses a shared
palette for stable colors. Inspect the result before committing; keep raw frames
and Python environments outside the repository.

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
Apple Silicon, and macOS Intel, then uploads workflow artifacts. Downloaded
workflow artifacts contain the package ZIP. Branch pushes and pull requests do
not publish releases.

### Publishing a release

After merging the changes you want to ship, update `Cargo.toml`, `Cargo.lock`,
and the macOS version fields in `packaging/macos/Info.plist`, then commit those
changes. Tag that commit and push the tag, for example:

```sh
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

Pushing a `v*` tag runs the same checks and native packaging jobs. Only after all
three platforms succeed does CI create a GitHub Release with the three ZIPs,
`SHA256SUMS.txt`, and automatically generated release notes. Tags containing a
hyphen (for example, `v0.2.0-rc.1`) are marked as prereleases and do not replace
the latest stable download links in the README. Use a new version tag for each
release; the job does not overwrite an existing release.

Publishing uses GitHub's built-in `GITHUB_TOKEN` with `contents: write` scoped to
the release job; no personal access token or extra secret is needed. Repository
or organization policies must allow that permission. The README download links
work after the first stable release is published. Check the **Actions** tab for
build failures before sharing a new release.

Packages are **not distribution-signed or notarized**; macOS packaging applies
only a local ad-hoc signature. Do not disable platform security protections.
Public binary distribution should use appropriate signing and notarization.

When changing the version, update `Cargo.toml`, `Cargo.lock`, and the macOS version
fields in `packaging/macos/Info.plist`. Keep generated packages, screenshots,
credentials, and local build output out of Git. Curated README media in
`docs/media/` is the exception.

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
| `src/gameplay_capture.rs` | Opt-in scripted gameplay capture for README media |
| `src/bin/server.rs` | Optional console host |
| `scripts/` | macOS/Windows packaging and gameplay GIF encoding |

There is no public backend to deploy, and all game artwork is generated in code.
