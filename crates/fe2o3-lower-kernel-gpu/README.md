# fe2o3-lower-kernel-gpu

`fe2o3-lower-kernel-gpu` owns the bounded, target-neutral transformation
boundary from `kernel.algorithm_root` to a deterministic bundle of `gpu.*`
operations. The bundle records abstract execution hierarchy, memory spaces,
and optional workgroup synchronization. It does not select a physical target
or produce executable code.

The current shell intentionally supports one logical source region and GPU
ranks up to three. Valid but unimplemented region counts and unsupported
source operations fail with terminal structured errors. There is no fallback
lowering.

This crate cannot lower to AMDGCN, compile, link, publish, load, launch, tune,
or grant proof or runtime authority. It has no target, runtime, filesystem,
process, COMGR, or `pliron-llvm` dependency. Its own source forbids unsafe
code; pinned Pliron remains part of the memory-safety trusted computing base.

The Pliron `Pass` adapter reports the source IR as unchanged because this
shell materializes detached operations. Callers retrieve that explicit bundle
from the pass result instead of treating it as an in-place rewrite.
