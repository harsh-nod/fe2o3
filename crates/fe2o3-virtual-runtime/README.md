# fe2o3 virtual runtime

`fe2o3-virtual-runtime` composes the deterministic KIR V7 CPU simulator with
the syscall-free runtime lifecycle model. It provides bounded virtual
allocations, host copies, queues, dependency-ordered dispatches, completions,
pre-publication cancellation, and explicit ambiguous-completion recovery
without requiring a GPU. Submitted
views are validated and storage-bounded before preparation. Host access remains
blocked while a dispatch retains an allocation, and ambiguous writable ranges
remain uninitialized after quiesced settlement.

The crate accepts only `AdmittedSimulationModuleV1`; it does not compile source
or parse unverified KIR. Handles are runtime-identity-bound ordinals, never host
pointers or GPU virtual addresses. Every successful outcome reports semantic
CPU simulation only. It grants no compiler, proof, artifact, load, launch,
hardware, equivalence, performance, or universal-correctness authority.

The companion `fe2o3-virtual-runtime-cli` reuses the hardened
`fe2o3-kir-sim-cli` KIR/bundle admission boundary and emits stable JSON.
