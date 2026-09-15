//! Structural identity consistency, not nominal type or value equivalence.

use super::SemanticTypeLayoutV1;

/// The producer hashes target + rustc LayoutData, not the semantic type shape.
/// Shape-dependent aggregate details must still pass validate_type separately.
/// Keep full record equality for source replay, ABI adjustments and encoding.
pub(super) fn same_structural_layout_v1(
    left: &SemanticTypeLayoutV1,
    right: &SemanticTypeLayoutV1,
) -> bool {
    // Exhaustive destructuring makes a newly retained layout field a review
    // obligation. Raw variants and largest_niche remain strictly equal;
    // shape-dependent semantic details are excluded from this comparison.
    let SemanticTypeLayoutV1 {
        rustc_size_bytes,
        size_bytes,
        alignment_bytes,
        fields,
        variants,
        backend_repr,
        largest_niche,
        uninhabited,
        max_repr_alignment_bytes,
        unadjusted_abi_alignment_bytes,
        randomization_seed,
        details: _,
    } = left;
    *rustc_size_bytes == right.rustc_size_bytes
        && *size_bytes == right.size_bytes
        && *alignment_bytes == right.alignment_bytes
        && fields == &right.fields
        && variants == &right.variants
        && backend_repr == &right.backend_repr
        && *largest_niche == right.largest_niche
        && *uninhabited == right.uninhabited
        && *max_repr_alignment_bytes == right.max_repr_alignment_bytes
        && *unadjusted_abi_alignment_bytes == right.unadjusted_abi_alignment_bytes
        && *randomization_seed == right.randomization_seed
}

#[cfg(test)]
mod tests;
