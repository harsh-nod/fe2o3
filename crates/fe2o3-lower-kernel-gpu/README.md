# fe2o3-lower-kernel-gpu

`fe2o3-lower-kernel-gpu` owns the bounded, target-neutral detached lowering
service from `kernel.algorithm_root` to a deterministic bundle of `gpu.*`
operations. The bundle records abstract execution hierarchy, memory spaces,
and optional workgroup synchronization. It does not select a physical target
or produce executable code.

The current shell intentionally supports one logical source region and GPU
ranks up to three. Valid but unimplemented region counts and unsupported
source operations fail with terminal structured errors. There is no fallback
lowering.

Results and registration markers are bound to a private, context-owned
`fe2o3-pliron` identity anchor, so moving public auxiliary-data markers cannot
transfer them to another context. The result accessor exposes contextless
Pliron `Ptr` values only for internal pipeline integration. Those values are
Pliron-TCB handles, not portable or self-authenticating references; callers
must validate the result against its owning context before using them and must
never dereference them in another context. Validation reports erased output
handles as typed errors instead of allowing Pliron traversal panics to escape.

This crate cannot lower to AMDGCN, compile, link, publish, load, launch, tune,
or grant proof or runtime authority. It has no target, runtime, filesystem,
process, COMGR, or `pliron-llvm` dependency. Its own source forbids unsafe
code; pinned Pliron remains part of the memory-safety trusted computing base.

This crate deliberately does not implement Pliron's `Pass` trait. The service
materializes detached operations outside the source root, which is not a legal
in-tree pass rewrite. Callers invoke `run_checked` and retrieve the explicit
detached bundle from the service result.
