# Packaging and artifact review

`.github/workflows/release.yml` runs manually from Actions or when a `v*` tag is pushed.
It builds `aarch64-apple-darwin` (Apple Silicon) and `x86_64-apple-darwin` (Intel) separately
on macOS runners with Rust stable and Node 22 (minimum supported Node version: 22.12).
These are not universal binaries. Tags do not automatically change application versions.
Before tagging, maintainers must review version consistency in package.json, package-lock.json,
src-tauri/Cargo.toml, Cargo.lock and tauri.conf.json.

## Build-only policy

The workflow uses the official `tauri-apps/tauri-action@v1` with explicit `--target` and
`--bundles app,dmg`. `beforeBuildCommand` already builds the frontend. No `tagName`,
`releaseName` or `releaseId` is passed, so the action does not create or upload to a GitHub Release.
Only workflow artifacts are uploaded. Repository contents permissions are read-only, checkout
credentials are not persisted, and no signing, vault, passphrase or personal access secrets are
injected. There is no pull-request or pull_request_target trigger in this packaging workflow.
Do not add privileged publication steps to untrusted pull-request builds.

Each successful matrix job uploads `Pelago-<target>-unsigned` with a `.dmg` and a
`.app.tar.gz`. The tar preserves application bundle permissions and symlinks that a raw artifact
upload would not preserve. Download from the completed Actions run within the 14-day retention
window. Artifact retention is not a permanent release channel or backup.

These builds have no Apple Developer ID signature or notarization. Toolchain-generated ad-hoc
signatures, if present, do not establish publisher identity. Gatekeeper can block downloaded builds.
Do not disable Gatekeeper, remove quarantine or otherwise bypass macOS security checks.
For development, build reviewed source locally. Public distribution should wait for a separately
reviewed signing and notarization process. This workflow does not provide auto-updates.

## Maintainer verification

1. Run contributor checks before dispatching or tagging a reviewed revision.
2. Confirm both architecture jobs succeed and both expected files exist in each artifact.
3. Inspect and test the appropriate build on disposable projects on each architecture.
4. Verify app launch, onboarding and filesystem operations before calling a build usable.
5. Treat encrypted parking as in development until its own destructive-operation and recovery
   acceptance checks pass. A successful packaging job is not evidence of data safety.

## Headless local packaging

To avoid Finder/AppleScript and desktop-window movement during local packaging:

```bash
npm run tauri build -- --bundles app
bash scripts/package-dmg.sh
```

The helper copies the built app into disposable staging, adds an Applications symlink, creates a
plain compressed disk image using `hdiutil`, and verifies its checksum. It derives the output name
from the built version and architecture and refuses to overwrite an existing image. Set
`PELAGO_BUILD_SCRATCH` to choose staging storage, or pass app/output paths as the first/second
arguments. It does not launch the app or mount/open the finished image in Finder. This differs
from Tauri's styled DMG packaging used by the hosted workflow.

Local Apple Silicon app building and headless DMG creation/checksum verification have passed.
Hosted runner execution, the workflow's styled DMG creation, Gatekeeper behavior and native launch
on both architectures still require actual CI and device testing. Live GitHub parking acceptance
is blocked because `gh` authentication is unavailable. Mock UI checks and local bare-Git archive
tests do not substitute for an authenticated upload/restore run. See [validation](VALIDATION.md).

## Upstream sources

Consulted November 10, 2026:

- Official Tauri GitHub pipeline guide: `https://v2.tauri.app/distribute/pipelines/github/`
- Official action inputs and build-only mode: `https://github.com/tauri-apps/tauri-action`
- Official Vite prerequisites: `https://vite.dev/guide/`

The Tauri example includes publishing permissions and release inputs. This repository deliberately
omits those parts. Action version updates should be reviewed separately, preferably pinned to
reviewed immutable commit SHAs when adopting a stricter supply-chain policy.
