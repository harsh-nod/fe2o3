# Ordered XGMI Scalar-Wait Qualification

Signed source `e563ea48c4d9de251052feac58fb5db2252020b4` passed the matched
ordered-list campaign on MI300X GPUs 1 and 2, ROCm 7.2.4. This source replaces
four temporary singleton wait vectors per descriptor with an inline scalar
completion result and forwards the original absolute deadline. Publication
currentness checks, terminal retention, and full scope closing remain enabled.
It is not a globally allocation-free path.

All six executions passed source/destination byte and guard validation, explicit
teardown, fresh preflight, and settled/delayed endpoint checks. After byte-exact
collection, the owned directory and processes were confirmed absent. Other
host work was not reset, stopped, or removed. Idle checks are not an exclusive
reservation or proof of continuous isolation.

## Workload And Results

Each direction transfers 65,536 useful bytes through 65 ragged descriptors into
reversed, disjoint destination slots. One prime, two warmups, and ten samples use
distinct poisoned bands in the same retained allocation. Host initialization,
final readback, and completion-record cleanup are outside list timing. There
are no host allocation reads or writes between lists.

Each cell is host-observed whole-list latency in milliseconds. p50 is the median
of ten samples; nearest-rank p95 equals the maximum. Directions 0 and 1 are
GPU 1 to 2 and GPU 2 to 1 respectively.

| Trial | Backend | Direction 0 p50 / p95 | Direction 1 p50 / p95 |
| --- | --- | --- | --- |
| 1 | KFD | 25.294530 / 25.431209 | 25.240034 / 25.497418 |
| 2 | HSA | 0.923360 / 0.926033 | 0.923350 / 0.927555 |
| 3 | HIP | 0.347660 / 0.380838 | 0.344214 / 0.354830 |
| 4 | HIP | 0.352472 / 0.382862 | 0.344314 / 0.359688 |
| 5 | HSA | 0.925493 / 0.926675 | 0.923540 / 0.927265 |
| 6 | KFD | 25.339374 / 25.549678 | 25.348067 / 25.494305 |

The four per-trial/per-direction medians range from 25.240034 to 25.348067 ms
for KFD, 0.923350 to 0.925493 ms for HSA, and 0.344214 to 0.352472 ms for HIP.
These are descriptive ranges, not confidence intervals. KFD is approximately
27 times slower than HSA and 72 to 74 times slower than HIP for this workload.

The KFD range overlaps the [prior source's range](../dev-xgmi-ordered-segments-mi300x-2026-09-20/README.md)
of 25.278530 to 25.432942 ms. Old and new KFD binaries were not interleaved in
one campaign. This historical comparison does not establish a speedup or
attribute a latency difference to vector removal. Copy-performance parity
remains absent.

## Scope And Provenance

- KFD includes descriptor admission, journaling, serial publication/wait, and
  full opening/closing currentness. HIP queues the complete list on one stream;
  HSA queues a predecessor-signal chain. Neither implements fe2o3's complete
  currentness contract. Each list uses one absolute 60-second deadline and an
  external watchdog. Closing-currentness remains inside KFD timing.
- This is one scattered-copy geometry on a shared host, not physical link
  bandwidth, GPU execution timing, native fault injection, executable formal
  refinement, general HIP/HSA parity, or performance acceptance.
- The disjoint native segments do not independently establish ordering. The
  focused CPU ordered-runtime tests cover overlapping descriptors and custody
  failures. The earlier correctness packet applies to its older source only;
  the eight-case native smoke was not rerun as part of this packet.
- `qualification/` retains the original nine scalar wait and six ordered-runtime
  test receipts, plus repeats bracketed by successful exact signed-source diffs
  over the bound selectors. Commands, environments, counts, chronology, and
  process cleanup are checked. The tests cover
  expiry/relative-zero observation, malformed tickets, timeout/retry, wrong
  completion, panic custody, scalar/batch differential behavior, closing-error
  precedence, and direct runtime deadline forwarding. They are CPU fixtures
  and source guards, not machine-code or DMA proofs.
- `local/` records the cached musl release build, pinned Rust/Cargo identity,
  three release example tests, two C++/Rust plan tests covering eight geometries,
  four result-parser tests, and three campaign tests. HIP/HSA were rebuilt on
  MI300X with optimization level 3 and warnings denied. Source, toolchain, and
  binary identities match their recorded before/after checks. This is not a
  clean-room or hermetic rebuild claim.
- `binding.json` identifies the signed source tree and complete selected source
  roster, comparator/observer inputs, payload hashes, and workload controls.
  The uploaded ELFs and source tarball are not included in this text-only packet.
  The enclosing signed archive commit anchors authenticity; hashes alone do not.

## Replay

```sh
python3 -I -B docs/evidence/dev-xgmi-ordered-segments-scalar-wait-mi300x-2026-09-20/verify.py
python3 -I -B docs/evidence/dev-xgmi-ordered-segments-scalar-wait-mi300x-2026-09-20/test_verify.py
```

Replay is offline but requires Git history, pinned helpers, and the qualified
source files unchanged from the signed source checkpoint. Use the archive
commit's worktree when replaying after later source changes. The verifier
reconstructs commands/environments, all 36 endpoint observations, statistics,
CPU test counts, process-group absence, and owned cleanup. Adversarial tests
rehash mutations before checking semantic rejection. The prior packet is
unchanged and cannot substitute for this source's qualification.

Next: measure success-only host phase attribution before larger optimizations,
then extend native testing to the remaining seven payload/count geometries.
