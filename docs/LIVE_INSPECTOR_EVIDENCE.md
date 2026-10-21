# Live Inspector evidence

Recorded September 28, 2026, using the native macOS developer preview and a user-owned
GitHub App configured for Inspector access. No credentials or personal email addresses
are recorded here. The user performed GitHub authorization and installation selection.

## Observed

- The native app obtained a device code, opened GitHub's device page, and subsequently
  displayed the authorized account. Read requests then succeeded through the native app.
- Installation discovery displayed one installation and the explicitly selected public
  disposable test repository, with its default branch and readable-contents capability.
- The rebuilt native app reported Contents read-only and no excessive installed grants.
  The account remained available after quitting and reopening the rebuilt app.
- The current-year (2026) and previous-year (2025) contribution snapshots loaded with
  observation timestamps. Known zero days were distinct from unavailable days.
- A day with contributions returned repository-level commit aggregates and a separate
  repository-creation contribution. The day total was not treated as an exact SHA list
  or a list of exclusively removable commit events.
- A read-only native import of the explicitly authorized disposable `virajparmaj/test`
  repository completed with a READY snapshot and timestamp. The remote advertised no
  refs; the inspector correctly displayed 0 commits and blocked preparation for empty
  history. No remote commit, ref, setting, or permission was changed. This public,
  empty-repository check does not establish private-repository credential challenges.
- Exact email evidence loaded in the native selection panel. A generated fixture's
  synthetic email correctly showed no verified exact account match; no actual account
  email was copied into this evidence record.
- Native day filtering, explicit email-group selection, plan review, and local preparation
  completed on generated history with all 13 preservation checks passing.
- The rebuilt Instructions page's demo entry point created a fresh six-commit fixture.
  Following its selection and confirmation steps produced 1 selected / 5 rewritten
  objects, 13/13 checks, a displayed backup path, and successful artifact revalidation.
  The native app was left open on Instructions for the user's first run.

## Limits and follow-up

These observations establish a basic native read workflow, not closure of Phase 0 or
the full Inspector gate. Server-reported read-only Contents access was checked, but
the broader permission and access matrix remains incomplete.

A CLI `auth status` call did not return within a 45-second bounded check and was stopped.
No token was printed or inspected. The reason was not established; a Keychain access
prompt is a possibility, not a confirmed diagnosis. Native reads succeeded independently.
Desktop/CLI credential interoperability and concurrent real refresh remain unverified.

Also pending: private/restricted coverage fixtures, positive account-email matching, negative
authorization cases, actual expiration/rotation, uninstall/revocation behavior, offline
native behavior, and comparison with GitHub's profile at a shared observation time.
Mocked/local tests remain separate evidence. Writer publication requires its own
registration, transport, recovery implementation, and disposable-repository tests.

The September 28 build passed 91 automated tests, strict workspace Clippy, formatting,
and native debug `.app` packaging. The actual Git → shell → CLI credential helper
roundtrip used a generated dummy token and a private inherited socket; it exercised
spaces/apostrophes in the executable path and Git's EOF-terminated helper input.
Abrupt app/CLI termination can still leave import children running without the parent
watchdog; Instructions and RECOVERY.md disclose this limitation.

## Content preservation

`crates/un-git-core/tests/content_preservation.rs` generates three commits with text
edits, additions, a deletion, and binary bytes. It rewrites only the middle commit's
author attribution and verifies every mapped tree, complete file snapshot, original
delta, restored backup, final content, and unchanged source. The selected commit's
changes and the later author's changes both remain present. This is local evidence;
it does not establish a particular GitHub calendar outcome.
