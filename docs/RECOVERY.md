# Local operation recovery

The current developer preview never pushes to GitHub or changes source repository refs.
Every local operation has a UUID directory under Application Support (or the explicit
`--data-dir` / `UN_GIT_DATA_DIR` directory). Preserve this directory when investigating.

1. Inspect an operation in the desktop Operations view or `un-git inspect-operation <id>`.
2. For a completed candidate, run `un-git verify <id>` or **Revalidate artifacts**.
   Corruption blocks the operation; do not modify the recorded plan or candidate to
   make a check pass.
3. Local cancellation is honored at durable checkpoints. A running subprocess may
   finish before that checkpoint; its output cannot change a remote ref.
4. After an interrupted local preparation, startup marks it `FAILED_LOCAL` and retains
   its files. Scan and prepare a new operation; do not reuse the interrupted directory.
5. Use report preview before exporting the default redacted support packet. Nothing is
   uploaded automatically. Repository paths/names, emails, SHAs, messages, and source
   files are omitted from that packet.

The independent recovery bundle is `operations/<id>/artifacts/backup.bundle`.
Its checksum, restored refs, and full local validation evidence are stored next to it.
It covers fetched Git history, not issues, repository settings, hidden GitHub refs,
external clones, or LFS storage. Do not delete an unresolved operation's backup.

For independent manual inspection, restore the bundle into a new destination:

```sh
git clone --mirror /absolute/path/to/backup.bundle /new/private/recovery.git
git -C /new/private/recovery.git fsck --full --strict
```

This restores local objects only. Remote rollback is not implemented in this preview.
Never substitute an unconditional force push. Any future recorded publication-unknown
state remains blocked in this Inspector until a Writer can reconcile its remote refs.

Credentials are stored in macOS Keychain, not SQLite. Disconnect removes the local
credential; revoke the GitHub App authorization in GitHub settings when needed.

## Read-only imports

Imports use fresh `imports/<uuid>/repository.git` directories and version 1 records.
An import becomes READY only after object/ref checks, final access verification, file
and directory flushes, and durable journal storage. Failed and interrupted directories
are retained; they are not automatically resumed or promoted into validated snapshots.
Retry creates a new directory. Existing scans, plans, and backups remain unchanged.

Keep the app open while importing. Abrupt app/CLI termination can leave Git descendants
running: parent-death supervision and automatic orphan cleanup are not implemented.
The per-file OS size ceiling survives, but the app-managed aggregate-space watchdog and
wall-clock deadline do not. Before retrying, ensure the previous import's Git process has
ended (Activity Monitor can identify the Git process using that import directory).
Do not treat a leftover FETCHING directory as a completed snapshot. Remote refs cannot
be changed by this read-only path. This limitation remains a release-hardening gate.
