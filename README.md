<p align="center"><img src="app-icon.png" width="120" alt="Pelago icon" /></p>
<h1 align="center">Pelago</h1>

A local-first home for programming projects on macOS, built with Tauri 2, React and TypeScript.
Organize projects into real folder-backed groups, scan your projects, open an editor or terminal,
and create projects with optional Git and GitHub setup.

## Status

Project browsing, first-run setup, editor launching, project creation, folder management, menu-bar
quick open and stale-project suggestions are implemented. Encrypted park/resume is **experimental**:
disposable local bare-Git round-trip and failure-path tests pass, but authenticated GitHub parking,
actual multi-GB snapshots and native interactive tray behavior still need end-to-end acceptance.
The frontend build, headless browser UI checks, 47 backend tests and strict Clippy pass. An Apple
Silicon macOS application bundle builds successfully. See [validation evidence](docs/VALIDATION.md)
for the exact checks and remaining limits. Keep independent backups.

## No Pelago login

Local features require no account or login. Optional GitHub features use your own GitHub CLI (`gh`)
authentication. Pelago has no hosted account service. Do not commit your settings, registry,
credentials, archives or local design inspiration to this source repository.

| Data | Default location |
| --- | --- |
| Projects | Real folders under `~/Code`, or your configured root |
| Settings | `~/Library/Application Support/dev.pelago.app/config.json` |
| Parked-project registry | `~/Library/Application Support/dev.pelago.app/registry.json` |
| Optional GitHub authentication | Managed by your own `gh` installation |

## Develop

Requirements: macOS 11+, Xcode Command Line Tools (`xcode-select --install`), Git,
**Node.js 22.12+** (use a supported 22.x release or newer supported even-numbered release),
and **Rust stable**. Vite 8 requires Node 20.19+ or 22.12+, so a generic “Node 20+” is insufficient.
CI uses Node 22 and Rust stable. Optional GitHub operations also require `gh` and `gh auth login`.

```bash
npm ci
npm run tauri dev
```

For UI-only work, `npm run dev` serves the browser preview at `http://localhost:1420`.
Use a disposable data directory and disposable project folders for backend development:

```bash
PELAGO_DATA_DIR="$HOME/Library/Application Support/pelago-dev" npm run tauri dev
```

The data-directory override isolates settings, not arbitrary filesystem operations. Do not point a
test instance at valuable projects. See [contributor checks](docs/CONTRIBUTING.md).

## Parking: experimental encrypted snapshots

Every project, whether public, private or without a Git remote, uses a **full encrypted
on-disk archive** protected by a user-supplied passphrase and stored in a configured **private GitHub
vault repository**. No vault is required for local browsing or creation. Create your own private
vault and enter `owner/repository` in Settings before parking. The old plaintext WIP branch design
is not used.
The archive includes on-disk Git state, uncommitted and untracked files, ignored files and secrets.
Encryption does not make uploading secrets risk-free.

Save all editor buffers and close editors, terminals, dev servers and other processes using the
project before parking. Unsaved buffers, exact open tabs, terminal sessions and running process
state are **not** promised to survive. The backend fetches, decrypts and compares the uploaded
archive against the local tree before local removal. Symlinks (including dependency-bin links),
hard links, linked worktrees, special files and macOS extended attributes/ACLs are blocked rather
than silently discarded. Keep an independent backup, the local registry and your passphrase.

Encrypted archives are split into **32 MiB chunks**, below GitHub's per-file limit, and pushed in
bounded batches. Hosting quotas still apply. Keep a separate backup of the local registry: it records
the mapping between
projects and their vault snapshots. Vault access alone is not a promise of automatic discovery or
recovery on another Mac. Keep the passphrase separately and securely. Lost passphrases cannot be
recovered by Pelago. See [parking and recovery](docs/PARKING.md).

## Build and artifacts

```bash
npm ci
npm run tauri build -- --bundles app,dmg
# Or, to avoid Finder/AppleScript during local packaging:
npm run tauri build -- --bundles app
bash scripts/package-dmg.sh
```

Native build outputs are under `src-tauri/target/release/bundle/macos/` and `bundle/dmg/`.
Explicit `--target` builds add the target triple below `src-tauri/target/`.
The manual/tag workflow produces separate Apple Silicon and Intel `.app` archives and `.dmg`
workflow artifacts. It does **not** publish a GitHub Release or configure an updater.
These are unsigned development builds, not Developer ID signed or notarized distributions.
Gatekeeper may block them. Do not disable Gatekeeper or strip quarantine attributes. Build from
reviewed source for development, or wait for a signed and notarized release.
See [packaging](docs/PACKAGING.md) for triggers, artifact handling and upstream references.

## License

[MIT](LICENSE)
