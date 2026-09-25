# Prepared Conditional Vecadd Replay

This follows [conditional proof retention](conditional-proof-retention-20260924.md)
and advances #272's test infrastructure. No milestone closes and the 47-kernel
production-to-safe-GPU-launch qualification matrix is unchanged.

## Integrated Changes

The test-only exporter captures genuine offline Cargo dependency builds for
the annotated shared-body Vecadd fixture on gfx942 and gfx950. It records raw
arguments and environment, normalized replay arguments, dependency hashes,
source stamps, executable and tool identities, and the exact named replay
support files. The support roster is not a full sysroot closure.

The shared Cargo capture helper explicitly selects the compiler beside the
actual Cargo executable. It no longer records a compiler selected indirectly
through PATH. Replay still requires that exact pinned compiler path and bytes.

Preparation rejects changed dependencies, links, unknown fields, unsupported
options, additional cfg entries, target substitutions, and authority claims.
The complete cfg vector must match the selected fixture, including spelling and
multiplicity. Inventories and hashing are bounded; no preparation record is a
proof, descriptor, or launch authorization.

The ignored replay parent invokes the existing in-process rustc child, not Cargo
or a second compiler pipeline. Prepared inputs stay read-only. Because rustc
writes dep-info before `after_analysis`, replay redirects its output into the
fresh results directory and binds the child request to those exact arguments.
It permits only top-level `.d` files there, rejects links and compiled artifacts,
and rechecks source and prepared inputs after the child. Empty result
subdirectories are tolerated; this is not an exact directory-topology check.

The parent requires actual receipt-retention observations and the existing
`FE2O3-COND-FINALIZER-001` refusal. This protected positive test has **not run**.
It still reports no artifact, launch, hardware, or qualification authority.

## Validation

All passing guards used nightly `2026-04-03`, locked offline dependencies, one
Cargo job/test thread, hidden GPUs, disabled HIP, a 12 GiB virtual-memory cap,
and a 1200-second deadline. Source and tool snapshots were stable.

- A: 8000 files, `b80971b61b554128a3fba4a3840f2715459b700d14e0270a5ed70b096fbe7aa0`.
- B: 8023 files, `028c8ba2ef7a03139a2ff482523992914aa70b65e81ccd300c93dcf6ca163361`.

These are source-inventory hashes, not Git tree hashes. B incorporates the
concurrent BF16 and authoring-contract work through
`1e368326c3d21cf3354b4fa839e45177cfa29282`. This page and its incoming link were
added after the guards.

| Run | Snapshot | Result | Log SHA-256 |
| --- | --- | --- | --- |
| `prepared-vecadd-local-r2` | A | 10 passed, 2 ignored | `3936ab313956a4fcf4e6829d0a53cf7164302609058673a25a3864abeda54803` |
| `prepared-vecadd-capture-r2` | A | One real two-target preparation test passed | `6c256fc27c20e33254a990e08af3a774c9211a9b98c1dac364f734f0d8f56259` |
| `prepared-vecadd-backend-artifact-r2` | A | Backend test executable selected from successful Cargo JSON; no tests executed | `c36c9443529ee941afee7dc6637a9a6762b80501873ad6334995a3b414e3d574` |
| `prepared-vecadd-verifier-artifact-r2` | A | Verifier test executable selected from successful Cargo JSON; no tests executed | `3029f75ca83057412865e605b095f198f248dc7d3d258992b0c06b75c8f43830` |
| `prepared-vecadd-merged-r3` | B | 10 passed, 2 ignored | `d8be3d7d876696beaba75d6d249f045c89a0b48f5871d69d7238eca4a83e47a5` |

The focused non-protected selection is:

```sh
cargo +nightly-2026-04-03 test --locked --offline \
  -p rustc-codegen-fe2o3 --lib \
  --features fe2o3-pliron/internal-proof-staging -- \
  prepared_inputs:: corpus_cargo:: --test-threads=1
```

Real preparation additionally requires a fresh absolute directory outside the
sources in `FE2O3_TEST_CONDITIONAL_VECADD_PREPARE_V1` and explicitly selecting
the ignored `prepare_actual_shared_body_vecadd_inputs` test in that module.
It requires preseeded offline dependencies and the bounded build controller.
Protected replay uses `FE2O3_TEST_CONDITIONAL_VECADD_INPUTS_V1` and a fresh
`FE2O3_TEST_CONDITIONAL_VECADD_RESULTS_V1`; it must use the same captured binary
and source under the existing protected-runtime admission contract.

The first preparation attempt failed on the relative compiler path; its log
hash is `2d78b742e94ff287bbc6170af3b61efa0fb85f955cfac36cda5a8e281df87d5c`.
Review also caught dep-info output isolation and alternate cfg spelling; both
were fixed before the passing runs. Post-merge attempts r1 and r2 were
interrupted during compilation without final guard reports. Neither counts as
passing. The successful retry above used fresh scratch space.

The local captured bundle passed the external runner's input validation, but
its RAM-backed copies subsequently disappeared during an interruption. The
on-disk logs remain; inputs must be regenerated before protected execution.
The external runner's separate 69-test offline suite is not compiler, proof,
or GPU qualification. No remote job was started for this checkpoint.

Guard SHA-256:
`20f8ea8ce430f72e9f2925dc4d79b28739229f36dd6d0c46a45b430caffd3642`.
Formatting, whitespace, and dependency policies passed. Existing dead-code and
duplicate-target fixture warnings remain. This is focused coverage, not a full
workspace rerun; real capture was not repeated after the concurrent merge.

## Remaining Gates

Protected actual-source replay, the generated-field descriptor bridge,
conditional finalization, applicable machine/numerical refinement, native
issuance, and generated safe host launch remain required. This checkpoint
adds no ISA-equivalence or floating-point error-bound theorem and makes no
new kernel qualification claim.
