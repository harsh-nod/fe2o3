//! Dependency-free heap visitor for the actual parallel-reference owner.
//! The enclosing observer supplies checked arithmetic and explicit bounds.

use super::{ParallelOutputRelationV1, ParallelReferenceContractV1};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn fixed_slice<T: Copy>(_: &[T]) {}

impl ParallelReferenceContractV1 {
    /// Visit each separately owned heap allocation as (element count, width).
    ///
    /// The first callback covers the relation Box, including its inline row
    /// headers. Each subsequent callback covers one row's hierarchy Box. Box
    /// extents are their actual lengths, not capacities of consumed input Vecs.
    /// The caller accounts the enclosing owner header/root once, and must check
    /// multiplication, cumulative addition, byte limits, and one item per callback
    /// on the same enclosing counter. No root visit is emitted by this method.
    ///
    /// Callback refusal immediately stops traversal, including before scanning
    /// any relation if the first callback refuses. Earlier callback effects are
    /// not rolled back; discard the entire observation on Err. A permissive
    /// callback does not establish a bounded or complete enclosing observation.
    /// No hashing, admission, allocation, cloning, or authority is performed.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            semantic_contract_identity,
            output_product_identity,
            relations,
        } = self;
        fixed(semantic_contract_identity);
        fixed(output_product_identity);
        visit(relations.len(), size_of::<ParallelOutputRelationV1>())?;
        for relation in relations {
            let ParallelOutputRelationV1 {
                identity,
                output_contract,
                logical_domain,
                ranked_view_identity,
                ownership_identity,
                frame_identity,
                schedule,
                numerical_policy,
                hierarchy,
                tensor_refinement_identity,
                policy_checked_staging_identity,
            } = relation;
            fixed(identity);
            fixed(output_contract);
            fixed(logical_domain);
            fixed(ranked_view_identity);
            fixed(ownership_identity);
            fixed(frame_identity);
            fixed(schedule);
            fixed(numerical_policy);
            fixed(tensor_refinement_identity);
            fixed(policy_checked_staging_identity);
            fixed_slice(hierarchy);
            visit(
                hierarchy.len(),
                size_of::<super::ParallelHierarchyLevelV1>(),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "parallel_reference_retained_storage_v1_tests.rs"]
mod tests;
