# Topology Link-Directory CPU Qualification

CPU qualification passes. No native performance result is claimed.

The production change removes one unused `ensure_directory` call immediately
before `read_directory`, which performs the same non-symlink/directory check.
It removes one path metadata query per I/O/P2P link per discovery. Every link
listing, property read, opened/closing file metadata check, node/root identity
check, generation bracket and full-host comparison remains. No snapshot is
cached, and no currentness contract is replaced with a weaker observation.

Three new tests check the ordered Rust I/O boundaries for every link in both
sets, fresh rediscovery after a link change, legacy stable contents/errors,
regular/FIFO/symlink/broken-symlink/loop refusals, and malformed/missing/extra
link contents. The hooks count Rust boundaries, not measured kernel syscalls.
Existing diagnostic-mode equivalence and currentness tests remain in the full
library runs. No stronger atomic-snapshot or path-replacement guarantee is added.

The selected-source delta from the signed hot-batch CPU map is exactly three
paths: `topology.rs`, its test-module registration and the new test file.
The fixed campaign uses cold private Cargo targets, offline/locked Cargo,
nightly 2026-04-03, two build jobs and four test threads. All thirteen command
stages pass, with 3,956 unchanged selected source inputs and the exact runner
bound before and after execution. Results on each GNU/musl target are:

- Three focused tests pass, with 1,558 other KFD tests filtered out.
- The complete KFD library passes all 1,561 tests, with no ignored tests.
- The complete runtime library passes 1,365 tests; the same twenty
  hardware-dependent tests remain ignored and are not hardware evidence.

All 73 doctests, formatting, strict all-feature/all-target Clippy and closing
tool-identity checks pass. Replay requires the complete named library rosters,
not only summary counts. It authenticates the earlier full-library and newer
topology transcripts from signed source, then adds the exact three new tests.

`cpu1` was interrupted during initial compilation after review found two runner
provenance gaps: a helper was reread after hashing, and the prior source map was
not authenticated. Its original runner, matching source brackets and failed,
reaped receipt remain retained. Its 96,542,720-byte owned cache was removed.
The corrected runner executes the exact hashed helper buffer and authenticates
the prior map against its signed Git-blob digest. `cpu2` is the accepted fresh
attempt. Its 887,775,232-byte owned cache was removed after byte-exact collection.
Both cleanup receipts record path-accounted allocated bytes, totaling
984,317,952 bytes, and replay checks that both exact cache paths remain absent.

The raw manifest contains 61 artifacts, including both frozen runners, command
receipts, output streams, source brackets and cleanup records. `audit1` passes
replay and all eighteen verifier tests, including rehashed hostile-record
controls. `audit2` brackets this final README and repeats those commands; its
terminal receipts and before/after input hashes are retained separately.

Replay from the repository root:

```sh
python3 -I -B docs/evidence/dev-topology-link-directory-cpu-2026-09-24/verify.py
python3 -I -B docs/evidence/dev-topology-link-directory-cpu-2026-09-24/test_verify.py
```

The candidate has not been used in the separate unchanged hot-batch native
campaign. A future baseline/candidate native comparison must use fresh matched
builds and the same whole-host freshness contract. Removing filesystem work
does not by itself prove a latency gain, HIP/HSA parity or formal production
refinement. A1/A2 and broader accepted checkpoints remain unchanged.
