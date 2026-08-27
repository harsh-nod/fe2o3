# fe2o3-pliron-conformance

This crate is a test-only integration and conformance harness for the bounded
fe2o3 Pliron surfaces. It checks fresh-context combined registration for the
feature-gated MIR dialect and the kernel, schedule, tile, GPU, proof, dispatch,
and autotune dialects through owner-scoped registration services in both
forward and reverse deterministic order. It also checks the `fe2o3-pliron`
session boundary and the retained bounded MIR-to-kernel service.

Hostile coverage includes duplicate, colliding, and corrupt registration;
registration-hook panic containment with stable bounded diagnostics;
terminal unsupported inputs with no fallback or prior-result reuse; stale and
erased source or output handles; and rejection of registered, populated foreign
contexts before stored pointers are dereferenced.

Successful registration and MIR-to-kernel lowering are representation
observations only. They create no artifact,
physical-target, proof, publication, load, launch, or runtime authority. The
harness has no production library behavior and no COMGR, `pliron-llvm`, AMD
target or backend, HSA, HIP, filesystem, process-execution, or unsafe-code
surface.
