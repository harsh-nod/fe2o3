# fe2o3 virtual runtime

`fe2o3-virtual-runtime` composes the deterministic KIR V7 CPU simulator with
the syscall-free runtime lifecycle model. It provides bounded virtual
allocations, host copies, queues, dependency-ordered dispatches, completions,
pre-publication cancellation, and explicit ambiguous-completion recovery
without requiring a GPU. Submitted
views are validated and storage-bounded before preparation. Host access remains
blocked while a dispatch retains an allocation, and ambiguous writable ranges
remain uninitialized after quiesced settlement.

`submit_with_dynamic_workgroup_memory` carries an explicit
`DynamicWorkgroupMemoryRequestV1` alongside the otherwise unchanged virtual
dispatch. Its byte extent is included in the simulator schedule transcript and
successful completion summary. Admission still requires exactly one reachable
canonical dynamic LDS base; missing, ambiguous, misaligned, and
`DynamicAtLeast` contracts fail through the simulator's typed boundary.

An expired completion wait records a typed ambiguity reason. It never claims
that virtual or physical execution stopped and never releases retained storage
until the queue is explicitly quiescent.

`reset_generation` first proves complete terminalization against the canonical
runtime model, then atomically installs an empty runtime under a mandatory
fresh identity. Old handles therefore fail as foreign instead of aliasing a
replacement allocation, module, queue, or completion.

The crate accepts only `AdmittedSimulationModuleV1`; it does not compile source
or parse unverified KIR. Handles are runtime-identity-bound ordinals, never host
pointers or GPU virtual addresses. Every successful outcome reports semantic
CPU simulation only. It grants no compiler, proof, artifact, load, launch,
hardware, equivalence, performance, or universal-correctness authority.

The companion `fe2o3-virtual-runtime-cli` reuses the hardened
`fe2o3-kir-sim-cli` KIR/bundle admission boundary and emits stable JSON.
