# fe2o3 legacy compiler adapter

`fe2o3-legacy-compiler` defines the adapter contract for presenting the
existing compatibility compiler path as a bounded compiler-driver backend.
It does not contain or move the existing codegen implementation, and no
production selection path depends on this crate yet.

The adapter accepts only `PipelineSelectorV1::Legacy`. Requests for either
Pliron selector are rejected before the wrapped path is invoked. A future
integration can implement `LegacyCompilePathV1` in the crate that owns the
existing implementation without transferring implementation ownership here.

## Authority boundary

The adapter returns only compiler API transaction records. It does not invoke
COMGR, publish artifacts, load modules, dispatch work, or launch kernels. An
opaque executable candidate returned by the legacy path grants none of those
authorities.
