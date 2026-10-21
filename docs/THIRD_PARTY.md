# Third-party assets and runtime dependencies

- Vendored `git-filter-repo` v2.47.0: upstream source, exact SHA-256, and license are
  recorded in `workers/rewrite/README.md` and `workers/rewrite/COPYING`.
- Geist Sans and Geist Mono: locally bundled through pinned Fontsource packages;
  their SIL Open Font License files accompany the desktop assets. Berkeley Mono
  is not downloaded, included, or licensed by this repository.
- Lucide icons and Radix/shadcn-style primitives retain their package license metadata.
- JavaScript and Rust dependency resolutions are fixed by package-lock.json and Cargo.lock.
- Local Git and Python are currently development prerequisites, not redistributed
  runtimes. Redistributing them is a separate provenance, license, signing, and
  clean-machine verification gate.

A product-wide software license has not been selected by the owner. This file is a
dependency inventory, not a license grant for un-git.
