//! Dependency-free observation of the actual retained induction report.
//! Analysis work receipts are not storage measurements.

use super::{SemanticReachableScopeV2, SemanticU32InductionNoOverflowReportV1};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn fixed_slice<T: Copy>(_: &[T]) {}

impl SemanticU32InductionNoOverflowReportV1 {
    /// Visit the certificate Box and, when present, the reachability Vec<bool>.
    ///
    /// Callbacks receive (element count, width): Box length and actual Vec
    /// capacity respectively. Rust Vec<bool> is a Vec of bool slots, not a
    /// compressed bitset. Certificates contain only fixed-width Copy payloads.
    /// Inline report/optional-scope/collection headers are excluded; the caller
    /// accounts the enclosing header and root once. This emits one collection
    /// callback, or two when the actual optional owner is present.
    ///
    /// The caller must check multiplication, cumulative addition, byte limits,
    /// and one item per callback on the same enclosing bounded counter. A
    /// refusal is returned immediately, with no later callback; earlier callback
    /// effects remain and the entire observation must be discarded on Err.
    /// No borrowed MIR, temporary CFG/SSA owners, work receipt, allocator
    /// overhead, scratch, stack, or peak/RSS is counted. No analysis is repeated.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            semantic_mir_sha256,
            function,
            function_identity,
            checked_additions_examined,
            certificates,
            work_units,
            reachable_scope,
            ssa_scope_work_units,
            reachable_blocks,
        } = self;
        fixed(semantic_mir_sha256);
        fixed(function);
        fixed(function_identity);
        fixed(checked_additions_examined);
        fixed(work_units);
        fixed(reachable_scope);
        fixed(ssa_scope_work_units);
        fixed_slice(certificates);
        visit(
            certificates.len(),
            size_of::<super::SemanticU32InductionNoOverflowCertificateV1>(),
        )?;
        if let Some(scope) = reachable_blocks {
            let SemanticReachableScopeV2 {
                blocks,
                block_count,
                statement_count,
            } = scope;
            fixed(block_count);
            fixed(statement_count);
            visit(blocks.capacity(), size_of::<bool>())?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "semantic_u32_induction_retained_storage_v1_tests.rs"]
mod tests;
