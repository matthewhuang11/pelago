# Parking and recovery

Parking is experimental. Disposable local bare-Git round trips and failure tests pass. Authenticated
GitHub and actual multi-GB acceptance are still pending. Do not use parking as your only backup.

## What is saved

All project types use a full on-disk tar archive encrypted with age using your passphrase. This
includes `.git`, branches, stashes, index state, ignored files, untracked files and saved `.env`
files. No plaintext WIP branch is used. Ciphertext is split into 32 MiB chunks and uploaded in
bounded batches to a **configured private GitHub vault**. Hosting quotas still apply. A private
vault is optional until you park something. Create one with your own GitHub account and enter
`owner/repository` in Settings. Pelago verifies its privacy and does not silently create a vault.

Pelago fetches the uploaded commit independently, reconstructs and authenticates/decrypts the
archive, and compares its path/content/mode inventory against the source. It saves the recovery
registry before moving the source into a same-filesystem quarantine, checks the tree again, then
removes the local copy. Failures retain the source, quarantine or remote snapshot, depending on
the stage. Read the error carefully for any retained-data location.

Restore pins the recorded commit, authenticates the complete encrypted stream before extraction,
checks the restored inventory and uses an atomic exclusive rename. It does not overwrite any
existing destination, including an empty directory or dangling symlink. The original parent must
exist and cannot resolve through symlinks. A successful restore retains the remote snapshot.
Remote cleanup and automatic recovery discovery are not implemented.

## Before parking

- Keep an independent backup. Save every editor buffer to disk.
- Close editors and stop dev servers, builds, watchers, database processes and terminals using
  the directory. Snapshot checks detect many concurrent changes, but cannot lock every writer.
- Save the passphrase somewhere secure **outside** the project. Pelago never persists it.
- Review whether uploading secrets, even encrypted, is acceptable under your policies.
- Allow substantial temporary disk space for encryption, Git transfer and independent restore
  verification. Insufficient space fails the operation rather than deliberately dropping files.

No promise is made to preserve unsaved buffers, exact editor tabs, terminal scrollback, process
memory, running services or operating-system session state. On-disk files are the snapshot boundary.

## Deliberately blocked cases

Symlinks, hard links, special files, linked Git worktrees, external Git object stores, macOS
extended attributes/resource forks and ACLs are rejected rather than silently omitted. This means
projects with dependency-bin links or virtual environments may not currently park. Ownership and
creation times are not preserved. Do not delete unsupported data merely to get past a warning
unless you have independently backed it up and know how it will be recreated.

Only canonical projects discovered in the configured project/scan folders may be parked. The
filesystem root, home, project collection root and a project containing Pelago's data are not valid
targets. Group changes are blocked when they would strand parked projects. Moves across volumes
and moving linked Git layouts are also blocked instead of using an unverified copy/delete fallback.

## Recovery dependencies and interruptions

Back up `registry.json` securely and separately from the Mac being parked. It maps local paths
to vault refs, commit IDs and inventory digests, and contains sensitive metadata. Keep `config.json`
for preferences. Recovery requires the correct vault access, snapshot metadata and passphrase.
A lost passphrase cannot be reset. Vault access alone does not provide automatic discovery on
another Mac.

A crash can leave `parking.lock` in the data directory. Remove it only after confirming no Pelago
parking operation is running. Do not delete quarantine folders, registry records or vault refs to
reclaim space until you have verified a separate restored copy. If local files and a registry entry
both remain after interruption, inspect the retained data and registry before retrying. Never
blindly overwrite or delete an existing destination to make restore proceed.

Corrupt registries fail closed. Tests cover wrong passphrases, tampering, missing chunks,
remote-commit mismatch, failed uploads, registry persistence errors, unsupported layouts and
restore collisions. These local tests do not replace an authenticated GitHub acceptance check.
