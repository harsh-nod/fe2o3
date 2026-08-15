use vstd::prelude::*;

#[path = "../lds_tiled_slice1_source_refinement.rs"]
mod model;

verus! {

/// Mutation: the LDS read is moved to the publish event instead of strictly
/// after it.
pub proof fn mutated_read_at_publish_event_refines_canonical_ir_v1()
    ensures model::source_to_canonical_ir_correspondence_v1(
        model::AttributedSlice1SourceV1 {
            lds_read_event:
                model::exact_attributed_slice1_source_v1().publish_barrier_event,
            ..model::exact_attributed_slice1_source_v1()
        },
        model::exact_canonical_slice1_ir_v1(),
    ),
{
}

} // verus!
