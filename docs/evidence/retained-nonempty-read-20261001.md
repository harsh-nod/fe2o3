# Retained nonempty-read checkpoint — 2026-10-01

A real Rust source case now exercises one retained, constant-shape shared read
through the private BF16 observer. Its read view, projected rows, instruction
prefix and SSA/argument counters survive successful, error and panic callbacks.
This is a checked component checkpoint, not ordinary whole-kernel compilation.

The source fixture adds a separately selected `single-strided-read` feature.
The source driver admits that feature, the independently bound empty identity
case, and the wrong-launch case separately. It does not fall back to historical
source features or fabricate a read effect.

## Qualified results

Qualification used compiler base
`76aec7187b64b3eb6a26534b4d0c0a66f46caa18` plus the seventeen-file cohort.
The source census was 9,531 files / 134,891,879 bytes, SHA-256
`7563132e7d584f9a98c2557646e8b28a1734c2614bbadcf940353601c469c15d`.

- Full backend regression: **3,496 passed, 0 failed, 203 ignored**.
  All 25 new controls passed, along with 357 observation-reader controls and
  six runner controls. Backend/extractor build, selected pinned rustfmt checks
  and whitespace checks passed.
- The new three-session actual-source parent passed in 81.110 seconds.
  The positive source retained **37 locals, 23 blocks, effect block 5,
  one read view, 23 projected rows, six prefix operations, next SSA value 5,
  and next runtime argument 1** in Compare, CallbackError and CallbackPanic.
- The separately bound empty identity source refused before materialization
  because it had no read-view effect. The wrong-launch source refused before
  the observer owner. Neither rejection emitted an accepted nonempty marker.
- Because the shared test wrapper and fixture changed, the historical
  five-session parent was rerun freshly: identity, swap01, callback-error,
  callback-panic and wrong-launch all passed in 197.402 seconds.
  Strict readback retained twelve records each for prewriter, empty-read,
  initial-graph and invocation-seed families in their required order.
  The four accepted historical source cases still had 31 locals, 19 blocks,
  one invocation seed, two prefix operations, next SSA value 1 and
  next runtime argument 1 in all three modes.

The new source output audit covered 482 files / 360,966,341 bytes before
losslessly archiving its derived dependency cache. The historical rerun audit
covered 492 files / 362,199,157 bytes. No archive operation changed source,
reports or logs; the uncompressed cache is recoverable from the verified archive.

## What is checked

The retained legacy-algorithm owner keeps partially constructed nested owners
attached across fallible work. Tests exercise every admitted work/storage cut,
same-root reuse, distinct roots, invalid source lengths/contracts/origins,
foreign source/counter/ledger/floor, nonlayout prefixes and unwinding.

The genuine observer prepays both complete comparison passes, checks current
payloads before and after canonical callback postflight, refuses stale
completion tokens, and rejects foreign counters and terminal reentry. Its
profile is deliberately narrow: one 16-bit shared constant 1×1 read, at most
32 blocks and 4,096 locals. A callback result counts only after its observation
completed. Strict LF-framed records reject wrapped, embedded, duplicated,
CR/CRLF and unterminated marker or JSON records.

The retained oracle and candidate use the original read algorithm in separately
owned outputs. This checks ownership, payload and namespace agreement; it is
not an independent numerical semantics proof. Logical map-slot accounting does
not claim physical hash-table bucket/control-byte, allocator or RSS bounds.

## Evidence and limits

| Qualification | Receipt SHA-256 |
| --- | --- |
| Backend regression/build | `35ce00bcd8b415ef905ab768bc1e701b623aa51871e1d2b6adcf1598b170b895` |
| New three-session actual-source parent | `ffdc887afc4b1265443a1e42328e5383d4f957c27a8596fd71c654adef155987` |
| Historical five-session actual-source rerun | `02d0dcfe05b311ec21d663187e07e0bc22b84db4f8f9fd373c0bd4e45bb496e9` |
| Historical strict marker readback | `a658b2b64af2faf420c07c699be482bc40f867fec686c029a6f245baa15f76e9` |
| Combined root audit | `206c0f7422005fca8224f61ffb59c87eb7e0f5f3e78c4023e14cf5a5db82c5fc` |

These retained records live under the MI350 task custody root
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`; hashes are custody
references, not public artifact download links. Qualification used
nightly-2026-04-03, offline locked Cargo and two build/test workers.

The new exact ignored parent selector is
`production_rustc_driver_v1::gfx942_bf16_call_source_cpu_qualification_v1_tests::single_strided_read::actual_single_strided_read_source_ladder`,
with a fresh `FE2O3_TEST_BF16_SINGLE_READ_OUTPUT_V1` directory.
The historical parent remains the selector documented in the
[invocation-seed checkpoint](retained-invocation-seed-20260930.md).
This does not authorize indiscriminate ignored/native test execution.

The positive source still reaches the explicit ordinary-path refusal:
BF16 nominal source-ranked projection does not yet support checked source-local
helpers. This checkpoint does not start graph/invocation propagation from the
new read, enable recovery, emit a production artifact, qualify numerical
execution or run a GPU kernel. Sampled guards and direct-child/process-group
supervision are not whole escaped-process-family closure.

The #280/#281/#282 accepted ledger remains **6/18**: M1, V1, V2, U1, U2 and U3.
No milestone is closed by this component checkpoint.
