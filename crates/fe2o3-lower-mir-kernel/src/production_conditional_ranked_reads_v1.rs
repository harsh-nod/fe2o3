// Reads are admitted only through their exact canonical/source occurrences.
// Revisit fixed-size canonical facts to avoid retaining a second correspondence map.
fn matched_read_role_v1(
    binding: Option<&ProductionConditionalOutputBindingV1<'_>>,
    candidate: NativeRankedSourceCandidateV1<'_>,
    write: &RankedWrite,
    view: Option<ProductionRankedValueV1>,
    site: Option<(u32, u32)>,
    budget: &mut Budget<'_>,
) -> JoinResult<bool> {
    let Some(binding) = binding else {
        return Ok(false);
    };
    let mut matched = false;
    for ordinal in 0..binding.coverage().read_count() {
        let mut current = 0usize;
        let mut selected = None;
        binding
            .coverage()
            .visit_reads_v1(budget, |read| {
                if current == ordinal {
                    selected = Some(read);
                }
                current += 1;
                Ok(())
            })
            .map_err(|error| match error {
                fe2o3_kernel_ir::ConditionalTotalViewErrorV1::Resource(error) => {
                    JoinError::Resource(error)
                }
                _ => JoinError::SourceAccess,
            })?;
        let read = selected.ok_or(JoinError::SourceAccess)?;
        let (source, read_view) =
            checked_read_source_v1(binding, candidate, read, write.index, budget)?;
        if read_view == write.view {
            return Err(JoinError::View);
        }
        matched |= view == Some(read_view)
            || site == Some((source.ranked_block(), source.ranked_operation()));
    }
    Ok(matched)
}

pub(super) fn checked_read_source_v1<'a>(
    binding: &ProductionConditionalOutputBindingV1<'_>,
    candidate: NativeRankedSourceCandidateV1<'a>,
    read: fe2o3_kernel_ir::ConditionalTotalViewReadV1,
    index: ProductionRankedValueV1,
    budget: &mut Budget<'_>,
) -> JoinResult<(&'a ProductionRankedAccessSourceV1, ProductionRankedValueV1)> {
    let site = canonical_store_source(
        binding.coverage().function(),
        &binding.owner().correspondence,
        binding.association(),
        read.location(),
        AccessKindAttr::Read,
        budget,
    )?;
    let source = ranked_source(candidate.access_sources(), site, budget)?;
    let argument = binding
        .owner()
        .with_checked_arguments_v1(
            binding.association().correspondence_owner(),
            binding.association().semantic_function(),
            budget,
            |arguments| {
                conditional_output_argument_v1(arguments, read.parameter() as usize, read.slice())
            },
        )
        .map_err(|error| match error {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => {
                JoinError::Resource(error)
            }
            _ => JoinError::SourceAccess,
        })?
        .ok_or(JoinError::SourceAccess)?;
    let view = check_ranked_read_v1(candidate, source, read, index, argument.source, budget)?;
    Ok((source, view))
}

pub(super) fn checked_read_bound_v1(
    binding: &ProductionConditionalOutputBindingV1<'_>,
    candidate: NativeRankedSourceCandidateV1<'_>,
    read: fe2o3_kernel_ir::ConditionalTotalViewReadV1,
    index: ProductionRankedValueV1,
    budget: &mut Budget<'_>,
) -> JoinResult<fe2o3_pliron::ProductionConditionalRankedReadBoundV1> {
    let (source, view) = checked_read_source_v1(binding, candidate, read, index, budget)?;
    let mut extent = None;
    for block in candidate.kernel().blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(4)?;
            match operation {
                ProductionRankedOperationV1::View {
                    result,
                    dynamic_extents,
                    ..
                }
                | ProductionRankedOperationV1::ViewInSpace {
                    result,
                    dynamic_extents,
                    ..
                } if ProductionRankedValueV1::Local(*result) == view => {
                    let [value] = dynamic_extents.as_slice() else {
                        return Err(JoinError::ExtentSource);
                    };
                    if extent.replace(*value).is_some() {
                        return Err(JoinError::AmbiguousView);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(fe2o3_pliron::ProductionConditionalRankedReadBoundV1 {
        block: source.ranked_block(),
        operation: source.ranked_operation(),
        view,
        index,
        extent: extent.ok_or(JoinError::ExtentSource)?,
        domain: read.access_domain(),
    })
}

fn check_ranked_read_v1(
    candidate: NativeRankedSourceCandidateV1<'_>,
    source: &ProductionRankedAccessSourceV1,
    read: fe2o3_kernel_ir::ConditionalTotalViewReadV1,
    index: ProductionRankedValueV1,
    source_argument: u32,
    budget: &mut Budget<'_>,
) -> JoinResult<ProductionRankedValueV1> {
    budget.charge_work(8)?;
    let Some(ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Read,
        view,
        indices,
    }) = candidate
        .kernel()
        .blocks()
        .get(source.ranked_block() as usize)
        .and_then(|block| block.operations().get(source.ranked_operation() as usize))
    else {
        return Err(JoinError::SourceAccess);
    };
    if indices.as_slice() != [index] || source.output_extent().is_some() {
        return Err(JoinError::ExtentSource);
    }
    let mut found = false;
    for block in candidate.kernel().blocks() {
        budget.charge_work(1)?;
        for operation in block.operations() {
            budget.charge_work(12)?;
            match operation {
                ProductionRankedOperationV1::View {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    ..
                }
                | ProductionRankedOperationV1::ViewInSpace {
                    result,
                    element_width,
                    writable,
                    shape,
                    dynamic_extents,
                    allocation_origin,
                    ..
                } if ProductionRankedValueV1::Local(*result) == *view => {
                    if found
                        || *writable
                        || *allocation_origin != u64::from(source_argument) + 1
                        || u64::from(*element_width)
                            != read
                                .element_bytes()
                                .checked_mul(8)
                                .ok_or(ResourceError::Arithmetic)?
                        || shape.as_slice() != [DYNAMIC_EXTENT]
                        || !matches!(
                            dynamic_extents.as_slice(),
                            [ProductionRankedValueV1::Argument(_)]
                        )
                        || matches!(operation, ProductionRankedOperationV1::ViewInSpace { memory_space, .. } if *memory_space != MemorySpaceAttr::Global)
                    {
                        return Err(JoinError::View);
                    }
                    found = true;
                }
                _ => {}
            }
        }
    }
    if !found {
        return Err(JoinError::View);
    }
    Ok(*view)
}
