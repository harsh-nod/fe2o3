# Mixed-Duration CPU Qualification

Development checkpoint, not native acceptance or HIP/HSA parity.

The final cold run uses signed source
`ca1edd1423b6f094c75bc363b6d6f02004f1fa0e`: 5,978 signed source inputs,
13 successful phases and 17 terminal command receipts. Direct execution of the
retained GNU and static-musl test ELFs passes **1,423 tests per target**, with
24 hardware-only ignores each. All 46 doctests (separate 4+42 summaries),
default-feature check, strict all-target Clippy and formatting pass.

## Implemented Scope

- Separately admitted gfx942 short/long objects, fixed 257/33,554,433 recurrence
  steps, Wave64 geometry, exact ABI, effects, allocation and initial-byte gates.
- A complete 384-byte output per operation, including both guard regions.
  Rust affine exponentiation is checked against an independent Python modular
  geometric-sum oracle. Both pinned objects rebuild byte-for-byte identically.
- Seven CPU tests cover profile substitution, changed geometry/arguments,
  allocation metadata and every input/output byte. Immutable shared initial
  storage avoids enlarging the backend launch-gate enum or adding a heap object.
- Two compiled, ignored native tests: separate profile correctness/refunds and
  actual async-owner later-short completion while the earlier long native signal
  remains pending. The latter joins retained native identities, full readback,
  queue lifecycles, profiler coverage and owner-thread cleanup.

The tests do not prove physical overlap, 2,048 native-retained operations,
general compiler authority, formal ISA refinement or matched performance.
The native campaign [stopped during upload](../dev-mixed-duration-native-2026-09-25/README.md);
neither native test launched. No hardware success is claimed.

## Reproduction And Retention

```sh
python3 -I -B docs/evidence/dev-mixed-duration-2026-09-25/verify.py
python3 -I -B docs/evidence/dev-mixed-duration-2026-09-25/test_verify.py
```

Replay independently authenticates the source commit, complete archive and
source tree, retained-versus-executed ELF digests, exact commands, raw CPU and
doctest summaries, and both full test rosters. Selected compiler identities are
recorded; this is not full toolchain/cache closure or independent OS attestation.

All 227 original scratch artifacts are copied byte-exactly to `retained/`.
Earlier attempts are preserved: v1 was interrupted, v2's successful split
doctests were incorrectly rejected by its orchestration, and a continuation
found a real Clippy enum-size problem. The final v3 cold run follows both fixes;
historical failures are not relabeled as passes.

`retention.json` binds the full original scratch set. Later `audit/` records are
separate and do not modify it. `SHA256SUMS` covers the publication packet.
After the execution context changed, the original `/dev/shm` build mounts were
not visible. `collect.py --retain-only` therefore performs no removal and makes
no host-process or build-tree cleanup claim. Exact historical paths remain in
`cleanup-pending.json`; the local scratch is deliberately preserved.

A1/A2 and broader Native R125, Admission R118B C1/C2/C3, and Resources R116/V3
acceptance checkpoints are unchanged.
