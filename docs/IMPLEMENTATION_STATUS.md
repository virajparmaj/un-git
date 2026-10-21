# Implementation status

The approved BUILD_PLAN.md and DESIGN_SYSTEM.md remain the product contract.
This document records implementation evidence; it does not relax their gates.

## Available implementation

- Tauri 2 / React 18 desktop, a thin Rust CLI, and one shared Rust core.
- Local bare/worktree repository inspection, raw commit bytes, exact identity selection,
  deterministic descendant impact, immutable hashed plans, independent baseline and
  candidate storage, verified/restored standalone bundles, and a fixed pinned worker.
- Full local validator checks mapping injectivity/completeness, trees, ordered parents,
  original dates/offsets, exact approved author-email edits, raw messages, ref scope,
  unchanged objects outside the descendant boundary, and object integrity.
- SQLite schema migrations, full-sync checkpoints, cross-process repository locks,
  safe local cancellation, retained interrupted artifacts, and redacted report preview/export.
- GitHub device-flow and Keychain adapter, serialized refresh, installation inventory,
  exact verified-email evidence, yearly observed calendars, and bounded day-level contribution detail.
  Account/App-scoped saved snapshots survive network/rate/server failures with their
  original timestamp and explicit stale status. Revoked authorization never falls back
  to a successful-looking saved response. Credential locks span alternate data directories.
  Native device authorization, installed test-repository discovery, current-year and
  historical calendars, and day aggregates were exercised on September 28, 2026.
  See [live Inspector evidence](LIVE_INSPECTOR_EVIDENCE.md) for limits; private data,
  revocation, refresh, and desktop/CLI Keychain interoperability remain open gates.
  Full GraphQL rate-limit failures retain their retryable classification for saved
  snapshots; mixed access failures cannot use that fallback. Day details expose
  incomplete coverage and API issues even when individual repository rows are complete.
- Graphite/mineral-lime desktop, the Disconnect vector, local Geist fonts, keyboard
  heatmap/day list, virtualized commits, exact identity confirmation, truthful DAG,
  distinct coral Plan selection layer, local operation evidence, and separate
  repository/calendar status text. Selection changes invalidate identity confirmation.
- Repository-access management opens GitHub settings for the user's selection, then
  refreshes the actual inventory. Calendar days link to local inspection; an original
  author-date filter groups exact author emails for explicit day selection. Filtering
  does not select commits or alter observations. Added content-preservation tests use
  real text/binary changes instead of relying only on empty-tree histories.
- Read-only GitHub imports accept an installed repository ID, recheck access, and create
  a fresh app-owned bare snapshot. Ref comparisons, object checks, storage/process
  limits, private credential channels, and durable import records precede ready status.
  Desktop import opens the commit inspector; CLI returns the snapshot path for scanning.
- An in-app Instructions page provides a generated-demo walkthrough, installed test-repo
  import steps, exact day/identity selection guidance, preservation explanations, and
  backup/error recovery. Import locks span alternate data directories. Desktop account
  changes are rejected while an import and its final scan are in progress.
- Bounded metadata/process loading, preview history/object limits, recorded disk-space
  admission checks, nested artifact ownership checks, and process-group deadlines.
- Test-only atomic/leased transport experiments against generated local bare remotes:
  concurrent work, unsupported atomic capability, receiver rejection, discarded
  acknowledgements, exact restore leases, and restore drift. No product push API exists.

## Deliberate restrictions

This is a developer preview of the local engine and Inspector, not Cleaner alpha.
There is no publish command, restore transport, or force-push UI. The `writer` Cargo
feature is reserved and does not enable remote writes. The local engine runner permits
file transport only; a separate fixed HTTPS path performs Inspector ref reads and fetches.

Local preparation changes confirmed **author email only**. Names, committer metadata,
messages, coauthor trailers, trees, and dates are preserved. Committer-only and coauthor-only
cleanup are unavailable. A `.invalid` replacement is a local test policy; its GitHub
attribution behavior is unverified.

Preparation requires one branch, no additional refs/tags, and rejects signatures,
extended headers, workflows, LFS, submodules, alternate object storage, grafts,
shallow/partial histories, and unsupported formats. Such repositories can still be
inspected where their objects are readable. Unsupported/missing objects can stop a scan.

## Gates still open

| Gate | Evidence needed before completion |
| --- | --- |
| Live Inspector | Basic native login/public reads pass; private/restricted coverage, revocation, refresh, complete permission inventory, and CLI Keychain tests remain |
| Git transport and Writer | Read-only import tests are separate from Writer evidence; separate Writer registration, explicit disposable repository, private HTTPS coverage, protection, rules, PR-ref, workflow and atomic-lease experiments remain |
| Writer transaction | Reviewed approval binding, fresh remote preflight, durable push intent, remote reconciliation, concurrent-push and lost-acknowledgement tests |
| Restore and observation | Reviewed leased restore, drift rejection, rate-aware calendar observation/resumption, discrepancy close-out |
| Broader identity support | Byte-safe terminal coauthor parsing/edits; live verification of account/noreply evidence |
| Large-history expansion | Current limits and 1k/10k/100k measurements are recorded in BENCHMARKS.md; broader DAG/file-shape and native IPC benchmarks remain |
| Packaging | Fully bundled Git/Python helpers, provenance and license review, clean-Mac discovery; current app includes worker resources but uses local Git/Python |
| Import interruption | Abrupt app/CLI exit can leave Git descendants running without the parent watchdog; parent-death supervision and orphan cleanup remain, as documented in RECOVERY.md |
| Public distribution | Supported macOS matrix, Developer ID signing/notarization, license decision and release identity |
| Typography | Berkeley Mono redistribution rights; Geist Mono remains the bundled fallback |

GitHub App configuration, disposable-remote authorization, Apple signing access, and
the software-license decision are external prerequisites. No real repository history
has been used for publication experiments.

## Phase progress and next execution order

1. **Phase 0 — partially proven.** Local fidelity, baseline restoration, bounded
   performance, and local Git lease/atomic experiments pass. Run the registered-App
   matrix in [GITHUB_SETUP.md](GITHUB_SETUP.md) before approving live transport scope.
   Prove bundled Git/Python discovery and clean-Mac execution before distribution.
2. **Phase 1 — implemented for the developer preview.** Workspace, storage schema v1,
   shared core/CLI, typed desktop boundary, restricted subprocesses, fixture builder,
   doctor, real scan, and the approved visual foundation are present.
3. **Phase 2 — implemented for the restricted author-email mode.** Frozen plans,
   independent bundle restoration, deterministic candidate preparation, full local
   validation, corruption rejection, cancellation, and interrupted-state retention work.
   Expand the fixture matrix before widening metadata edits or supported histories.
4. **Phase 3 — Inspector implementation present; live gate open.** Native calendar,
   day detail, selected installation repositories, verified-email evidence, local
   history inspection, manual advisor, offline snapshots, and diagnostics are wired.
   Read-only snapshot imports are implemented. Exhaustive native/accessibility checks,
   live refresh and private visibility remain open verification work.
5. **Phase 4 — not implemented in the product.** Following the live feasibility gate,
   implement separate Writer credentials, complete eligibility inventory, candidate-bound
   review, fresh preflight, durable publication intent, exact atomic leased transport,
   remote reconciliation, reviewed restore, and resumed calendar observation in that order.
   Port the test-only local transport scenarios to this complete operation state machine.
6. **Phase 5 — partially prepared.** CI, icons, bundled font/worker notices, recovery
   guide, redacted reports, and a native debug bundle exist. Remaining work includes
   storage cleanup/export controls, bundled runtimes, license/provenance review, declared
   macOS test matrix, release identity, Developer ID signing and notarized DMG.

Routine defaults remain those in the approved documents: Apple Silicon first, an
Inspector-only build, separate Writer authorization later, selected repositories,
no Workflows permission escalation, manual email detachment, and Geist Mono until
Berkeley Mono redistribution rights are available. These are not new blockers.

## Verified local evidence

See the test commands in the README. Tests use synthetic histories and temporary
directories. No credential, GitHub account, or network mutation is required.
The six-commit fixture includes multiple identities, a selected old commit, descendants,
a two-parent merge, an intentionally empty commit, offsets, coauthors, and an old-SHA
message reference. Repeated preparation must produce the same candidate refs.

On the current development host, `npm run check`, strict workspace Clippy, the native
Tauri tests, and debug `.app` packaging have passed. The current
verification run passed all `npm run check` stages after correcting the credential
helper's EOF handling (only the Rust stage needed repeating), strict workspace Clippy,
and both native tests. The suite includes 34 frontend tests, 40 Rust core unit tests,
2 CLI credential-helper integration tests, 8 local transaction/content integration
tests, 5 local transport experiments, and 2 native tests. Local tests use generated
data only; the CLI helper test uses a dummy token and no Keychain/network access.
The rebuilt app's Instructions walkthrough passed native demo preparation and
revalidation. A read-only import of the authorized public test repository completed;
that repository was empty, so the app correctly showed zero commits and preparation
blockers. No remote history was changed. Private authenticated Git fetch remains unverified.
The native synthetic flow has also been exercised through selection, local preparation,
13 preservation checks, operation reopening, and diagnostic preview. Public distribution
is not established by these results. Separate live Inspector evidence is linked above.

The complete current capacity measurements and enforced limits are in
[BENCHMARKS.md](BENCHMARKS.md). No release claim is inferred from the debug bundle.
