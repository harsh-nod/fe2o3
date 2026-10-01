# Monotonic Wait Arithmetic

Signed source: `a7d110af1ff14bf71a82799c6224abc7894149e9`.
This packet records qualification of the four shared numeric helpers for wait
attempt saturation, the spin/yield prefix, sleep minimum selection, and bounded
canonical seconds/nanoseconds backoff. It is not a whole-runtime parity claim.

## Results

- Four complete two-input proofs, including independently relocated, equivalent
  and closing positives: each 7 verified, 0 errors.
- Seventeen reviewed actual-body negatives: each 6 verified, 1 intended logical
  failure, with strict source-span diagnostics. The absent-None case exercises
  the broader numeric-helper contract, not a reachable native adapter branch.
- Four signed metadata groups and 23 proof groups closed. The original managed
  raw result keeps zero historical qualified kills and signed acceptance false;
  the separate complete signed result records 17 qualified numeric negatives.
- The exact native candidate passed 1,841 no-default and 1,847 all-feature KFD
  library tests, with no ignored or filtered tests, and both strict Clippy modes.
  The remote CPU owner completed all 20 stages, but SSH ended with exit 255.
  That transport remains rejected; complete raw CPU evidence was separately
  recovered and independently replayed. Its temporary remote root was cleaned
  only after durable collection and inspection.

`final.tar.gz` is the unchanged 190-member custody archive, accompanied by its
original manifest and result. It includes six signed source buffers, all proof
projections, raw streams/receipts, both distinct results, source/review records,
and exact external prerequisite references. Originals and failed predecessors
remain retained. The archive SHA-256 is
`7681239d4852c20aa8a5bc59dc871a36e1302c743a72abe325eb2f7b38ce3a95`.

## Portable Inspection

From this directory, run:

```sh
python3 -I -B check-recorded-diagnostics.py
python3 -I -B test_recorded_diagnostics.py
```

The small offline checker verifies the exact archive and every member, rebuilds
all 20 source projections, and replays all 17 recorded diagnostic classifications
through the unchanged reviewed parser. Original absolute paths are treated only
as diagnostic source labels; no original host files are opened. The preserved
mutation module's host-specific `sources()` entrypoint is not called.

This does not rerun Verus, authenticate historical process closure anew, replay
CPU tests, or make the historical archive controllers portable. The raw positive
outputs and exact source are available for inspection. A fresh proof requires
the recorded Verus/std toolchain and its separately pinned release closure.

## Limits And Prerequisites

This is not a self-contained campaign replay bundle. The full CPU source/raw/ELF
archives, signed Git objects, pinned tools/std binaries, managed process helpers,
native-checker ancestors and earlier rejected attempts remain explicit external
prerequisites in `summary.json` and the archived records. Bundled checker helpers
are exact copies, not new qualification authority.

The `Duration`/`Instant` adapter, constructors, complete cursor, scheduling,
hardware and performance are not proved here. Native regression tests cover
the adapter separately, including the original 16,800-case differential oracle.
No GPU campaign measured this arithmetic revision; the earlier cadence benchmark
used different source. Defaults, operational checks and milestone exits are
unchanged. HIP/HSA parity and orders-of-magnitude speedups remain unestablished.
