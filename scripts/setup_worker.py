"""Create the per-project Python environment and check the vendored worker."""
import hashlib
import pathlib
import subprocess
import sys
import venv

ROOT = pathlib.Path(__file__).resolve().parents[1]
EXPECTED = "67447413e273fc76809289111748870b6f6072f08b17efe94863a92d810b7d94"


def main() -> None:
    """Keep worker dependencies isolated and verify their provenance."""
    if sys.version_info < (3, 11):
        raise SystemExit("Python 3.11 or later is required.")
    source = ROOT / "workers/rewrite/git_filter_repo.py"
    if hashlib.sha256(source.read_bytes()).hexdigest() != EXPECTED:
        raise SystemExit("Pinned git-filter-repo checksum mismatch.")
    target = ROOT / ".venv"
    if not (target / "bin/python3").exists():
        venv.EnvBuilder(with_pip=False).create(target)
    subprocess.run([str(target / "bin/python3"), "-I", "--version"], check=True)
    print("Worker environment ready. No external Python packages are required.")


if __name__ == "__main__":
    main()
