fn prepare_formal_admission_payload_v5(
    admitted: &ProductionFormalMemoryOwnerV1,
    publication: bool,
    middle_end: &[u8],
    semantic_identity: &[u8; 32],
    neutral_kir: ProductionCanonicalKernelIrIdentityV1,
) -> Result<Vec<u8>, ProductionSemanticLineageErrorV3> {
    let bytes = if publication {
        fe2o3_lower_mir_kernel::InertCanonicalFormalMemoryAdmissionEvidenceV5::from_live_owner(admitted)
            .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?
            .into_canonical_bytes()
    } else {
        InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(admitted)?
            .canonical_bytes().to_vec()
    };
    if validate_formal_middle_end_pair_v5(middle_end, &bytes, semantic_identity)? != neutral_kir {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "formal admission names a different current KIR",
        ));
    }
    Ok(bytes)
}

// Production chooses one closed pair, never a historical record with an
// unrepresented HB proof. This decoder authenticates custody relationships,
// not a runtime allocation, target memory model, or launch permission.
fn validate_formal_middle_end_pair_v5(
    middle_end: &[u8],
    formal_memory: &[u8],
    semantic_identity: &[u8; 32],
) -> Result<ProductionCanonicalKernelIrIdentityV1, ProductionSemanticLineageErrorV3> {
    let invalid = || ProductionSemanticLineageErrorV3::AxisMismatch(
        "middle-end and formal publication evidence are cross-wired",
    );
    if formal_memory.starts_with(b"F2FMA4\0\0") {
        let formal = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(formal_memory)?;
        let middle = InertProductionMiddleEndEvidenceV5::decode(middle_end)
            .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;
        if middle.source_semantic_identity() != semantic_identity
            || formal.grants_authority() || middle.grants_artifact_or_launch_authority()
        { return Err(invalid()); }
        return Ok(formal.canonical_kernel_ir_identity());
    }
    let formal = fe2o3_lower_mir_kernel::InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(formal_memory)
        .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;
    let middle = fe2o3_pliron::InertProductionMiddleEndEvidenceV6::decode(middle_end)
        .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;
    let (Some(formal_publication), Some(middle_publication)) =
        (formal.static_publication(), middle.static_publication())
    else { return Err(invalid()); };
    if formal.source_semantic_identity() != semantic_identity
        || middle.source_semantic_identity() != semantic_identity
        || formal.ranked_graph_identity() != middle.live_ranked_graph_identity()
        || formal.unresolved_inter_invocation_conflict_count() != 0
        || formal_publication.payload_origin() != middle_publication.payload_origin()
        || formal_publication.flags_origin() != middle_publication.flags_origin()
        || formal_publication.global_extents() != [256, 1, 1]
        || formal_publication.workgroup_extents() != [128, 1, 1]
        || formal_publication.ranked_potentially_conflicting_cell_pairs()
            != middle_publication.potentially_conflicting_cell_pairs()
        || formal.grants_authority() || middle.grants_artifact_or_launch_authority()
    { return Err(invalid()); }
    for (formal_site, middle_site) in formal_publication.ranked_sites().iter().zip(middle_publication.sites()) {
        if *formal_site != [middle_site.block(), middle_site.operation()] { return Err(invalid()); }
    }
    Ok(formal.canonical_kernel_ir_identity())
}

#[cfg(test)]
mod static_publication_lineage_tests {
    use super::*;
    #[test]
    fn publication_lineage_rejects_missing_truncated_or_future_records() {
        for bytes in [b"".as_slice(), b"F2FMA4\0\0", b"F2FMA5\0\0", b"F2FMA6\0\0"] {
            assert!(validate_formal_middle_end_pair_v5(&[], bytes, &[1; 32]).is_err());
        }
    }
}

#[cfg(test)]
mod publication_model_pair_v1_tests {
    include!("publication_model_pair_v1_tests.rs");
}
