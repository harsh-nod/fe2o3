# Prechecked Topology Reads CPU Qualification

Status: CPU-qualified on GNU and musl; all 21 recorded commands passed.
Live-source verification and all 14 verifier calibrations passed. This packet
does not establish native execution, formal refinement or performance acceptance.

This packet covers the private path-bound, consumed regular-file observation
used by link-properties discovery. It reuses the previously authenticated
21-command GNU/musl pair-currentness recipe, with the new source and exact test
membership frozen independently by this packet's verifier.

The reviewed observation boundary is described in
`docs/runtime-topology-prechecked-reads-v1.md`, separately digest-bound by the
verifier. The change removes the repeated path inspection for each IO/P2P link
property while retaining every fresh discovery and the opened/post-read identity
checks. Common opens now refuse final-symlink replacement. Their nonblocking
flag lets the descriptor check reject FIFO replacement without waiting for a
writer.

Fourteen new tests cover production wiring, fresh IO/P2P rediscovery, exact
inspection counts, type/size/UTF-8 failures, same-inode and replacement races,
non-following opens and error precedence. Test hooks do not execute in normal
builds. Helper-call counts do not establish syscall timing or a hardware gain.

## Recorded Results

- GNU and musl each passed the same exact 162 KFD tests and 30 runtime tests,
  with no failures or ignored tests in those rosters. The KFD roster adds exactly
  the 14 prechecked-read tests to the prior 148, with no removals.
- Each target passed all four benchmark-example unit tests. The diagnostic
  feature-off example also passed all four tests; these are not GPU executions.
- Strict all-feature/all-target clippy, no-default-feature compilation,
  formatting and diff checks passed.
- The unsafe-source inventory passed five tests, with its one explicitly
  ignored inventory-refresh maintenance test unchanged.
- All 21 command receipts record successful exit and absent process groups.
  Before/after snapshots are byte-identical across 5,573 selected source inputs.
- The source snapshot SHA-256 is
  `9cf73b317f1e2bc3a357d6f8f156f1792cdface87b0ab8ec94b12eb3e79a0a2b`.
  Its base is `fa9413425531458f9ac6b1ceb41d96912a9ae2d4`; the only selected
  changes are `topology.rs` and the added `topology/tests/prechecked_reads.rs`.
- Post-run verifier calibration passed all 14 tests in 1.035 seconds, including
  hostile receipt/roster/source mutations and relocated verification. Its
  execution transcript is external to this sealed packet.

## Replay

The non-overwriting driver is `python3 -I -B qualify.py`. The recorded run must
not be overwritten. Calibrate the verifier on disposable copies with
`python3 -I -B test_verify.py`. Verification is `python3 -I -B verify.py`, adding
`--live` only for the exact qualified source. Historical verification does not
require the current source to remain unchanged. The initial seal is created
with `python3 -I -B verify.py --live --seal` and cannot be overwritten.

This packet claims no GPU execution, native provider compatibility, performance
acceptance, HIP/HSA parity or formal/machine-code refinement. The prior MI300X
pair-currentness performance packet does not measure this newer source revision.
