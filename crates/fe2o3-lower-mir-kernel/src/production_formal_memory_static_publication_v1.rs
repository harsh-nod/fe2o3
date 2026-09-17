#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionStaticConflictReasonV1 {
    ProducerInjectivity,
    PublishedReadHappensBefore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionStaticConflictDischargeV1 {
    pub(crate) conflict: InterInvocationConflictRequirement,
    pub(crate) reason: ProductionStaticConflictReasonV1,
}

/// Kept separately from the unmodified affine obligations and incomplete LDS
/// reasons. Replay binds source custody, live ranked graph and actual KIR.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ProductionStaticPublicationDischargeV1 {
    pub(crate) source: crate::production_semantic_kir_v1::RetainedStaticPublicationRosterV1,
    pub(crate) ranked: fe2o3_pliron::RankedStaticPublicationProofV1,
    pub(crate) payload_parameter_index: u32,
    pub(crate) conflicts: Box<[ProductionStaticConflictDischargeV1]>,
}

fn derive_static_publication_discharge_v1(
    semantic_kir: &ProductionSemanticKirOwnerV1,
    kernel: &fe2o3_kernel_ir::Kernel,
) -> Result<Option<ProductionStaticPublicationDischargeV1>, ProductionFormalMemoryErrorV1> {
    use ProductionFormalMemoryErrorV1::StaticPublicationDischarge as Invalid;
    let roster = semantic_kir.retained_static_publication_roster_v1(kernel.id.as_str())
        .map_err(|_| Invalid("source, ranked, launch and KIR event roster could not be rederived"))?;
    let ranked = semantic_kir.retained_static_publication_inputs_v1(kernel.id.as_str())
        .and_then(|(input, _)| input.race_report().static_publication()).copied();
    let (source, ranked) = match (roster, ranked) {
        (None, None) => return Ok(None),
        (Some(source), Some(ranked)) => (source, ranked),
        _ => return Err(Invalid("source publication and ranked proof disagree")),
    };
    if source.global_extents != [256, 1, 1]
        || source.workgroup_extents != [128, 1, 1] || !source.full_physical_workgroups
        || ranked.maximum_invocations() != 256
        || ranked.payload_origin() != u64::from(source.payload_source_argument) + 1
        || ranked.flags_origin() != u64::from(source.flags_source_argument) + 1
        || !matches!(kernel.domain, LaunchDomain::D1 { x: LaunchExtent::Dynamic })
    { return Err(Invalid("publication maximum domain or allocation root changed")); }
    for (event, ranked_ordinal) in source.events.iter().zip([0, 1, 2, 3, 5]) {
        let site = ranked.sites()[ranked_ordinal];
        if site.block() != event.ranked_block as usize || site.operation() != event.ranked_operation as usize {
            return Err(Invalid("publication source/KIR event lost its exact ranked site"));
        }
    }
    let function = semantic_kir.module().function(&kernel.entry)
        .ok_or(Invalid("publication entry function missing"))?;
    let body = function.body.as_ref().ok_or(Invalid("publication entry body missing"))?;
    let payload_parameter_index = body.parameters.iter().position(|value| *value == source.payload_parameter)
        .and_then(|index| u32::try_from(index).ok()).ok_or(Invalid("payload is not an original KIR parameter"))?;
    static_publication_geometry::prove_geometry(
        kernel, function, source.events[0].kir_location, source.events[4].kir_location,
        source.producer_index, source.consumer_index,
    ).map_err(Invalid)?;
    Ok(Some(ProductionStaticPublicationDischargeV1 {
        source, ranked, payload_parameter_index, conflicts: Box::new([]),
    }))
}

fn discharge_static_publication_conflicts_v1(
    publication: &mut ProductionStaticPublicationDischargeV1,
    obligations: &FormalMemoryObligations,
) -> Result<(), ProductionFormalMemoryErrorV1> {
    let write = publication.source.events[0].kir_location;
    let read = publication.source.events[4].kir_location;
    let mut discharged = Vec::new();
    let mut unresolved = Vec::new();
    for conflict in obligations.inter_invocation_conflicts() {
        let reason = if conflict.allocation().parameter_index() != publication.payload_parameter_index {
            None
        } else if conflict.left() == write && conflict.right() == write {
            Some(ProductionStaticConflictReasonV1::ProducerInjectivity)
        } else if (conflict.left() == write && conflict.right() == read)
            || (conflict.left() == read && conflict.right() == write)
        {
            Some(ProductionStaticConflictReasonV1::PublishedReadHappensBefore)
        } else { None };
        if let Some(reason) = reason {
            discharged.push(ProductionStaticConflictDischargeV1 { conflict: *conflict, reason });
        } else {
            unresolved.push(*conflict);
        }
    }
    if !unresolved.is_empty() {
        return Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts {
            conflicts: unresolved.into_boxed_slice(),
        });
    }
    publication.conflicts = discharged.into_boxed_slice();
    Ok(())
}
