# Genuine Rust-source BF16 CPU observation — 2026-09-25

This checkpoint adds numerical execution of the actual, unchanged Rust-produced
whole graph to the earlier [inspection and normal-continuation work](source-transport-tiled-debugger-qualification-20260924.md).
It is a bounded CPU observation, not edited-tile promotion, general BF16
semantics, machine equivalence, protected finalization or hardware qualification.
Accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

## One actual source and engine

The 3,950-byte `tiled-region-inspection-v1/src/lib.rs` fixture is unchanged
(SHA256 fcb26135ad4f931bb8dda63d631639a34dd8461e7801c22a1f6a383e0e735a3e).
The genuine rustc callback retains the original source transaction and its
Budget. The CPU view is independently decoded and equality-checked against
that live verified V12 owner, not reconstructed from an observation report.
The source graph is 8,049 canonical bytes, 20 blocks and 323 operations.

`V12CpuObservationInputV1` borrows the verified owner and request.
`with_v12_cpu_observation_v1` invokes the existing engine once on the whole
unchanged graph. It does not substitute a matrix-only graph, second simulator,
new source owner or reset ledger. Its observer receives a borrowed execution
result/error and can return only a Copy, static value; successful callback
return alone does not mean successful execution. The execution result is
dropped before releasing the prepaid scope on success, error or unwind.

The finite profile retains one WG64, full Wave64 participation, at most
131,072 steps and 65,536 delivered debug records. Its 128-MiB logical
prepayment includes engine, bounded transient observation and result/error
coexistence. This is neither measured RSS nor a wall-time guarantee.
Caller inputs and retained summaries remain separately charged to the same
original Budget. No storage/work ceiling is increased by this checkpoint.

The actual source exposed two overly narrow preflight cases. The profile now
admits exactly full-wave LaneId, which the engine already executes. Call-depth
collection now agrees with the existing scanner/executor: the exact zero-argument
diagnostic Trap is a terminating builtin, not an extra callee frame. Near names,
wrong arity and real helper/recursive calls retain their original handling;
the call-depth limit remains one. Fixed-size refusal summaries made these
failures diagnosable without retaining an error graph or allocating its text.

## Numerical and negative cases

Four fresh rustc children qualify direct execution, wrong source launch,
observer error and observer unwind. The direct child performs 34 attempts:

- Six independent input patterns (zero, identity, dense mixed signs,
  positive/negative extrema and cancellation), each with output lengths 64, 13, 0:
  18 positive runs.
- Sixteen precise controls for uninitialized input, excluded numerical values,
  step/record limits, stopped capture, failing event sink and unsupported launch.

The independent oracle uses dense signed-integer multiplication and an integer
F32 bit encoder; it does not call the simulator's numerical evaluator.
Every positive run checks all 256 actual SSA matrix results, including all four
components in every lane. The source stores only component zero, so the separate
272-byte output/canary check follows that actual projection and each output tail.
It is not a claim that this fixture stores a full 16x16 output tile.

All positive attempts restore their original logical storage floor 875,607.
The direct source phase retains cumulative work 4,798,397,636,508,952 with no
sticky work/storage denial. Error and panic cases retain the 63,600-byte copied
summary charge while unwinding the original source/materializer lifetime.
These are logical observations, not peak process-memory measurements.

RecordLimit and DebugStop mean zero DELIVERED Matrix-completion/global-write
observations in those controls. The engine may continue after delivery stops.
Missing records do not establish cancellation, absence of effects or rollback.
Full 64-lane masks are unsigned 64-bit values: consumers must preserve the exact
decimal token or use a lossless decoder, not JavaScript Number.

## Retained executed evidence

The first passing actual-source gate is
`compiler-bf16-source-cpu-actual-r4`:

- Outer receipt 93,791 bytes,
  b7d726d610e312694b1137359c26b588618d77430193f619469dd293f7c2512f.
- Actual report 100,796 bytes,
  38a6527d121e4cb9db2555aa8d64a8a29e64907c28e8fceb2e07ae3ad2d196fa.
- Source census 8,028 files / 115,915,064 bytes /
  9ff9f2ed9bb8de48f140c1f9ad3de64597666d9a03a532359bff891c4635f351.
- Seventeen preflight unit tests and six backend CPU controls passed before
  the successful parent; its four actual rustc children are separate from
  those unit-test counts. Two child/parent entries were intentionally ignored
  by the preceding unfiltered backend-control invocation.

This was based on main 1e368326c, before integrating the separate prepared-Vecadd
commit d6b5887f5. The failed actual R1–R3 attempts remain retained, including
their exact first refusals; they are not passing results. The receipt and
report are inert audit evidence, not source/artifact/launch authority.
The bounded direct-child/process-group runner is not a whole-family proof.

## Reproduction and remaining boundary

From the pinned configured checkout with cached dependencies, choose a fresh,
nonexistent output directory; do not overwrite a previous attempt:

~~~sh
FE2O3_TEST_TILED_CPU_OUTPUT_V1=/absolute/new/output \
cargo test --offline --locked -p rustc-codegen-fe2o3 --lib \
  production_rustc_driver_v1::gfx942_tiled_region_qualification_v1_tests::observation::cpu::actual_bf16_source_cpu_ladder \
  -- --exact --ignored --nocapture
~~~

The existing closed numerical domain is integral BF16 inputs in [-16, 16] and
the admitted F32 accumulator contract, with positive zero. It is not general
BF16 rounding or gfx950 numerical support. Ordinary bounds, initialization,
race and convergence checks still apply. This test stops after source analysis;
the previously qualified normal continuation is a different invocation, not
a claim that this numerical callback emitted or launched the final artifact.
Tiled-region editing, complete caller/output machine refinement, generated
protected host admission, target-matched hardware and the remaining curriculum
still require their own evidence.

## Fresh merged regression and source runs

After the prepared conditional-Vecadd peer commit d6b5887f55a4b7592b6905f4415ec88b544a0191
was integrated without changing this source, the complete selected regression
and all three fresh source ladders passed together. The selected gate recorded
7,249 successful Rust test executions in 118 result groups, zero failures and
286 intentionally ignored entries. These are executions across overlapping
configurations, not 7,249 unique tests or a whole-workspace qualification.
The gate also checked strict simulator Clippy, the unsafe-source inventory,
backend/extractor builds and selected KFD CPU/build/doc paths; no GPU ran.

The completed outer receipt is 580,880 bytes, SHA-256
0c344fa20a8247724ea019792ecabbdbadd4b92e4892b5e07f9d7dc912b11c2f.
Its unchanged source census is 8,094 files / 116,505,691 bytes /
791f0f8693d281eacad612b707ddb9f1468327710a9edd43705de88bc3189fc5.
The earlier broad R1 functional tests passed, but that gate failed strict
Clippy; it remains failed. R2 includes the corrected equivalent let-chain and
reruns the selected suite from the lint gate onward.

The fresh actual-source reports are separate invocations:

| Gate | Actual rustc children | Report bytes | SHA-256 |
| --- | ---: | ---: | --- |
| Source inspection | 4 | 32,806 | bfe497977d83d08a24e1face68d8672e8d9cb3ad525698b125ade54ed1e49358 |
| Normal compilation and inert handoff | 2 | 26,711 | 0162ddfe1a10a67bab3abdaa9bff73786bd888f0254a4681b4e6b4cf32c26ae5 |
| Numerical CPU R5 | 4 | 100,796 | 15e996f1fe49adfeb8d25c1c6fbd0295e51d2e8c83a2547f262b45ca182b3844 |

R5 again passes 18 positives and 16 precise request refusals, with original
ledger restoration, exact 64-bit lane masks and the unchanged canonical graph.
The source-only and normal reports still say numerical qualification false;
the CPU report still says normal handoff false. Their evidence is not merged
into a fictitious single compilation, and none grants launch authority.

## Publication checks and known repository-wide failures

Changed Rust formatting, the new standalone locked metadata, workspace/Pliron
dependency policy (15 controls), codegen shard policy, unsafe inventory and
the relocated producer checks passed again after publication formatting.
All three strict C++ fixtures were rebuilt and rerun, including the output
fixture with its trailing blank line removed. The completed final R3 receipt
is 154,197 bytes, SHA-256
6906c7133b7d2c3edd699c8e72a7628c6acced3f72f25e9d0b71d6e2c464a7a1.

Two broader preflight attempts did not pass. Whole-workspace rustfmt requests
formatting changes in the existing ordered_composition_publish_v1.rs source
fixture. The complete standalone-lockfile scan stops because the existing
complete-body-packing-consumer lockfile needs regeneration under --locked.
Those source/manifest/lock bytes are identical to base main d6b5887f5 and were
not changed by this batch. The failed gates are retained, not silently skipped
or relabeled green. This checkpoint does not claim a passing complete generic
CI or whole-workspace formatting/standalone-lockfile gate.
