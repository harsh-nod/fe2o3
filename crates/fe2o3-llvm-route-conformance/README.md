# fe2o3-llvm-route-conformance

This crate supplies deterministic, generic-CI conformance fixtures for the
public gfx942 LLVM handoff V1 model. The fixture covers every address-space,
origin, obligation, and device-library kind currently expressible by that
model, together with its canonical target features, kernel calling convention,
pointer alignments, function attributes, and module metadata.

The corpus classifies each named case as a represented handoff property, an
expected typed rejection, or a coverage gap. Atomic operation, memory-order,
memory-scope, and intrinsic semantics are explicit gaps because the current
public handoff API does not represent them. The corpus also exercises the
public worker-admission V1 boundary and the closed public scalar Pliron LLVM
lowering V1 API. Unsupported lowering calls, types, address spaces, and target
policies are named expected rejections rather than inferred backend coverage.

Successful handoff fixture construction and round trips establish canonical
handoff representation only. Scalar lane success establishes only that the
public lane produced a deterministic canonical handoff and structural receipt.
Neither establishes intrinsic or atomic semantics, emitted machine
correspondence, code-object correctness, hardware behavior, device-library
contents, or publication authority. The crate does not invoke a compiler,
linker, runtime, device, or subprocess.
