# MI300X R39 persistent SDMA wait policy, 2026-09-05

Status: `Qualified exact product; bounded formal model verified`. R39 applies
a bounded 50 us elapsed active-spin floor only to the directional persistent
single/window and same-device persistent window SDMA waits, then returns to
the existing bounded spin/yield/sleep policy. The exact production archive
validates the finalized commit and reports KFD E2E p50 of 262554, 262889, and
262358 ns. Private policy-screen evidence is kept separate from that product
qualification.

## Exact product qualification

- Exact R39 production:
  [`4be5243dbe835c94618a62b3702f9624cd8f9d1f`](https://github.com/harsh-nod/fe2o3/commit/4be5243dbe835c94618a62b3702f9624cd8f9d1f),
  tree `56af9dee4a50e1777d767cbe424a01b018eb8fa0`.
- Production delta: KFD `queue_live.rs`, `sdma.rs`, and `wait.rs`, plus four
  R26 benchmark policy pins. The three enabled call sites are the persistent
  directional single-submission wait, persistent directional window wait, and
  same-device persistent window wait. Generic and non-persistent waits retain
  the default policy. The elapsed floor is clamped to the caller's deadline.
- Host and workload: `sharkmi300x-1`, Linux `6.8.0-124-generic`, ROCm
  `7.2.4`, GPU 2 (`gfx942:xnack-`, unique ID `0xd2e26fef80cf5c33`), one
  1 MiB in-place `u32` transform, 10 warmups, 30 samples per backend per
  slot, and 10 iterations averaged into each sample.

The corrected external archive is
`/tmp/fe2o3-r39-production-4be5243d-r26-20260905-corrected-evidence.tar.gz`,
SHA-256
`d1e7ce085bfe57126af554868affccf811fea9348daf0a1773022e5ffd83e0cf`,
size 28,197,732 bytes. Its 23-entry `content-manifest.sha256` verifies 23/23
files. The source bundle verifies, the source worktree records zero status
bytes, all nine monitors are clean, all 18 topology and busy-zero edges are
present, all three slot validations pass, and the set validation passes.

The corrected archive changes only the summary's slot-validation count from
zero to three and the reproduction filename to the corrected archive name.
`content-manifest.sha256` was necessarily rehashed for those two corrected
records. Measurement logs, binaries, source bundle, and other evidence payloads
are unchanged.

The set ID is
`9b85a4b08c0871a14c8c133751a3ffe2010d77f88047775315fdda7850fff9d5`;
set manifest SHA-256 is
`51746e12e7a8cd8e6e80f02b8429783e3852bf4588c7d398558485de71e43204`;
set-validation SHA-256 is
`6be579f0a67d30a2c887714c1be9b55b0e4c43ccdf3100156309cc941061eff9`.
The archive is external and non-durable.

## Measured path

R39 selects `PersistentElapsedSpinFloor` at exactly three sites: the
directional persistent single-submission wait, directional persistent window
wait, and same-device persistent window wait. R26 H2D and D2H exercise the
directional path. The first completion observation still precedes the deadline
decision. The 50 us floor is bounded by the caller deadline; after it expires,
the shared wait cursor resumes its 64-spin, 16-yield, exponentially bounded
sleep sequence. Default waits do not inherit the floor.

All values below are host-monotonic p50 nanoseconds. Lower is better.

| Slot | Backend | H2D | Compute | D2H | E2E |
| --- | --- | ---: | ---: | ---: | ---: |
| 0 | KFD | 133976 | 82864 | 45128 | 262554 |
| 0 | HSA | 45179 | 20716 | 25245 | 91321 |
| 0 | HIP | 47187 | 25141 | 26827 | 99311 |
| 1 | KFD | 134500 | 82493 | 45347 | 262889 |
| 1 | HSA | 44548 | 30132 | 25356 | 100197 |
| 1 | HIP | 46990 | 25034 | 26833 | 99195 |
| 2 | KFD | 134483 | 82146 | 45629 | 262358 |
| 2 | HSA | 44736 | 20801 | 25447 | 91102 |
| 2 | HIP | 47061 | 25137 | 26696 | 98991 |

R39 remains 2.643x-2.650x slower than HIP E2E and 3.268x-3.296x slower than
HIP compute. The elevated slot-1 HSA compute value is retained rather than
normalized away; it limits HSA-relative interpretation.

## Sequential production comparison

The exact production baseline archive is
`/tmp/fe2o3-r39-baseline-a1ea30cf-r26-20260905-evidence.tar.gz`, SHA-256
`c49703d9736711fde3201a57c568149498c3abd206c60c4dfcab9026a46e900a`,
size 28,160,037 bytes. It records exact production commit
`a1ea30cffbd24a5714a5fe0318b4231f42e98727`. The raw KFD p50 values for this
sequential baseline and the exact R39 product are:

| Slot | Revision | H2D | Compute | D2H | E2E |
| --- | --- | ---: | ---: | ---: | ---: |
| 0 | baseline | 188916 | 83328 | 45732 | 318560 |
| 0 | R39 | 133976 | 82864 | 45128 | 262554 |
| 1 | baseline | 188195 | 83468 | 45917 | 317860 |
| 1 | R39 | 134500 | 82493 | 45347 | 262889 |
| 2 | baseline | 189827 | 83237 | 45981 | 319150 |
| 2 | R39 | 134483 | 82146 | 45629 | 262358 |

R39 H2D was 29.082%, 28.532%, and 29.155% lower, with a 29.082% median
slotwise reduction. E2E was 17.581%, 17.294%, and 17.795% lower, with a
17.581% median reduction. Median ratio-of-ratios reductions were 29.275%
against HSA and 29.436% against HIP for H2D, and 17.941% against HSA and
17.738% against HIP for E2E.

This is a sequential descriptive comparison, not strong causal attribution.
The same `a1ea30cf` baseline produced roughly 206-209 us H2D in the earlier
R38 session, versus 188-190 us here, showing material session variation even
for identical production code.

## Private policy screen

The private screen archive is
`/tmp/fe2o3-r39-policy-screen-20260905-evidence.tar.gz`, SHA-256
`bf1588a6c1801319cb82f41d186aa89c8ba9a1ecca3c03a5abd999b402c43a69`,
size 30,675,053 bytes.
It uses a four-slot cyclic Latin order across private base, 25 us, 50 us, and
100 us policies. Base and 25 us KFD H2D p50 values were 208-212 us, while
50 us and 100 us values were 134-136 us. Their E2E bands were respectively
341-351 us and 262-267 us. All 16 run monitors and both diagnostic monitors
were clean. This selected a policy for production qualification; it did not
qualify a public revision.

A separate private matched qualification compared exact base
`a1ea30cffbd24a5714a5fe0318b4231f42e98727` with private 50 us candidate
`9a0f0e8d30b20b1615d53c907e48369164f0b824`. Its three E2E p50 pairs were
318560/240035, 317860/239718, and 319150/241863 ns, reductions of 24.650%,
24.584%, and 24.220%. These private candidate observations motivate the
policy; they are not measurements of production commit `4be5243d`.

The private candidate archive is
`/tmp/fe2o3-r39-floor50-9a0f0e8d-r26-20260905-evidence.tar.gz`, SHA-256
`da89673af4d08c4a5bddb7fbc53a2fb8b75a27f8d266f31a00313f947e1a3b90`,
size 28,159,926 bytes. Together with the exact baseline archive above, it
supports the private candidate comparison only; the candidate is not
exact-product qualification.

## Formal status

The signed integration commit
[`4a0a31c413de5f354f62f759c0406f752dc44994`](https://github.com/harsh-nod/fe2o3/commit/4a0a31c413de5f354f62f759c0406f752dc44994)
adds the dedicated R39 executable and Verus models. The positive source pin is
`ba6513b69ccab2cc7ea84adb971ebc84da7cbf7b4f4756565c8d6da8b799dc02`;
the aggregate transcript pin is
`bc8baeaaed14f979e9ddec4e6d0b7d322a7dbd7635319e97ba027a8efd8a4534`.
The direct proof verifies 20 obligations with zero errors. The aggregate
runner verifies 877 obligations and rejects 349 pinned mutations, including
all 10 R39 countermodels. Six focused Rust tests check exactly 156
snapshot/scenario combinations.

The model proves bounded mathematical properties for the exact three-site
allowlist, checked and clamped floor endpoint, observation-before-deadline
ordering, separately ordered deadline-check and action-time samples,
saturating cursor arithmetic, and preservation of the contracted R37
snapshot. It admits a cursor start after an already computed deadline and a
deadline crossing between the two time samples. The runner pins the proof and
countermodel sources; it does not authenticate production commit `4be5243d`,
production Rust, or a Rust-to-Verus refinement. Real `Instant` behavior,
syscalls, native completion, driver/firmware/hardware behavior, liveness,
parity, and performance remain outside the proof boundary.

## Archive integrity

The corrected exact-product archive and every content-manifest entry can be
checked without rebuilding or rerunning the workload:

```bash
archive=/tmp/fe2o3-r39-production-4be5243d-r26-20260905-corrected-evidence.tar.gz
printf '%s  %s\n' \
  d1e7ce085bfe57126af554868affccf811fea9348daf0a1773022e5ffd83e0cf \
  "$archive" | sha256sum --check
gzip -t "$archive"
root=$(mktemp -d)
tar -xzf "$archive" -C "$root"
evidence=$(find "$root" -mindepth 1 -maxdepth 1 -type d -print -quit)
test -n "$evidence"
test "$(wc -l < "$evidence/content-manifest.sha256")" -eq 23
(cd "$evidence" && sha256sum --check content-manifest.sha256)
```

## Claim limits

The exact-product archive is a clean, reproducible qualification of one commit,
one host, one GPU, and one fixed 1 MiB workload. It has no matched production
baseline in the same archive, randomized revision order, confidence interval,
power or thermal normalization, broad size sweep, concurrent application,
multi-device workload, or hardware performance counters; both `perf` probes
were denied by host policy (`perf_event_paranoid=4`). The product run and the
private screen support a bounded policy repair, not causality, general speedup,
HIP/HSA parity, or an orders-of-magnitude claim.
