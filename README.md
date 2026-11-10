<p align="center">
  <img src="app-icon.png" width="120" alt="Pelago icon" />
</p>

<h1 align="center">Pelago</h1>

<p align="center">
  <b>A friendly, local-first home for your programming projects on macOS.</b><br/>
  See every project at a glance, jump into your editor in one click, spin up new repos in seconds,
  and park old projects in the cloud to free disk space without losing your place.
</p>

---

## Why

Finder is a bad project manager. Projects end up scattered across `~/Desktop`, `~/Downloads`,
`~/untitled folder 3`, and five different "Projects" folders. Pelago gives you one place that only
cares about code:

- **Groups are real folders.** `~/Code/Startups/my-app` shows up in the *Startups* group. Rename a
  group in Pelago and the folder is renamed on disk.
- **One click to code.** Open any project in VS Code, Cursor, Windsurf, Zed, JetBrains IDEs, Xcode,
  or a terminal.
- **Create without the busywork.** Every step is a toggle: `git init`, GitHub repo (public, private,
  or none), keep it on this Mac or not, which editor to open.
- **Park & resume.** Not touching a 2 GB project for a while? Park it: Pelago saves *everything*
  (uncommitted edits, untracked files, `.env` secrets, current branch) to GitHub, verifies it, and
  deletes the local copy. Click it later and it comes back exactly where you left off.

## No accounts, no database, no server

Pelago is just a desktop app. Everything lives on your machine:

| What | Where |
| --- | --- |
| Your projects | Real folders under your projects root (default `~/Code`) |
| Settings | `~/Library/Application Support/dev.pelago.app/config.json` |
| Parked-project registry | `~/Library/Application Support/dev.pelago.app/registry.json` |
| GitHub access | Your own [`gh`](https://cli.github.com) CLI login. Pelago never sees a token. |

Fork it, run it, and the first-run setup asks for *your* folders and editor.

## Getting started

### Prerequisites

- macOS 11+
- [Node.js](https://nodejs.org) 20+
- [Rust](https://rustup.rs) (stable)
- `git` (ships with Xcode Command Line Tools: `xcode-select --install`)
- Optional, for GitHub features: [`gh`](https://cli.github.com) (`brew install gh && gh auth login`)

### Run from source

```bash
git clone https://github.com/matthewhuang11/pelago.git
cd pelago
npm install
npm run tauri dev
```

### Build a `.app` / `.dmg`

```bash
npm run tauri build
# -> src-tauri/target/release/bundle/macos/Pelago.app
# -> src-tauri/target/release/bundle/dmg/Pelago_<version>_aarch64.dmg
```

## How parking works

| Project type | Where the work-in-progress snapshot goes |
| --- | --- |
| Private GitHub repo | A hidden `pelago/wip` branch in that same repo |
| Public GitHub repo | Your own **private** vault repo (`<you>/pelago-vault`), so half-finished code and secrets never go public |
| No GitHub repo | Your private vault repo |

The vault is created on first use with your own `gh` login, and only if you park something that
needs it. Every user has their own vault. Pelago only deletes a local folder **after** confirming
the snapshot was pushed.

## Roadmap

- [x] Phase 0: scaffold, CI
- [ ] Phase 1: first-run setup, project scanner, launcher UI, editor support
- [ ] Phase 2: new-project wizard, folder-backed groups, safe moves with undo
- [ ] Phase 3: park & resume
- [ ] Phase 4: menu-bar quick open, stale-project suggestions, releases

## Tech

[Tauri 2](https://tauri.app) (Rust) + React + TypeScript. Small, fast, native-feeling.

## License

[MIT](LICENSE)
