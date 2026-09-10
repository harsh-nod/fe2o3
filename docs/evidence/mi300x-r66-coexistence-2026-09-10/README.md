# MI300X R66 Coexistence: Rejected Qualification

**Not accepted.** The first signed R66 live campaign failed its native-custody
observation. No eight-cell pass, second run, physical-overlap measurement,
native-memory-budget result or HIP/HSA comparison is established.

## Exact Attempt

- Source: signed `bd8aa3ded708ecd6e799a803bee8e9c7ccd6c91a`.
- Host: authorized `mi300x`; selected GPU 1, UID `0xab83d2ffef0d3cdf`,
  PCI `0000:26:00.0`. No other GPU was selected.
- Target: `x86_64-unknown-linux-musl`, release, no default features, explicit
  `fe2o3-runtime/hardware-qualification`, existing BFD/static-link profile.
- Scope: exact R26 in-place compute plus disjoint directional H2D/D2H copies,
  both publication orders and one/two-packet profiles; two complete runs required.
- Outer stage: `/dev/shm/fe2o3-r66-owner.MoPM9aSZ`; private source, build home,
  cache, target and evidence storage. Installed toolchain components were
  checked before use; no shared toolchain/cache installation was performed.

The release build completed. Signature, snapshot, compiler/linker, production
dependency, ELF/static-symbol, topology, placement and idle-device checks ran
before the first monitored qualifier. Twenty-four runner commands succeeded;
the twenty-fifth, `monitor-owner-0`, exited 2 after the qualifier exited 1.
The campaign stopped without running `owner-1` or retrying the rejected set.

## Failure

The qualifier reported `R66 exact retained native roster unavailable`.
Its cleanup attempt explicitly retained one stream, one submission, one module
and seven allocations, together with all seven allocation-credit records.
The stream release rejected because it still owned a pending KFD dispatch.
This was not represented as completed cleanup or refunded credit capacity.

The transcript does not identify the exact rejected extractor predicate or
prove whether GPU computation was still active. The pending compute record
narrows diagnosis to a compute-containing observation, but does not substitute
for typed stage diagnostics or a full native trace. The next qualification
packet must explain the mismatch, add representative actual-state regression
coverage, then rerun the complete signed campaign. No guard is relaxed by this
record and no failed attempt is promoted to an accepted one.

Read-only native review found no unique rejected predicate. R26 constructs the
expected persistent runtime execution, published single native attachment and
attached dispatch control; H2D promotion retires its settled frontier. Those
representation hypotheses are not established defects. OVL-DIAG-1 will add
bounded typed runtime/native rejection stages and exact cell/observation phase,
with actual-R26-shape CPU fixtures and one-coordinate negatives. Observation
must remain immutable and address-free; a hardware rerun is a separate gate.

## Independent Audit And Cleanup

`rejection-audit.json` records verification of the trusted SSH commit signature,
byte-exact fresh `git archive` equality and all 6,932 source-file hashes. The
118,845,440-byte outer capture has SHA-256
`fa9c7733250bc07f2c7aaee445189b1e4f16852234db00f7060d5a5c5bf70a6a`;
its source archive has SHA-256
`adedf25216e3fabed36ed5a699c422a841c4a27b2f7104158a26309affc7cf34`.

The rejected runner retained source and command/audit output but not the actual
owner executable. Thus captured ELF/symbol checks are command evidence, not an
independent reinspection of retained binary bytes. Preserve the actual binary
on future post-build rejections to close this diagnostic limitation.

The outer wrapper reaped its runner and deleted its exact private stage.
A separate read-only SSH check established:

- The stage and all 27 recorded owned PIDs were absent.
- All 25 recorded process groups were absent, and no visible process command
  line still referenced the stage.
- Selected GPU 1 was idle with zero VRAM allocated.
- GPU 0 still reported its pre-existing 44% VRAM allocation. No reset, foreign
  process signal or foreign-storage cleanup was performed.

These process-exit/host-cleanup observations do not turn the runtime's incomplete
in-process cleanup into a successful quiescent drain. `cleanup.json` retains the
raw independent result. The private wrapper, controller and read-only audit
scripts are included for reproduction. `retained-files.sha256` identifies the
captured files and scripts; documentation is descriptive, not hardware authority.

OVL-QUAL-2, production generated authority, physical budgets, measured overlap,
performance parity and the broader [A1/A2 plan](../../runtime-a1-a2-swarm-plan.md)
remain open.
