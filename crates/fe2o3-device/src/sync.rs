/// Executes one uniform workgroup barrier with acquire-release ordering over
/// workgroup memory.
///
/// This low-level entry point is intentionally unsafe. Prefer deriving a typed
/// [`crate::Workgroup`] and consuming a [`crate::WorkgroupConvergence`] witness.
/// The current compiler does not recognize or lower this function, so calling
/// it on a host or through an unsupported compilation path always panics.
///
/// # Safety
///
/// Every active work-item in the current workgroup must execute this exact
/// dynamic call once and in the same barrier sequence. No work-item may reach
/// it through non-uniform control flow, return before it, or skip it. The
/// compiler must preserve all of the following semantics:
///
/// - workgroup execution scope;
/// - workgroup memory scope;
/// - acquire-release ordering over workgroup memory; and
/// - uniform workgroup convergence.
///
/// Calling this function without compiler recognition that preserves those
/// properties does not synchronize a device program.
#[inline(never)]
pub unsafe fn syncthreads() {
    unreachable!("syncthreads must be lowered by the fe2o3 backend")
}
