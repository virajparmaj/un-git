# un-git

A macOS developer preview for inspecting GitHub contribution attribution and preparing validated local Git history rewrites.

The shared Rust engine powers a CLI and Tauri desktop interface. Repository contents,
backups, and rewrite operations stay on the user's machine.

**Status:** working local-engine and Inspector developer preview. Remote publication is unavailable.

See the [product and build plan](docs/BUILD_PLAN.md) for scope, architecture, milestones, validation requirements, and open technical questions.

The selected aesthetic is **Surgical Precision, refined**: graphite surfaces, restrained mineral lime, Geist + Berkeley Mono, and The Disconnect motif. The [design system](docs/DESIGN_SYSTEM.md) defines palette, typography, heatmap states, components, and motion within the build plan.

The intended release sequence is a local engine prototype, a read-only contribution inspector, then a constrained rewrite release with backup, validation, publication, and recovery.

Changing repository history cannot guarantee a particular GitHub contribution-calendar result.

## Run locally

On Apple Silicon macOS with Xcode, Node (see `.node-version`), Rust (see
`rust-toolchain.toml`), Git, and Python 3.11+:

```sh
npm ci
npm run setup:worker
npm run doctor
npm run desktop
```

Open **Repositories → Create local demo** to inspect a synthetic six-commit history.
Select the historical commit, choose `demo@un-git.test`, confirm the synthetic identity,
review the affected descendants, and prepare a local candidate. The operation shows
the tested backup, exact old/new tip, and preservation checks. Source refs remain unchanged.

The **Instructions** sidebar page walks through the demo, read-only repository import,
day selection, expected validation results, backups, and recovery. Start with
**Instructions → Create demo and start**. Keep the app open while importing; abrupt-exit
cleanup is still a preview limitation described in the guide.

`npm run dev` starts a browser-only visual preview; local Git and Keychain require the native app.
`npm run desktop:build -- --debug --bundles app` creates a local development app bundle.
It includes the worker and fonts, but still requires local Git/Python and is not a signed
public distribution.

## CLI

```sh
cargo run -p un-git-cli -- doctor
cargo run -p un-git-cli -- demo
cargo run -p un-git-cli -- scan /absolute/repository/path --json
cargo run -p un-git-cli -- plan <scan-id> --selection selection.json --json
cargo run -p un-git-cli -- prepare <plan-id>
cargo run -p un-git-cli -- verify <operation-id>
cargo run -p un-git-cli -- preview-report <operation-id>
cargo run -p un-git-cli -- export-report <operation-id>
```

Selection files contain exact original OIDs and explicit identity confirmation:

```json
{
  "commit_oids": ["<full-original-commit-oid>"],
  "confirmed_email": "demo@un-git.test",
  "replacement_email": "removed@un-git.invalid"
}
```

Commands return JSON. Progress and errors use stderr. Exit codes distinguish invalid
input (2), blocked work (3), unknown remote outcome (4), and other failures (1).
Use `--data-dir` to isolate CLI fixtures and operation storage.

## GitHub setup

The native Connect dialog accepts the **public client ID** of an Inspector GitHub App.
Enable device flow and expiring user access tokens. Request email addresses, metadata,
contents, and pull requests read permissions; install on selected repositories.
No client secret or private key belongs in the application. The account and app
installation jointly determine accessible repositories.

Alternatively set `UN_GIT_INSPECTOR_CLIENT_ID` or use `auth login --client-id <public-id>`.
CLI `repositories` and `calendar --year 2026` use the same adapter. Tokens stay in
macOS Keychain. Live integration still requires the feasibility checks in the status document.
`identities` returns exact account-email evidence; `day YYYY-MM-DD` reads contribution
detail. Calendar/day results wrap the saved snapshot with `stale` and `fetch_error`.
See [GitHub setup](docs/GITHUB_SETUP.md) for registration and disposable-test steps.

To add repositories, choose **Repositories → Manage repository access**, configure the
Inspector's selected repositories on GitHub, save, then **Refresh access** in un-git.
Choose **Import and inspect** to download a fresh local snapshot. A calendar day's
**Inspect commits for this day** action carries its date into the inspector.
Day selection groups exact author emails; preparation preserves file contents and
historical trees while changing only approved attribution metadata.

CLI `import <repository-id>` downloads an installed repository into app-owned storage
and returns its path for `scan`. Imports are read-only; local preparation does not
remove events from GitHub's contribution calendar.

## Verification and remaining gates

```sh
npm run check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p un-git-desktop
```

See [implementation status](docs/IMPLEMENTATION_STATUS.md) for verified behavior and
open Writer/distribution gates, [recovery](docs/RECOVERY.md) for retained artifacts,
and [third-party notices](docs/THIRD_PARTY.md) for the bundled worker and fonts.
See [capacity evidence](docs/BENCHMARKS.md) for measured history sizes and enforced limits.
