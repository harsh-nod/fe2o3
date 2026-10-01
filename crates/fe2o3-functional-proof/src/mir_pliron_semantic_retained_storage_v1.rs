//! Dependency-free observation of the actual MIR/PLIRON semantic contract.
//! The caller supplies one shared bounded arithmetic ledger.

use super::{
    MirPlironSemanticContractV1, SemanticCollectiveContractV1, SemanticFiniteDomainV1,
    SemanticFiniteExtentV1, SemanticLoopContractV1, SemanticOutputContractV1, SemanticTypedRootV1,
};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn fixed_slice<T: Copy>(_: &[T]) {}

// These rows are not Copy, but currently own no nested heap. Exhaustive
// compile-time shape guards check every field without scanning the row arrays.
const _: fn(&SemanticLoopContractV1) = |row| {
    let SemanticLoopContractV1 {
        identity,
        header_block,
        latch_block,
        exit_block,
        iteration_domain,
        induction,
        lower_bound,
        upper_bound,
        step,
        transition,
        variant,
        direction,
        maximum_steps,
    } = row;
    fixed(identity);
    fixed(header_block);
    fixed(latch_block);
    fixed(exit_block);
    fixed(iteration_domain);
    fixed(induction);
    fixed(lower_bound);
    fixed(upper_bound);
    fixed(step);
    fixed(transition);
    fixed(variant);
    fixed(direction);
    fixed(maximum_steps);
};

const _: fn(&SemanticCollectiveContractV1) = |row| {
    let SemanticCollectiveContractV1 {
        identity,
        kind,
        view_identity,
        source_domain,
        target_domain,
        actual,
        expected,
        witness0,
        witness1,
        domain_bound,
        step_bound,
        order,
        coverage,
    } = row;
    fixed(identity);
    fixed(kind);
    fixed(view_identity);
    fixed(source_domain);
    fixed(target_domain);
    fixed(actual);
    fixed(expected);
    fixed(witness0);
    fixed(witness1);
    fixed(domain_bound);
    fixed(step_bound);
    fixed(order);
    fixed(coverage);
};

impl MirPlironSemanticContractV1 {
    /// Visit all actual retained heap payloads as (element count, width).
    ///
    /// Visits five outer Boxes, each domain's extent Box, and each output's
    /// auxiliary-root Box: exactly 5 + domains.len() + outputs.len() callbacks.
    /// Empty collections still receive a zero-count callback. Each outer Box
    /// includes its inline row headers; nested Box headers must not be counted
    /// again. The caller accounts this contract's enclosing header/root once;
    /// this method emits no separate root visit.
    ///
    /// The callback must checked-multiply count by width, checked-add bytes and
    /// items, apply explicit remaining byte/item limits, and charge one item per
    /// callback on the same enclosing counter. The first callback precedes all
    /// variable traversal. Each nested row is reached only after its outer Box
    /// callback succeeds; no value scans precede that charge.
    ///
    /// The first callback Err is returned immediately. Accepted prefix effects
    /// are not rolled back: discard the entire observation on Err. A permissive
    /// callback is not itself a bounded or complete enclosing-owner observation.
    /// The visitor does not allocate, hash, validate, clone, or grant authority.
    /// Box lengths describe logical retained payloads, not allocator rounding,
    /// temporary constructor sets, stack/scratch, peak, or RSS.
    pub fn visit_retained_heap_storage_v1<E>(
        &self,
        mut visit: impl FnMut(usize, usize) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self {
            safe_reference_mir,
            kernel_mir,
            pliron_evidence,
            domains,
            typed_roots,
            loops,
            collectives,
            outputs,
        } = self;
        fixed(safe_reference_mir);
        fixed(kernel_mir);
        fixed(pliron_evidence);

        visit(domains.len(), size_of::<SemanticFiniteDomainV1>())?;
        for domain in domains {
            let SemanticFiniteDomainV1 { identity, extents } = domain;
            fixed(identity);
            fixed_slice(extents);
            visit(extents.len(), size_of::<SemanticFiniteExtentV1>())?;
        }

        fixed_slice(typed_roots);
        visit(typed_roots.len(), size_of::<SemanticTypedRootV1>())?;
        visit(loops.len(), size_of::<SemanticLoopContractV1>())?;
        visit(collectives.len(), size_of::<SemanticCollectiveContractV1>())?;
        visit(outputs.len(), size_of::<SemanticOutputContractV1>())?;
        for output in outputs {
            let SemanticOutputContractV1 {
                identity,
                view_identity,
                output_domain,
                actual,
                reference,
                auxiliary_roots,
            } = output;
            fixed(identity);
            fixed(view_identity);
            fixed(output_domain);
            fixed(actual);
            fixed(reference);
            fixed_slice(auxiliary_roots);
            visit(
                auxiliary_roots.len(),
                size_of::<fe2o3_proof_contracts::DigestV1>(),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "mir_pliron_semantic_retained_storage_v1_tests.rs"]
mod tests;
