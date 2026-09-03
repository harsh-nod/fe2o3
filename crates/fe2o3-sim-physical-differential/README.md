# fe2o3 simulator/direct-KFD physical differential

This opt-in integration crate owns the identity-bound comparison between one
canonical KIR simulator execution and a sealed observation returned by the
production generated-host Worker V3 direct-KFD completion path. Keeping this
crate separate ensures the CPU-only `fe2o3-sim-differential` and virtual runtime
closures have no generated-host, runtime-authority, or KFD dependencies.

The V1 state machine requires Bundle V4 plus the admitted
V8-production-to-V7-simulator structural bridge, verifies generated scalar and
slice packing without retaining native addresses, and distinguishes agreement,
discrepancy, and typed hardware unavailability. An unavailable GPU or protected
verifier contributes zero hardware and parity passes. A completed direct-KFD
execution that disagrees with the simulator contributes one hardware pass and
zero parity passes; runtime failure or ambiguous completion cannot mint an
observation and is never converted into a mismatch.

```text
fe2o3-sim-physical-differential physical-capabilities-v1
fe2o3-sim-physical-differential protected-physical-qualification-v2
```

The V2 qualification command enumerates every protected prerequisite without
accepting caller claims or pass counts. The sealed verifier adapter, independent
finalizer replay, and the invocation-to-comparison bridge are implemented. A
concrete protected verifier backend, protected key and Worker-ledger deployment,
an independently administered monotonic rollback authority, and authenticated
proof-to-machine, Rust-layout, and Rust-effect refinement receipt producers are
not provisioned. Per-invocation application handoff, compiler proof/target
lineage, generated packing, checked device, and unambiguous completion can be
decided only from their exact move-only owners.

`prepare_generated_worker_v3_physical_differential_v1` accepts only an already
authenticated `GeneratedWorkerV3KfdInvocation`; it accepts no verifier, HSACO,
digest, token, or address. It binds the invocation's protected differential
identity to exact simulator state, retains both owners in a single-use value,
then calls `execute_for_differential` and compares only the sealed completed
observation. Runtime, currentness, or completion failure returns an error and
cannot become unavailable, discrepancy, or agreement evidence. The normal
protected Worker V3 application verifier remains absent, so qualification is
currently unavailable and the legacy handwritten LLVM vecadd fixture remains
excluded.

This crate is direct-KFD-only. It has no HIP or HSA runtime path.
