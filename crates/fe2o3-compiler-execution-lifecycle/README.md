# fe2o3-compiler-execution-lifecycle

This crate owns the descriptor-relative admission and close-only custody of one
protected service's shared compiler-execution lifecycle lease. The root
coordinator passes independently opened lock descriptions to the supervisor
and external anchor so either protected child continues excluding provisioning
if the coordinator exits without running destructors.

The value grants no compiler, signing, publication, linking, loading, launch,
execution, or GPU authority. It is move-only and releases its lease only when
the last duplicate of its own open file description closes.
