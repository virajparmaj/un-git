# un-git implementation instructions

Read `docs/BUILD_PLAN.md` and `docs/DESIGN_SYSTEM.md` before changing behavior.
They are the approved architecture, safety contract, and selected aesthetic.
Read `docs/IMPLEMENTATION_STATUS.md` for implemented behavior and open gates.

- Tauri 2 + React 18 + strict TypeScript; one shared Rust core and a thin CLI.
- Keep SQLite migrations and operation schema versions explicit. Flush/finalize artifacts
  before journal checkpoints; desktop and CLI share repository/credential locks.
- Never expose arbitrary shell execution to React. Keep credentials in Keychain only.
- Inspector has no remote mutation path. The reserved `writer` feature is not permission
  to implement an unconditional push or bypass unverified eligibility/recovery gates.
- All Git tests use generated histories/local bare remotes. Live GitHub publication tests
  require explicitly provisioned disposable repositories; never use real user history.
- Keep observed contribution counts, Git evidence, and attribution estimates separate.
- Preserve exact design tokens, truthful topology, and distinct local/remote/calendar states.
- Use the project `.venv` for Python. The upstream rewrite dependency is vendored and
  checksum-pinned; do not modify it to make preservation tests pass.
- Run `npm run check`, `cargo clippy --workspace --all-targets -- -D warnings`, and native
  build/smoke checks appropriate to the change. Document results and remaining gates honestly.
