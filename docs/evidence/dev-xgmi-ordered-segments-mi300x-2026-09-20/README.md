# Ordered XGMI Segment Comparison

Signed source `b7fd95cfb007fb8406649849459fa1de8cbc2088` passed the first
matched ordered-list campaign on MI300X GPUs 1 and 2, ROCm 7.2.4.
All six executions passed full source/destination byte and guard validation,
explicit teardown, fresh preflight and settled/delayed endpoint checks.
The owned remote directory and owned processes were confirmed absent after
byte-exact collection. Other host work was not reset, stopped, or removed.

## Workload And Results

Each direction transfers 65,536 useful bytes through 65 ragged descriptors into
reversed, disjoint destination slots. One prime, two warmups and ten samples use
distinct poisoned bands in the same retained allocation. Source and destination
allocations are not read or written by the host between lists. Initialization,
final readback and completion-record cleanup are outside the timed interval.

Each cell is host-observed whole-list latency in milliseconds. p50 is the median
of ten samples; nearest-rank p95 therefore equals the maximum of those samples.
Directions 0 and 1 are GPU 1 to 2 and GPU 2 to 1 respectively.

| Trial | Backend | Direction 0 p50 / p95 | Direction 1 p50 / p95 |
| --- | --- | --- | --- |
| 1 | KFD | 25.305846 / 25.518634 | 25.278530 / 25.523590 |
| 2 | HSA | 0.923931 / 0.929159 | 0.922373 / 0.925383 |
| 3 | HIP | 0.350218 / 0.375311 | 0.343564 / 0.373468 |
| 4 | HIP | 0.347239 / 0.372687 | 0.345331 / 0.372367 |
| 5 | HSA | 0.923852 / 0.927126 | 0.923616 / 0.927597 |
| 6 | KFD | 25.376057 / 27.368510 | 25.432942 / 27.579986 |

KFD is approximately 27.4 times slower than HSA and 72 to 74 times slower than
HIP for this workload. The four per-direction/per-trial medians per backend
range from 25.278530 to 25.432942 ms for KFD, 0.922373 to 0.923931 ms for HSA,
and 0.343564 to 0.350218 ms for HIP. These ranges are not confidence intervals.
There is no demonstrated copy-performance parity or speedup here.

## Comparison Limits

- KFD submits one logical list and includes descriptor admission, journal work,
  serial singleton publication/wait, and full opening/closing currentness.
- HIP enqueues the complete list on one stream and queries final completion.
  HSA enqueues a predecessor-signal chain and loads the final completion signal.
  Neither baseline implements fe2o3's full-currentness contract.
- All implementations use a single absolute 60-second list deadline and an
  external process watchdog. Submission-handle release and HSA signal reset are
  outside list timing; KFD closing-currentness remains inside it.
- This is one small scattered-copy geometry on a shared host, not exclusive
  performance qualification, kernel latency, physical link bandwidth, formal
  executable refinement, or general HIP/HSA parity. Endpoint observations do
  not establish continuous isolation.
- Disjoint benchmark segments cannot independently prove ordering. The separate
  [ordered-copy correctness packet](../dev-ordered-peer-copy-mi300x-2026-09-20/README.md)
  and CPU overlap tests cover that contract.

## Provenance And Replay

The controller recorded a cached local musl release build, pinned Rust/Cargo
identity, three release example tests, two C++/Rust plan tests including all
eight geometries, four result-parser tests, and three campaign tests. No tests
were skipped in these receipts. The HIP/HSA comparators were built on MI300X
with optimization level 3 and warnings denied. Remote compiler identities,
source inventories and binary hashes match before/after the campaign. Local
Rust/Cargo identities bracket the local build, before upload. This is not a
clean-room independent rebuild or a hermetic system-library provenance claim.

`binding.json` identifies the complete selected local source roster and signed
tree, the smaller remote comparator/observer source archive, all payload hashes,
and exact workload controls. `local/` and `remote/` contain original receipts;
`remote/validated-results.json` contains all raw durations and derived summaries.
The uploaded ELFs and source tarball are not included in this text-only packet.
The enclosing signed archive commit, not a self-reported hash alone, supplies
the archive's authenticity anchor.

```sh
python3 -I -B docs/evidence/dev-xgmi-ordered-segments-mi300x-2026-09-20/verify.py
python3 -I -B docs/evidence/dev-xgmi-ordered-segments-mi300x-2026-09-20/test_verify.py
```

The offline verifier reconstructs commands, environments, source roster, exact
trial order, all 36 endpoint admissions and postflight observations, sample
statistics, process-group absence and owned cleanup. Adversarial tests rehash
modified receipts before replay, so rejection does not rely only on the seal.

Next: attribute the 65-segment cost before changing native behavior, expand the
remaining seven payload/count geometries, and evaluate allocation-free singleton
waits and ordered queue windows with their own custody/deadline qualification.
