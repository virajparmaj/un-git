#!/usr/bin/env python3
"""Fixed local rewrite worker. The manifest is data, never executable Python."""
import hashlib
import importlib.util
import json
import pathlib
import re
import sys

UPSTREAM_SHA256 = "67447413e273fc76809289111748870b6f6072f08b17efe94863a92d810b7d94"


def main():
    if len(sys.argv) != 3:
        raise SystemExit("usage: worker.py <candidate.git> <selection.json>")
    candidate = pathlib.Path(sys.argv[1]).resolve(strict=True)
    manifest = json.loads(pathlib.Path(sys.argv[2]).read_bytes())
    if set(manifest) != {"version", "selected_oids", "confirmed_email", "replacement_email", "refs"}:
        raise SystemExit("invalid manifest fields")
    if manifest["version"] != 1 or not manifest["selected_oids"] or len(manifest["refs"]) != 1:
        raise SystemExit("unsupported manifest scope")
    selected = set()
    for oid in manifest["selected_oids"]:
        if not re.fullmatch(r"[0-9a-f]{40}", oid):
            raise SystemExit("invalid OID")
        selected.add(oid.encode("ascii"))
    original = manifest["confirmed_email"].encode("ascii")
    replacement = manifest["replacement_email"].encode("ascii")
    if not replacement.endswith(b".invalid") or any(c in replacement + original for c in b"\n\r\x00 <>\t"):
        raise SystemExit("invalid email")
    for ref in manifest["refs"]:
        if not ref.startswith("refs/heads/") or any(c in ref for c in "\n\r\x00 "):
            raise SystemExit("invalid ref")
    source = pathlib.Path(__file__).with_name("git_filter_repo.py")
    if hashlib.sha256(source.read_bytes()).hexdigest() != UPSTREAM_SHA256:
        raise SystemExit("pinned upstream checksum mismatch")
    spec = importlib.util.spec_from_file_location("git_filter_repo", source)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    seen = set()

    def edit_author(commit, _metadata):
        if commit.original_id in selected:
            if commit.author_email != original:
                raise ValueError("confirmed author identity mismatch")
            commit.author_email = replacement
            seen.add(commit.original_id)

    args = module.FilteringOptions.parse_args([
        "--force", "--prune-empty", "never", "--prune-degenerate", "never",
        "--preserve-commit-hashes", "--preserve-commit-encoding",
        "--source", str(candidate), "--target", str(candidate),
        "--refs", *manifest["refs"],
    ])
    module.RepoFilter(args, commit_callback=edit_author).run()
    if seen != selected:
        raise SystemExit("not every selected commit was rewritten")


if __name__ == "__main__":
    main()
