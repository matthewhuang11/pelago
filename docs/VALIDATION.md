# Validation record

Local validation on November 10, 2026, on Apple Silicon macOS. This records observed checks,
not a production-readiness or backup guarantee.

## Passed

- `npm run build`: TypeScript and production Vite build.
- `node scripts/ui-smoke.mjs`: real headless Chrome against the **mock** backend. Checked deletion
  acknowledgments, passphrase matching, successful mock parking, missing-vault/preview failures,
  restore failure and clearing passphrases after failure. No desktop browser windows were opened.
- `cargo test --manifest-path src-tauri/Cargo.toml --locked`: **47 passed, 1 ignored, 0 failed**.
  The ignored test intentionally scans user-selected real folders and is opt-in.
- Strict all-targets Clippy and Rust formatting checks.
- Disposable real Git filesystem tests for creation, move/undo, no-overwrite renames, linked
  worktree/external-object-store refusal, corrupt history and persistence errors.
- Encrypted parking through a disposable **local bare Git repo**, using production transport code:
  Git state, ignored/untracked files, wrong passphrases, authenticated tampering rejection,
  chunk reconstruction/missing chunks, pinned remote commits, failed upload, registry errors and
  restore collisions. Small forced chunk sizes exercise multi-chunk logic without claiming GB scale.
- Git scan preserves index bytes and modification time, using `--no-optional-locks`.
- Parked-group guard blocks folder rename/delete that would strand recorded restore paths and
  fails closed if the parking registry is corrupt.
- `npm run tauri build -- --bundles app`: built Apple Silicon `Pelago.app`.
- `bash scripts/package-dmg.sh`: created a plain headless development DMG and passed `hdiutil verify`.
  The app executable is arm64 and its generated Info.plist passes `plutil -lint`.
- Source diff whitespace and repository hygiene checks. Local inspiration ZIP/images, per-user
  settings and generated native bundles are not tracked.

## Artifacts

The local build outputs, not committed source files:

- `src-tauri/target/release/bundle/macos/Pelago.app`
- `src-tauri/target/release/bundle/dmg/Pelago_0.1.0_arm64.dmg`

They are development artifacts without Developer ID signing or notarization. No app was launched
or moved to another desktop during these final checks. The native window/tray runtime remains
unverified by this record.

## Pending or blocked

- Authenticated GitHub create/upload/restore acceptance: `gh auth status` reports no authenticated
  host. Git transport push of the source repository is separate from optional GitHub CLI login.
- Actual multi-GB snapshot performance, disk-space pressure and GitHub quotas/rate limits.
- Native interactive app/tray behavior, Gatekeeper behavior, Intel execution and hosted build
  workflow execution. Local headless packaging differs from the workflow's styled Tauri DMG.
- Personal project migration: no real project folders were moved, parked or deleted. Migration
  requires a separately confirmed batch. No GitHub vault was created during testing.

Parking remains experimental. Read [PARKING.md](PARKING.md) before using it, retain independent
backups and keep the registry and passphrase outside the project. Unsupported links/macOS metadata
are rejected, not silently omitted. Unsaved editor buffers and exact live sessions are not captured.

The toolchain also emits a transitive future-compatibility notice for `proc-macro-error2`. It is
not a current test/Clippy failure. The existing bundle identifier ending in `.app` produces a Tauri
recommendation warning. It was retained to avoid changing the established application data path.
