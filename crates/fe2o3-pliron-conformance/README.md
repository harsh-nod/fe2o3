# fe2o3-pliron-conformance

This crate is a test-only integration and conformance harness for the bounded
fe2o3 Pliron surfaces. It checks fresh-context combined registration for the
feature-gated MIR dialect and the kernel, schedule, tile, GPU, proof, dispatch,
and autotune dialects through owner-scoped registration services in both
forward and reverse deterministic order. It also checks the `fe2o3-pliron`
session boundary, the bounded MIR-to-kernel and kernel-to-GPU detached lowering
services, and exact canonical KIR V1-V5 bridge envelopes.

Hostile coverage includes duplicate, colliding, and corrupt registration;
registration-hook panic containment with stable bounded diagnostics;
terminal unsupported inputs with no fallback or prior-result reuse; stale and
erased source or output handles; rejection of registered, populated foreign
contexts before stored pointers are dereferenced; bridge bound preflights;
opaque envelope ownership; unexpected envelope metadata; shell-order mutation;
and expected-record substitution.

The tested stage outputs remain separate. MIR-to-kernel output may be supplied
explicitly to the bounded kernel-to-GPU shell, but neither lowering produces a
canonical KIR record. The KIR bridge starts from independently supplied
canonical KIR bytes and does not consume, validate, or establish semantic
correspondence with either lowering result. This harness intentionally defines
no full MIR-to-KIR connection. Trust-boundary recovery is always bound to an
expected canonical KIR record; self-consistency alone is not acceptance.

Successful registration, lowering, and KIR envelope round-trips are
representation observations only. They create no artifact,
physical-target, proof, publication, load, launch, or runtime authority. The
harness has no production library behavior and no COMGR, `pliron-llvm`, AMD
target or backend, HSA, HIP, filesystem, process-execution, or unsafe-code
surface.
