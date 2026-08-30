# fe2o3-compiler-execution-lifecycle

This crate owns the descriptor-relative admission and close-only custody of one
protected service's shared compiler-execution lifecycle lease. The root
coordinator passes independently opened lock descriptions to the supervisor
and external anchor so either protected child continues excluding provisioning
if the coordinator exits without running destructors.

The value grants no compiler, signing, publication, linking, loading, launch,
execution, or GPU authority. It is move-only and releases its lease only when
the last duplicate of its own open file description closes.

Services that close unrelated descriptors can request exact private custody of
the canonical parent. The anchor daemon uses private FD 258 for the lock and FD
259 for that parent, preserving pathname and inode revalidation through its
cleanup boundary. The crate's subprocess regression kills a coordinator holder
through a pidfd and checks both supervisor-first and anchor-first exit orders.
