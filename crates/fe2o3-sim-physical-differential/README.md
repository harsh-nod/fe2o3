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
```

The normal protected Worker V3 application verifier is not wired yet, so the
capability response reports `protected_verifier_unavailable`. The legacy
handwritten LLVM vecadd fixture is excluded and cannot be used as same-body
parity evidence. Generated hosts can inspect
`GeneratedWorkerV3KfdInvocation::differential_availability` before launch and
can obtain a sealed observation only through `execute_for_differential`.

This crate is direct-KFD-only. It has no HIP or HSA runtime path.
