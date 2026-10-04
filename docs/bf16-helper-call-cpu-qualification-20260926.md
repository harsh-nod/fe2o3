# Closed BF16 helper-call CPU observation — 2026-09-26

This checkpoint adds a distinct, bounded two-frame CPU observation API for an
already admitted canonical graph. It does not yet qualify emission or numerical
execution from genuine Rust helper source, normal LLVM continuation, hardware,
or additional broad milestone exits.

## Implemented profile

`Bf16CallCpuObservationOptionsV1` and
`AdmittedSimulationModuleV1::with_bf16_call_cpu_observation_v1` retain the actual
root and internal helper, a single static call, twelve scalar inputs (eight BF16
and four F32), one full-wave matrix operation, and four return components.
Identity and Swap01 returns are supported. The helper graph is acyclic and its
block-edge transport is identity; every block is covered. Existing root-only
observation remains a separate depth-one profile.

The new profile allows exactly two live frames. Existing bounds remain:
16 KiB canonical bytes, 32 combined blocks, 1,024 operations, 512 definitions,
four kernel arguments, one 64-lane workgroup, 131,072 steps, 65,536 records,
128 MiB phase storage, 64 MiB engine storage and original source work at most
2^54. Conservative work is prepaid on the original ledger. The result remains
borrowed inside the callback; errors, panic and budget denial do not leak a
result or clear sticky denial state. Failed outputs are dropped before refund.

## Executed on mi350

The focused gate passed ten new tests (two unit and eight integration) and
24 compatibility tests. It checked both actual helper/caller frames, all four
SSA components against the independent matrix oracle, initialized buffers,
output canaries, domain/divergence refusal, exact and one-short budgets, and
callback/error/unwind cleanup.

The subsequent all-target simulator, CLI, debugger and runtime regression,
plus the unsafe-source-policy gate, passed 753 test executions with zero
failures and four ignored tests in 49 result groups. Strict simulator Clippy
(`--all-targets --no-deps -- -D warnings`) and `git diff --check` passed.
These are execution counts, not unique test counts.

Two earlier regression attempts retained passing test output but failed lint:
a test module preceded production items in the touched observation module,
and an existing fixture used a manual divisibility expression. The module was
moved without semantic changes and the fixture now uses `is_multiple_of(2)`.
The failed receipts are preserved; they are not counted as passing gates.

Root-retained evidence under
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`:

- Focused R18 receipt:
  `logs/phase28-resume-r18-compiler-bf16-call-cpu-facade-r1/receipt.json`,
  22,555 bytes, SHA-256
  `3bbb7c2d9e4fa5b09ab478b7b2b31d9c3c014deac326ab6c0688318f2c557850`.
- Broad R19 receipt:
  `logs/phase28-resume-r19-compiler-bf16-call-cpu-regression-r3/receipt.json`,
  22,740 bytes, SHA-256
  `6b495ff7953bb71d595dab3f3591d121a4b568b53950d99374de31cb0070f60c`.
- Broad tested source census: 8,225 files / 118,014,058 bytes, SHA-256
  `8126e67b17bbe86bd745716a927c068c4327c77e31a2c68c9a73d694dd487cbe`.

The root independently checked request/input/stream hashes and unchanged
before/after source and tool records. Documentation added afterward is not part
of that tested source census.

## Remaining boundaries

The genuine-source helper-emission and CPU ladders are separate work. Their
same-owner reconstruction must retain both envelopes under the unchanged 2 GiB
compiler phase budget. This canonical CPU checkpoint does not substitute for
those runs, general helper ABI support, source round-trip, physical register
mapping, or GPU validation. Accepted broad exits remain
**M1/V1/V2/U1/U2/U3 (6/18)**.
