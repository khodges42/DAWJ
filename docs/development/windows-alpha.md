# Windows release-profile alpha builds

This procedure produces a **local development-alpha staging folder**, not a
signed installer, public release or proof of live-show readiness. Use Windows
11 x64, the pinned Rust 1.93.0 MSVC toolchain, C++ Build Tools/Windows SDK,
WebView2, Git, Node with the built-in test runner, and Python.

From the repository root:

```powershell
# Fetch locked Windows dependencies, run checks, build both optimized executables:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build_windows_alpha.ps1
# With cached dependencies, also validate the actual staged binaries on HyperX:
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build_windows_alpha.ps1 -Offline -Smoke -Output HyperX
```

The script verifies the host/toolchain, formatting, workspace check, Clippy,
workspace tests, frontend tests, Python checks, licensing inventory, documentation
links and generated IPC fixtures. It forces a known `target` directory so an
inherited `CARGO_TARGET_DIR` cannot cause copying an unrelated stale executable.
Both packages are built from the same lockfile and source tree.

Each run creates a new timestamp/revision directory under `target/alpha-windows/`;
existing folders are never recursively removed or overwritten. It contains:

- `dawj.exe`: primary Tauri alpha, with frontend assets embedded.
- `dawj-egui.exe`: native fallback.
- `LICENSE`, `NOTICE`, the dependency inventory and local alpha instructions.
- `build-manifest.json`: revision, tracked-dirty flag, compiler, artifact SHA-256,
  software-check status, whether native smoke ran, and redistribution-not-cleared status.

`-Smoke` opens actual audio and requires a unique named output match. It never
falls back to the system default. The runner tests primary, fallback and primary
safe-mode startup, including the default application's native skin-reset routine,
both decks, software output restart and clean shutdown. User skin data is backed
up/restored by smoke; it does not intentionally erase the user's library.
Without `-Smoke`, the manifest explicitly records native smoke as not performed.

The same runner can be invoked directly after a manual release build:

```powershell
cargo build --release -p dawj -p dawj-egui --locked --offline
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/smoke_frontends.ps1 -Configuration Release
```

## Launch and recovery

Double-click `dawj.exe`, or run it from PowerShell. Select the intended output
before starting audio. The egui fallback can be launched independently; the two
processes do not share an audio session and simultaneous device use depends on
the driver. WebView2 is a prerequisite for Tauri, not bundled by this procedure.

```powershell
.\dawj.exe --safe-mode
.\dawj.exe --reset-skin
.\dawj-egui.exe
```

Safe mode ignores the saved skin for that run. Reset clears local user skin state.
The native recovery accelerator remains outside skin CSS; the menu bar is hidden.
Use Restore default interface in Audio settings or Ctrl+Alt+Shift+R. Keep the
existing identifier/profile paths when making future branding changes.

## Before sharing or installing elsewhere

Optimized `--release` is a compiler profile, not distribution clearance. Review
[licensing](licensing.md), gather dependency/font texts and corresponding source,
resolve original-design asset redistribution rights, and document native runtime
requirements. Match the source to the manifest; a dirty build needs its exact
uncommitted changes too. Validate a clean Windows machine, runtime installation,
signing/installer behavior and upgrade/uninstall/data retention before publishing.
No automatic ZIP, upload, GitHub release, push or merge occurs here.
