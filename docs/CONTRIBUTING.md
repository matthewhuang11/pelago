# Contributing

Use Node.js 22.12+ and Rust stable with the rustfmt and clippy components, plus macOS Xcode
Command Line Tools. Install dependencies with `npm ci` so the lockfile is respected.

Run from the repository root before proposing a change:

```bash
npm ci
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path src-tauri/Cargo.toml --locked
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
```

CI performs frontend typecheck/build, Rust formatting, tests and all-targets Clippy. Fix formatting
with `cargo fmt --manifest-path src-tauri/Cargo.toml --all`, then review the diff. Do not silence
warnings to hide failures. `--all-targets` includes tests and examples for the host platform, not
all supported CPU architectures. Packaging runs each macOS target separately.

For UI development, `npm run dev` uses the browser preview. Use `npm run tauri dev` for real
backend behavior. Always use a disposable `PELAGO_DATA_DIR` and disposable project directories
for tests that create, move, delete, park or resume files. The environment variable changes the
settings directory, not the filesystem permissions or GitHub identity of your process.

## Browser UI smoke check

With Node 22.12+ and Google Chrome installed, start the Vite preview in one terminal:

```bash
npm run dev -- --host 127.0.0.1 --port 1420 --strictPort
```

In another terminal, set an existing disposable scratch directory and run:

```bash
export JCODE_SCRATCH_DIR="$HOME/Library/Caches/pelago-smoke"
mkdir -p "$JCODE_SCRATCH_DIR"
node scripts/ui-smoke.mjs
```

The script launches headless Chrome with a temporary profile under `JCODE_SCRATCH_DIR` and connects
to the mock UI at `http://localhost:1420`. It defaults to the macOS Google Chrome application path.
Set `CHROME` to another Chrome executable path if necessary. It checks parking acknowledgment and
passphrase gating, missing-vault/preview failures and restore error handling. It asserts that the
page is not connected to the Tauri backend. This is a browser interaction check, not proof of real
archive encryption, filesystem deletion, GitHub upload, native tray behavior or successful recovery.
The script cleans up its temporary Chrome profile on exit. A passing run must be reported separately
from backend and native packaging acceptance.

## Repository hygiene

Do not commit per-user `config.json`, `registry.json`, `PELAGO_DATA_DIR` contents, `.env` files,
credentials, passphrases, private vault snapshots, generated app/DMG outputs, or local `inspo/`
assets and design ZIPs. Gitignore helps prevent accidents but is not a security boundary and does
not untrack files already committed. Inspect `git status --short` and your staged diff before
committing. Keep shared code/configuration distinct from runtime settings.

## Safety and documentation

Filesystem operations must be tested with collisions, partial failures and recovery in mind.
Encrypted parking is under development: document actual behavior rather than promising a tested
backup or exact editor session restoration. Keep README status aligned with the implementation.
The tray module and stale-project suggestions are currently being integrated. Distinguish code
presence from end-to-end acceptance, and do not describe either as release-ready before validation.
Packaging artifacts are unsigned development outputs. Do not suggest disabling OS security checks.
