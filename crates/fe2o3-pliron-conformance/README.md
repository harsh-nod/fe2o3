# fe2o3-pliron-conformance

This crate is a test-only integration and conformance harness for the bounded
fe2o3 Pliron surfaces. It checks fresh-context combined registration for the
feature-gated MIR dialect and the kernel, schedule, tile, GPU, proof, dispatch,
and autotune dialects through owner-scoped registration services in both
forward and reverse deterministic order. It also checks the `fe2o3-pliron`
session boundary and the bounded pointer-independent MIR-to-kernel conformance
facade.

Hostile coverage includes duplicate registration; registration-hook panic
containment with stable bounded diagnostics; terminal invalid inputs with no
fallback or prior-result reuse; deterministic observations across private
sessions; and compile-time exclusion of raw lowering registration, context,
and operation-pointer capabilities. The lowerer crate keeps equal-slot foreign
contexts, transplanted markers, and erased-handle attacks inside its private
raw-core tests.

Successful registration and MIR-to-kernel lowering are representation
observations only. They create no artifact,
physical-target, proof, publication, load, launch, or runtime authority. The
harness has no production library behavior and no COMGR, `pliron-llvm`, AMD
target or backend, HSA, HIP, filesystem, process-execution, or unsafe-code
surface.
