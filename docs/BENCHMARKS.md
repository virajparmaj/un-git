# Developer-preview capacity evidence

Measured September 27, 2026 on the current Apple Silicon macOS development host,
Git 2.52.0, project Python 3.11.14, pinned filter-repo 2.47.0, Rust 1.96.0 **debug**
build. These are single-run feasibility measurements, not a performance guarantee.

`cargo run -q -p un-git-core --example benchmark` generates independent temporary bare
repositories with 1,000, 10,000, and 100,000 linear empty-tree commits. It selects one
mid-history author-email edit. Fixtures contain no real source code or user history.

| Reachable commits | Scan | Plan | Complete preparation and validation | Scan JSON |
| ---: | ---: | ---: | ---: | ---: |
| 1,000 | 261 ms | 61 ms | 2,084 ms | 756,986 bytes |
| 10,000 | 1,171 ms | 631 ms | 7,265 ms | 7,578,987 bytes |
| 100,000 | 8,695 ms | 6,387 ms | Blocked by preview limit | 75,889,092 bytes |

The complete benchmark process, including fixture creation, took 31.34 seconds.
`/usr/bin/time -l` reported 387,448,832 bytes maximum resident size and 421,725,000
bytes peak memory footprint. This is the combined benchmark invocation's reported
peak, not a per-phase memory profile. Desktop IPC/rendering and filesystem diversity
need separate large-history measurement before expanding the supported scope.

## Enforced limits

- Inspection: 100,000 reachable commits and 500,000 reachable objects.
- Local preparation: 10,000 reachable commits and 1 GiB of source object storage.
  Larger inspectable histories have a visible preparation blocker.
- Metadata loading: 16 MiB per requested object, 64 MiB total per batch. Commit
  and tree payloads are checked before loading; ordinary source blobs are not loaded
  into the UI. Small blobs are inspected for unsupported LFS pointers.
- Each checked process: 128 MiB per output stream and a 180-second lifecycle deadline.
- Read-only HTTPS import: 1 GiB per-file ceiling and aggregate download watchdog,
  50,000 storage entries, 8 MiB per process-output stream, 20,000 advertised ref lines,
  and a 180-second deadline for each Git request. Import requires 1.25 GiB free at
  admission and stops when monitored free space drops below a 256 MiB reserve.
  The aggregate watchdog checks every 100 ms and can overshoot between checks; it
  is not a strict filesystem quota. Completed imports must also pass scan limits.
- Free space: six times source object storage plus a 256 MiB reserve, checked before
  capture and again before candidate creation. The capacity decision is recorded.

Disk admission is conservative headroom, not a filesystem quota or guarantee against
concurrent disk use or unusually large repacks. An I/O failure retains artifacts and
cannot enable publication. Old backups are never deleted automatically to make room.

The ordinary integration fixtures separately exercise merges, empty commits, multiple
identities, offsets, raw non-UTF-8 messages, stale plans, corruption, and cancellation.
The linear benchmark does not establish performance or correctness for all those
shapes at 100,000 commits. Streaming scan storage and IPC pagination are required
before raising these limits substantially.
