# Pinned local rewrite worker

`git_filter_repo.py` is the unmodified `git-filter-repo` executable from upstream
tag `v2.47.0`, downloaded from
<https://raw.githubusercontent.com/newren/git-filter-repo/v2.47.0/git-filter-repo>.
Its SHA-256 is
`67447413e273fc76809289111748870b6f6072f08b17efe94863a92d810b7d94`.
See `COPYING` for its license. No pip installation or third-party Python modules
are required. Run only through the Rust core, which supplies an isolated process
environment, checks the pinned source, and validates the complete resulting DAG.

`worker.py` is a fixed callback. Its JSON selection is validated data. This
prototype rewrites only the explicitly confirmed author email on exact OIDs.
Names, committer identities, dates, message bytes, and coauthor trailers are left
unchanged. Coauthor-only and committer-only cleanup are not supported. The
`.invalid` replacement is a local fixture policy, not a verified claim about
GitHub calendar behavior. Remote publication remains disabled.

Development uses a checked local Git and Python 3 executable. Bundled signed
runtimes, clean-machine discovery, and GitHub behavior remain release gates.
