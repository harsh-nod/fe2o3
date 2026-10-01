//! Heap-only observation of the two V5 evidence owner shapes.
//! No codec, provenance, validation, or constructor semantics are changed.

use super::{InertProductionMiddleEndEvidenceV5, ProductionMiddleEndEvidenceV5};
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};
use std::ops::Range;

fn fixed<T: Copy>(_: &T) {}

impl InertProductionMiddleEndEvidenceV5 {
    /// Charge the actual canonical Box payload, with one collection visit.
    ///
    /// The caller accounts this owner's inline header and root visit once.
    /// All summaries, identities, and the ranked-IR range are inline; the range
    /// addresses bytes already inside the canonical Box, not another allocation.
    /// This one charge is atomic on arithmetic/byte/item refusal. It does not
    /// allocate, decode, hash, or confer trust on this inert value.
    pub fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self {
            source_semantic_identity,
            ranked_kernel_identity,
            ranked_ir_range,
            coverage,
            semantics,
            typed_summary,
            reconciliation,
            identity,
            canonical_bytes,
        } = self;
        fixed(source_semantic_identity);
        fixed(ranked_kernel_identity);
        let Range { start, end } = ranked_ir_range;
        fixed(start);
        fixed(end);
        fixed(coverage);
        fixed(semantics);
        fixed(typed_summary);
        fixed(reconciliation);
        fixed(identity);
        counter.array::<u8>(canonical_bytes.len())
    }
}

impl ProductionMiddleEndEvidenceV5 {
    /// Charge the same actual Box owned through the inline inert wrapper.
    ///
    /// Excludes both inline headers and a root visit: they are already part of
    /// the one enclosing owner header. The single collection charge is atomic.
    /// Observing bytes does not construct or renew live-produced provenance.
    pub fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        let Self { inert } = self;
        inert.charge_retained_heap_storage_v1(counter)
    }
}

#[cfg(test)]
#[path = "middle_end_evidence_retained_storage_v1_tests.rs"]
mod tests;
