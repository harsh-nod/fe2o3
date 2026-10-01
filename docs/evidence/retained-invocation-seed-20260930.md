# Retained invocation-seed checkpoint — 2026-09-30

This checkpoint advances the private BF16 source observer through initial
invocation seeding. It does not enable ordinary whole-kernel compilation,
propagation, nonempty reads, native execution, or a new artifact path.

The retained owner keeps the original execution prefix, SSA/argument counters,
scalar capabilities, graph rows, and both FIFO owners attached across fallible
work and canonical callback postflight. Foreign counters, changed source or
capability inventories, prior completion tokens, and terminal reentry reject.
Grid-leader recovery remains explicitly unavailable.

## Qualified results

The MI350 CPU-only qualification used compiler base
`fe0b3520811536a82cf14af8973ba992454bfd1b` plus the fourteen-file seed cohort.
The complete source census was 9,522 files / 134,762,452 bytes, SHA-256
`e72caa2a7a89a3fae03386600e514c5604b9b7de1fc58e23db1c1b478f5f069b`.

- Backend library: **3,471 passed, 0 failed, 201 ignored**, including 25 new
  component/observer controls. Backend library/extractor build, selected pinned
  rustfmt checks, and whitespace checks passed.
- Strict observation readers: **357 passed**, including 118 seed-reader
  controls. Same-child local/block dimensions are joined across seed, graph,
  and empty-read families; the prewriter block metric is deliberately distinct.
- The separately selected actual-source parent passed in 197.21 seconds. Its
  five fresh rustc sessions comprise identity, swap01, callback-error,
  callback-panic, and wrong-launch.
- Each of the four accepted source sessions observed 31 locals, 19 blocks,
  **one actual invocation seed**, two prefix operations, next SSA value 1,
  and next runtime argument 1 in Compare, CallbackError, and CallbackPanic.
  The wrong-launch session emitted no accepted component markers.
- All four marker families retained 12 records apiece in their required order.
  The recursive output audit covered 492 files / 362,194,349 bytes.

The independent retained donor and the candidate are compared as separately
owned outputs, but share the original seed algorithm. This establishes wrapper,
custody, and namespace agreement, not an independent numerical semantics proof.
No actual zero-seed source case was observed in this run; zero-seed behavior is
covered by component controls.

## Evidence and limitations

Retained qualification receipt SHA-256 values:

| Qualification | Receipt SHA-256 |
| --- | --- |
| Complete backend regression/build | `967247b9e084452c4ceb1b9d9ebc8473e564a4331bf463641eaf3e77f1e2a4e6` |
| Five-session actual-source parent | `6ef6c4cbe27861437b80eb0fe4e5eb58fea52b4feea765e1ed888808afab481e` |
| Strict actual marker readback | `b5cdd3b3ef593959699d8cac304b78035b8da99b20ce070a9a6464cfaef30800` |
| Combined actual-source audit | `cb904829031119739ce8be0da0b8d9a2b1416ff5a955298da2a58fe4c798c617` |

The first component-only regression attempt hit the existing storage guard.
It is retained as incomplete, not a pass. Five completed derived dependency
caches were losslessly archived and verified before removing their recoverable
uncompressed copies; the successful run did not relax the guard.

Qualification used nightly-2026-04-03, offline locked Cargo, two build/test
workers, and the exact ignored parent selector
`production_rustc_driver_v1::gfx942_bf16_call_source_cpu_qualification_v1_tests::actual_bf16_call_source_cpu_ladder`.
This is not authorization to run every ignored or native test.

The [current contract applicability decision](../authoring-contract-closure-20260924/APPLICABILITY-20260930.md)
adopts scoped integration boundaries and qualification budget targets. Those
targets are not measured compliance or external owner signoff. M0/U0/V0 remain
open. The #280/#281/#282 accepted ledger remains **6/18**: M1, V1, V2, U1, U2,
and U3. The other twelve milestones require their full exit evidence.

The two upstream commits between the tested base and
`cccd210bba11aee0bcb402ed85bf9678fae8d8cf` change only the separate
scoped-invocation evidence document. Their results are not combined with this
checkpoint to claim ordinary source or production completion.
