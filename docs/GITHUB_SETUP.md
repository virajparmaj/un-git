# GitHub setup and feasibility checks

This guide follows the approved [build plan](BUILD_PLAN.md). The current implementation
is the local-engine and Inspector developer preview described in
[implementation status](IMPLEMENTATION_STATUS.md). It can authorize an account, list
installation repositories, read observed contribution data, and import repository
history into an isolated local snapshot. It cannot publish a rewrite or restore remote refs. Registering
a Writer app or enabling the reserved `writer` feature does not unlock those actions.

## 1. Register the Inspector app

Use a personal GitHub.com account with a verified primary email. In **Settings →
Developer settings → GitHub Apps → New GitHub App**, use these settings:

| Setting | Development value |
| --- | --- |
| Name | A unique name such as `un-git Inspector <your-login>` |
| Homepage URL | The public project repository URL, or the app owner's account URL |
| Callback URL / Setup URL | Leave blank; this client uses device flow |
| Expire user authorization tokens | Enabled |
| Request user authorization during installation | Disabled; authorize explicitly from the desktop or CLI |
| Enable Device Flow | Enabled |
| Webhooks / Active | Disabled; this client has no webhook server |
| Where can this app be installed? | Only on this account for the feasibility run |

GitHub documents device-flow registration, optional callback URLs, expiring tokens,
and disabling webhooks in its [registration guide](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/registering-a-github-app).

Set only these permissions:

| Permission group | Permission | Inspector | Separate Writer, later |
| --- | --- | --- | --- |
| Account | Email addresses | Read-only | Read-only |
| Repository | Metadata | Read-only | Read-only |
| Repository | Contents | Read-only | Read and write |
| Repository | Pull requests | Read-only | Read-only |
| Repository | Administration | No access | Read-only |
| Repository | Workflows | No access | No access until the approved experiment establishes a supported case |
| Other account/repository/organization permissions | All others | No access | No access |

Create the app and record its **Client ID**, not its numeric App ID. Do not create,
download, or place a client secret, private key, personal access token, or installation
token in this repository, `.env`, command arguments, or application settings.
Device-originated user tokens support refresh without a client secret. Expiration
must remain enabled because the adapter expects the access/refresh pair. See GitHub's
[device-token flow](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app)
and [refresh contract](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens).

Open the app's **Install App** page, choose the personal account, select **Only select
repositories**, and include only the explicit disposable fixtures for the first run.
Account authorization and installation are separate: signing in alone does not grant
repository access. A user token has the intersection of the user's and installed
app's access. See [GitHub's user authorization model](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/authenticating-with-a-github-app-on-behalf-of-a-user).

## 2. Connect the current preview

Use the checked-in Node and Rust toolchain pins, Python 3.11+, local Git, Xcode, and
macOS Keychain. Follow the README's setup steps first and run `npm run doctor`.

In the native app, open **Connect to GitHub**, enter the public Inspector client ID,
start sign-in, and enter the displayed code on GitHub's device page. Complete app
installation separately if the repository list is empty. The browser-only Vite
preview cannot use native Keychain or local Git.

The equivalent CLI flow is:

```sh
cargo run -p un-git-cli -- auth login --client-id YOUR_PUBLIC_INSPECTOR_CLIENT_ID
cargo run -p un-git-cli -- auth status
cargo run -p un-git-cli -- repositories
cargo run -p un-git-cli -- calendar
cargo run -p un-git-cli -- calendar --year 2024
```

`calendar` defaults to the current UTC year. Historical requests must be between
2008 and the current year. `UN_GIT_INSPECTOR_CLIENT_ID` is an alternative public-ID
configuration; export it in the environment before launching. Copying `.env.example`
does not itself guarantee that the native Rust process loads the file.

The client ID is ordinary configuration. Access and refresh credentials reside in
one macOS Keychain item for that client ID; the UI receives neither credential.
The device code stays in the Rust client's in-memory session. Losing the process
requires a new device flow. CLI and desktop share a credential lock under the user's
Application Support directory, independent of operation storage. An alternate
`--data-dir` isolates operation files and snapshot caches, but does **not** isolate
the Keychain account or its refresh lock for that App client ID.

`auth status` reports locally stored authorization as `stored_not_verified`; it does
not prove a grant is still valid. `repositories` performs a live identity/access
check. Repository inventory is separate from fetching Git objects: choose **Import
and inspect** to download a new snapshot, or use the local repository picker for
existing clones. CLI `import <repository-id>` returns the saved snapshot path;
`scan <returned-repository-path>` opens its history.

### Add repositories and inspect a contribution day

In **Repositories → Manage repository access**, open GitHub's installed-app settings.
Choose **Configure** beside the Inspector, retain **Only select repositories**, add the
repositories to inspect, and save. Return to un-git and choose **Refresh access**.
Repository selection and App permissions are separate settings; keep Inspector permissions
read-only. Public contribution aggregates can mention repositories outside the installed
list, so a calendar row does not prove the App has repository-content access.

In **Calendar**, load a snapshot, choose a day, and choose **Inspect commits for this day**.
Choose **Refresh access → Import and inspect** beside an installed repository, or
open an existing local clone with **Browse**. The inspector uses each commit's original author
date and offset. It does not infer exact SHAs from GitHub's aggregate count.

Choose the exact email's day-selection button, or individual commit checkboxes. Confirm
the identity, review the plan, and prepare the local candidate. Changing a date filter
does not change the selection; selected commits outside the filter remain disclosed.
Selecting a day's email group explicitly replaces the selection and clears old approval.

Preparation changes attribution metadata while preserving the selected commits' file
changes and all mapped historical snapshots. It creates a backup and independent
candidate, checks tree equality and topology, and leaves the source repository intact.
This is not a revert, drop, or squash. Publication remains unavailable in this preview;
no local action removes GitHub contribution events. Even after future publication,
non-commit activity and GitHub indexing can leave contributions visible.

Every import creates a new snapshot; importing again does not update an older snapshot
or its plans. The backend rechecks installed Contents access and repository identity,
compares advertised refs before and after fetching, verifies object connectivity, and
retains failed or interrupted downloads separately. The Inspector's HTTPS transport
has fixed read operations only. Tokens pass from Keychain through a private in-memory
credential channel; no token is saved in Git configuration or the snapshot.

`auth logout` deletes the local credential and pending sign-in sessions. It does
not uninstall the app or revoke GitHub's authorization. For revocation tests, use
GitHub account settings explicitly, then reconnect when needed.

## 3. Inspector feasibility run

Provision fixtures through the account owner before testing. Record their full
`owner/name` and immutable repository IDs. Use synthetic history without private
source code. Keep an unselected fixture to test access boundaries. No fixture
creation, installation changes, revocation, or remote history changes are performed
by the application during this checklist.

| Check | Procedure and required evidence |
| --- | --- |
| Configuration | Record public client ID, app owner, permission set, selected repository IDs, app build, API version, macOS, and test time. Record no credentials. |
| Login and cancellation | Complete one login; deny a second attempt; allow a code to expire; close/reopen the app during a pending flow. Each result is explicit, and reopening requires a fresh code. |
| Polling | Confirm early polls remain local and `authorization_pending` continues at the server interval. Exercise `slow_down` in the automated/mock harness; do not deliberately flood GitHub. |
| Installation boundary | Authorize before installing: account authorization succeeds but access remains incomplete. Install only the chosen fixtures, reload repositories, and verify the unselected private fixture is absent. |
| Capability evidence | Compare observed permissions with registration and the user's access. Unknown or inaccessible permission data stays unknown; it never establishes write eligibility. |
| Public calendar | Compare the returned current-year observations and one historical year with GitHub at a recorded time. Keep aggregate observations separate from local candidate SHAs. |
| Private/restricted coverage | Include a selected private fixture and unselected private activity; test private-contribution visibility on/off using the owner's settings. Record surviving counts and limitations. A zero restricted count does not prove full private coverage. |
| Day details | Open a contribution day and compare repository/day aggregates. A bounded repository-list mismatch or missing detail must remain partial; do not synthesize missing SHAs or category counts. |
| Pagination | Exercise installation/repository multi-page fixtures and the collection's bounded repository grouping using recorded responses. Do not create hundreds of live repositories merely to test pagination. |
| Email evidence | The core `emails()` API and CLI `identities` return the account's verified/unverified flags unchanged. The selection panel marks only an exact verified match as ownership evidence; explicit edit confirmation remains required. |
| Refresh | After the real access token expires, issue simultaneous desktop and CLI reads, including an alternate data directory. Confirm one serialized rotation succeeds and both callers recover using the stored pair. Test expired refresh and interrupted rotation with mocks; never dump Keychain values to inspect them. |
| Revocation and uninstall | Revoke the user grant, then reinstall/re-authorize as needed; separately remove the installation. Subsequent reads must show denied/missing access and retain existing local artifacts. |
| Keychain denial and offline | Deny Keychain access and disconnect networking in controlled local runs. No fallback token file is created; the UI offers an actionable error and retry. |
| Diagnostics | Inspect the JSON/status/report output for token/device-code leakage, raw remote error echoes, and misleading success claims. Export only a reviewed redacted result. |

The current mocked adapter tests cover response parsing, unknown-versus-zero counts,
restricted/partial data, permission intersections, safe error classes, expiration,
local polling, lock coordination, and secret-free status/logout. They do not prove
live GitHub permissions, private-calendar coverage, or real refresh rotation.

```sh
cargo test -p un-git-core github::
```

## 4. Separate Writer setup and disposable tests

Create a **second registration** when the Writer implementation is ready. Reuse the
device-flow and expiration settings above with the Writer column's permissions;
keep its client ID and authorization separate from Inspector. Do not put the Writer
ID into `UN_GIT_INSPECTOR_CLIENT_ID` to try to unlock publication.

GitHub supports HTTP Git authentication with user tokens and Contents permission;
workflow files introduce the separate Workflows permission. The unchanged-workflow
rewrite case still needs the approved experiment. See
[Git access permissions](https://docs.github.com/en/apps/creating-github-apps/registering-a-github-app/choosing-permissions-for-a-github-app#choosing-permissions-for-git-access).
Classic protection inspection requires Administration read, whereas branch rules
must also be inventoried. See the official
[branch protection endpoint](https://docs.github.com/en/rest/branches/branch-protection#get-branch-protection)
and [branch rules endpoint](https://docs.github.com/en/rest/repos/rules#get-rules-for-a-branch).

These are acceptance scenarios for the forthcoming Writer harness, **not commands
available in this preview**. Every remote-changing experiment needs an explicitly
authorized disposable target and a recorded baseline/backup. Use fresh fixtures for
destructive scenarios; no real project history belongs in this matrix.

| Fixture / experiment | Required outcome |
| --- | --- |
| Eligible baseline | Personal, owned, non-fork, non-archived GitHub.com repository; one default branch; no tags, PRs/retaining PR refs, workflows, unsupported objects, signatures, LFS, or submodules. Fresh complete inventory and permissions succeed. |
| HTTPS credentials | Fetch and leased push use a controlled Keychain-backed credential channel. Tokens never enter URLs, arguments, logs, artifacts, SQLite, or React. Missing/revoked credentials fail without another helper or interactive fallback. |
| Exact candidate | Publish only the immutable, completely validated candidate that was reviewed; use its exact target OID and explicit expected old OID. Require atomic transport and no wildcard, mirror, implicit-ref, or unconditional-force fallback. |
| Target concurrency | Move the target ref after baseline capture from a separate fixture client. Publication rejects the stale lease and preserves concurrent work. |
| New unrelated ref | Create another branch after preflight. Post-push inventory reports drift/eligibility loss; it does not claim a global repository lock or certain calendar effect. |
| Protection/rules | Separately test classic protection, an active ruleset, missing Administration read, and policy changes between review and push. Unknown checks and rejected writes block; the app never edits policy. |
| Workflow history | Compare no-workflow history, an unchanged workflow tree in affected history, and a workflow introduced/changed in the exported scope using the initial Contents-only Writer grant. Record exact rejection/success. Any unsupported case stays read-only; no automatic Workflows permission escalation or retry. |
| Retained history | Add an open PR and detected PR refs on dedicated fixtures. A retaining ref or incomplete PR-impact inventory blocks publication. Record that forks, hidden refs, and external clones cannot be fully inventoried. |
| Lost acknowledgement | Interrupt after GitHub may have accepted the push. Reconcile refs: all new means published; all old permits a separately requested retry of the same candidate after preflight; mixed/unknown retains the lock and requires investigation. |
| Cancellation/restart | Crash at each durable boundary and reopen. Before push, cancellation is local; after publication intent, resolve remote state before another rewrite. Candidate/bundle tampering and missing artifacts block recovery. |
| Restore | Create a separate reviewed restore plan. Restore only with an explicit lease against expected rewritten tips. New remote work rejects the restore; no automatic rollback is allowed. |
| Attribution policy | Verify the reserved replacement identity on synthetic history. Keep original names, trees, topology, dates, and non-approved message bytes unchanged. Local `.invalid` fixture behavior is not evidence of GitHub attribution behavior. |
| Calendar observation | Record repository ref correctness separately from calendar snapshots. Observe with backoff and resume after restart. The 24-hour point is a checkpoint, not a success deadline; unchanged/increased counts never trigger another rewrite. |

## 5. Evidence and gate closure

The test-only local transport suite now passes explicit atomic/leased publication,
stale-lease rejection, receiver rejection, missing atomic capability, discarded
acknowledgement reconciliation, exact restore, and restore drift scenarios against
generated temporary bare repositories. Run `cargo test -p un-git-core --test
local_transport_contract`. This proves local Git semantics only; there is still no
product push API or live GitHub publication evidence. Read-only HTTPS imports are a
separate Inspector capability; they do not establish Writer transport eligibility.

For each run, retain a redacted record of: scenario; fixture ID; app/tool versions;
permission snapshot; baseline, approved candidate, and observed ref OIDs; backup
checksum/restore result; start/end timestamps; expected result; actual result; and
pass, fail, or inconclusive with reason. Preserve private fixture artifacts locally
under the application's protected storage. Publish only intentionally redacted evidence.

The live Inspector gate closes only after the Inspector checks above pass against
the registered app. A successful Inspector run does not close Writer gates. Writer
availability also requires the complete preservation, publication, interruption,
restore, and observation contract in the approved build plan. Signed distribution
further requires bundled runtime provenance, licenses, signing/notarization, and a
clean-Mac test; current local Git/Python prerequisites do not establish that result.
