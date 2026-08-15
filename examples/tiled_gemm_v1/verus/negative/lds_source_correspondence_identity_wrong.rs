use vstd::prelude::*;

#[path = "../lds_tiled_slice1_source_refinement.rs"]
mod model;

verus! {

/// Mutation: a one-bit portable-MIR identity drift is claimed to retain the
/// exact source-to-canonical-IR correspondence.
pub proof fn mutated_portable_mir_identity_refines_canonical_ir_v1()
    ensures model::source_to_canonical_ir_correspondence_v1(
        model::AttributedSlice1SourceV1 {
            portable_mir_identity: model::Digest256V1 {
                word0: model::source_portable_mir_identity_v1().word0 + 1,
                word1: model::source_portable_mir_identity_v1().word1,
                word2: model::source_portable_mir_identity_v1().word2,
                word3: model::source_portable_mir_identity_v1().word3,
            },
            ..model::exact_attributed_slice1_source_v1()
        },
        model::exact_canonical_slice1_ir_v1(),
    ),
{
}

} // verus!
