//! Bounded concrete CFG emission for the source-owned nominal block stream.
//! Produces actual ranked blocks and coordinate maps, not verification authority.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TensorSite {
    pub(super) source_block: usize,
    pub(super) ranked_block: usize,
    pub(super) ranked_operation: usize,
}
pub(super) struct Emitted {
    pub(super) blocks: Vec<ProductionRankedBlockV1>,
    pub(super) controls: checked_control::Controls,
    pub(super) sources: Vec<ProjectedAccessSourceV1>,
    pub(super) tensors: Vec<TensorSite>,
    operations: Vec<ProductionRankedOperationV1>,
    scratch: Option<ProductionRankedOperationV1>,
    pub(super) base: [Option<usize>; MAX_BLOCKS],
    pub(super) reachable: [bool; MAX_BLOCKS],
    operation_count: usize,
    pub(super) complete: bool,
}
impl Emitted {
    pub(super) const fn empty() -> Self {
        Self {
            blocks: Vec::new(),
            controls: checked_control::Controls::empty(),
            sources: Vec::new(),
            tensors: Vec::new(),
            operations: Vec::new(),
            scratch: None,
            base: [None; MAX_BLOCKS],
            reachable: [false; MAX_BLOCKS],
            operation_count: 0,
            complete: false,
        }
    }
}
fn plus(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn successors(
    terminator: &ProjectedCfgTerminatorV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
    mut visit: impl FnMut(usize) -> Result<()>,
) -> Result<()> {
    resources.work(16)?;
    match terminator {
        ProjectedCfgTerminatorV1::Branch(target) => visit(*target),
        ProjectedCfgTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
        } => {
            visit(*first_block)?;
            visit(*second_block)
        }
        ProjectedCfgTerminatorV1::AnalysisMultiSplit { blocks } => {
            for target in blocks {
                resources.work(8)?;
                visit(*target)?;
            }
            Ok(())
        }
        ProjectedCfgTerminatorV1::Return | ProjectedCfgTerminatorV1::Trap => Ok(()),
        ProjectedCfgTerminatorV1::AbsentMaterialized => Err(Error::Incomplete(
            "nominal emitted CFG reaches an absent materialized source block",
        )),
        ProjectedCfgTerminatorV1::Predicate { .. } | ProjectedCfgTerminatorV1::ExactSwitch(_) => {
            Err(Error::Incomplete(
                "nominal block emitter requires the original conservative CFG profile",
            ))
        }
    }
}
fn layout(
    rows: &[ProjectedSemanticBlockV1],
    terminators: &[ProjectedCfgTerminatorV1],
    entry: usize,
    emitted: &mut Emitted,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<usize> {
    resources.work(64)?;
    if rows.len() != terminators.len()
        || rows.is_empty()
        || rows.len() > MAX_BLOCKS
        || entry >= rows.len()
    {
        return Err(Error::Incomplete(
            "nominal emitted CFG source census differs",
        ));
    }
    // Fixed stack; each source block is marked before its sole push. Empty
    // and unknown switch alternatives remain explicit, never assumed uniform.
    let mut stack = [0usize; MAX_BLOCKS];
    let mut top = 1usize;
    stack[0] = entry;
    emitted.reachable[entry] = true;
    while top != 0 {
        resources.work(16)?;
        top -= 1;
        let source = stack[top];
        successors(&terminators[source], resources, |target| {
            if target >= rows.len() {
                return Err(Error::Incomplete("nominal CFG successor outside source"));
            }
            if !emitted.reachable[target] {
                emitted.reachable[target] = true;
                stack[top] = target;
                top += 1;
            }
            Ok(())
        })?;
    }
    let mut count = 1usize;
    for (index, (row, term)) in rows.iter().zip(terminators).enumerate() {
        resources.work(16)?;
        if !emitted.reachable[index] {
            continue;
        }
        emitted.base[index] = Some(count);
        count = plus(count, 1)?;
        for item in &row.items {
            resources.work(16)?;
            if let ProjectedBlockItemV1::Guarded(access) = item {
                if access.comparisons.is_empty() || access.checked_success.is_some() {
                    return Err(Error::Incomplete(
                        "nominal emitted guard leaves original identity-access profile",
                    ));
                }
                count = plus(count, plus(access.comparisons.len(), 2)?)?;
            }
        }
        if let ProjectedCfgTerminatorV1::AnalysisMultiSplit { blocks } = term {
            if blocks.len() < 3 {
                return Err(Error::Incomplete(
                    "nominal multi-split has fewer than three successors",
                ));
            }
            count = plus(count, blocks.len() - 2)?;
        }
    }
    if count > fe2o3_pliron::MAX_RANKED_BOUNDS_BLOCKS {
        return Err(Error::Unsupported(
            "semantic CFG projection exceeds the ranked block limit",
        ));
    }
    Ok(count)
}
fn target(emitted: &Emitted, source: usize) -> Result<u32> {
    let target = emitted
        .base
        .get(source)
        .copied()
        .flatten()
        .ok_or(Error::Incomplete(
            "nominal CFG edge targets a pruned source block",
        ))?;
    ranked_block_id(target)
}
fn reserve_operation(
    emitted: &mut Emitted,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(16)?;
    if emitted.operation_count >= MAX_RANKED_BOUNDS_OPERATIONS {
        return Err(Error::Unsupported(
            "nominal emitted CFG exceeds ranked operation limit",
        ));
    }
    resources.reserve(&mut emitted.operations, 1)
}
fn copy_operation(
    source: &ProductionRankedOperationV1,
    emitted: &mut Emitted,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    reserve_operation(emitted, resources)?;
    if emitted.scratch.is_some() {
        return Err(resource(Resource::Accounting));
    }
    let operation = match source {
        ProductionRankedOperationV1::ExecutionLayout {
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
            full_physical_workgroups,
        } => ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: *grid_identity,
            global_extents: *global_extents,
            workgroup_extents: *workgroup_extents,
            subgroup_size: *subgroup_size,
            full_physical_workgroups: *full_physical_workgroups,
        },
        ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension,
            launch_extent,
        } => ProductionRankedOperationV1::InvocationIndex {
            result: *result,
            dimension: *dimension,
            launch_extent: *launch_extent,
        },
        ProductionRankedOperationV1::IndexConstant { result, value } => {
            ProductionRankedOperationV1::IndexConstant {
                result: *result,
                value: *value,
            }
        }
        ProductionRankedOperationV1::ViewInSpace {
            result,
            element_width,
            writable,
            memory_space,
            allocation_origin,
            noalias_class,
            ..
        } => ProductionRankedOperationV1::ViewInSpace {
            result: *result,
            element_width: *element_width,
            writable: *writable,
            shape: Vec::new(),
            dynamic_extents: Vec::new(),
            memory_space: *memory_space,
            allocation_origin: *allocation_origin,
            noalias_class: *noalias_class,
        },
        ProductionRankedOperationV1::Access { kind, view, .. } => {
            ProductionRankedOperationV1::Access {
                kind: *kind,
                view: *view,
                indices: Vec::new(),
            }
        }
        ProductionRankedOperationV1::AllocationEffect {
            kind,
            memory_space,
            allocation_origin,
            noalias_class,
        } => ProductionRankedOperationV1::AllocationEffect {
            kind: *kind,
            memory_space: *memory_space,
            allocation_origin: *allocation_origin,
            noalias_class: *noalias_class,
        },
        ProductionRankedOperationV1::TensorLayout {
            contract,
            convergence,
            active_lanes,
            binding,
        } => ProductionRankedOperationV1::TensorLayout {
            contract: *contract,
            convergence: *convergence,
            active_lanes: *active_lanes,
            binding: *binding,
        },
        _ => {
            return Err(Error::Incomplete(
                "nominal emitted operation leaves the closed source stream",
            ));
        }
    };
    emitted.scratch = Some(operation);
    match (source, emitted.scratch.as_mut().expect("outer copy slot")) {
        (
            ProductionRankedOperationV1::ViewInSpace {
                shape: from_shape,
                dynamic_extents: from_extents,
                ..
            },
            ProductionRankedOperationV1::ViewInSpace {
                shape,
                dynamic_extents,
                ..
            },
        ) => {
            resources.work(from_shape.len())?;
            resources.reserve(shape, from_shape.len())?;
            shape.extend_from_slice(from_shape);
            resources.work(from_extents.len())?;
            resources.reserve(dynamic_extents, from_extents.len())?;
            dynamic_extents.extend_from_slice(from_extents);
        }
        (
            ProductionRankedOperationV1::Access { indices: from, .. },
            ProductionRankedOperationV1::Access { indices, .. },
        ) => {
            resources.work(from.len())?;
            resources.reserve(indices, from.len())?;
            indices.extend_from_slice(from);
        }
        _ => {}
    }
    emitted
        .operations
        .push(emitted.scratch.take().expect("copied original operation"));
    emitted.operation_count += 1;
    Ok(())
}
fn push_block(
    emitted: &mut Emitted,
    expected: usize,
    terminator: ProductionRankedTerminatorV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(32)?;
    if emitted.blocks.len() != expected || expected >= fe2o3_pliron::MAX_RANKED_BOUNDS_BLOCKS {
        return Err(Error::Incomplete("nominal emitted block cursor differs"));
    }
    resources.reserve(&mut emitted.blocks, 1)?;
    let operations = std::mem::take(&mut emitted.operations);
    emitted
        .blocks
        .push(ProductionRankedBlockV1::new(operations, terminator));
    Ok(())
}
fn source_row(
    emitted: &mut Emitted,
    block: usize,
    operation: usize,
    source: ProjectedEffectSourceV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.push(
        &mut emitted.sources,
        ProjectedAccessSourceV1 {
            block,
            operation,
            access: source.access,
            memory_space: source.memory_space,
            source: source.source,
            output_extent: source.output_extent,
            semantic_site: source.semantic_site,
        },
    )
}
pub(super) fn emit(
    rows: &[ProjectedSemanticBlockV1],
    entry_operations: &[ProductionRankedOperationV1],
    terminators: &[ProjectedCfgTerminatorV1],
    entry: usize,
    emitted: &mut Emitted,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(64)?;
    if !resources.is_metered()
        || resources.has_denial()
        || emitted.complete
        || !emitted.blocks.is_empty()
        || emitted.blocks.capacity() != 0
        || !emitted.sources.is_empty()
        || emitted.sources.capacity() != 0
        || !emitted.tensors.is_empty()
        || emitted.tensors.capacity() != 0
        || !emitted.operations.is_empty()
        || emitted.operations.capacity() != 0
        || emitted.scratch.is_some()
        || emitted.base.iter().any(Option::is_some)
        || emitted.reachable.iter().any(|v| *v)
        || emitted.operation_count != 0
    {
        return Err(resource(Resource::Accounting));
    }
    let expected = layout(rows, terminators, entry, emitted, resources)?;
    for operation in entry_operations {
        copy_operation(operation, emitted, resources)?;
    }
    let entry_target = target(emitted, entry)?;
    push_block(
        emitted,
        0,
        ProductionRankedTerminatorV1::Branch {
            target: entry_target,
        },
        resources,
    )?;
    for (semantic, (row, terminator)) in rows.iter().zip(terminators).enumerate() {
        resources.work(16)?;
        let Some(mut current) = emitted.base[semantic] else {
            continue;
        };
        for item in &row.items {
            resources.work(32)?;
            match item {
                ProjectedBlockItemV1::Effect { operation, source } => {
                    if let Some(source) = source {
                        let operation = emitted.operations.len();
                        source_row(emitted, current, operation, *source, resources)?;
                    }
                    if matches!(operation, ProductionRankedOperationV1::TensorLayout { .. }) {
                        resources.push(
                            &mut emitted.tensors,
                            TensorSite {
                                source_block: semantic,
                                ranked_block: current,
                                ranked_operation: emitted.operations.len(),
                            },
                        )?;
                    }
                    copy_operation(operation, emitted, resources)?;
                }
                ProjectedBlockItemV1::Guarded(access) => {
                    let access_block = plus(current, access.comparisons.len())?;
                    let failure = plus(access_block, 1)?;
                    let continuation = plus(failure, 1)?;
                    for (index, (lhs, rhs)) in access.comparisons.iter().enumerate() {
                        resources.work(32)?;
                        let next = plus(current, 1)?;
                        let true_block = if index + 1 == access.comparisons.len() {
                            access_block
                        } else {
                            next
                        };
                        push_block(
                            emitted,
                            current,
                            ProductionRankedTerminatorV1::IndexLessThan {
                                lhs: *lhs,
                                rhs: *rhs,
                                true_block: ranked_block_id(true_block)?,
                                false_block: ranked_block_id(failure)?,
                            },
                            resources,
                        )?;
                        current = next;
                    }
                    reserve_operation(emitted, resources)?;
                    emitted.scratch = Some(ProductionRankedOperationV1::Access {
                        kind: access.access,
                        view: ProductionRankedValueV1::Local(access.view),
                        indices: Vec::new(),
                    });
                    let Some(ProductionRankedOperationV1::Access { indices, .. }) =
                        emitted.scratch.as_mut()
                    else {
                        unreachable!()
                    };
                    resources.work(access.indices.len())?;
                    resources.reserve(indices, access.indices.len())?;
                    indices.extend_from_slice(&access.indices);
                    emitted.operations.push(
                        emitted
                            .scratch
                            .take()
                            .expect("retained original guarded access"),
                    );
                    emitted.operation_count += 1;
                    source_row(
                        emitted,
                        access_block,
                        0,
                        ProjectedEffectSourceV1 {
                            access: access.access,
                            memory_space: access.memory_space,
                            source: access.source,
                            output_extent: access.output_extent,
                            semantic_site: access.semantic_site,
                        },
                        resources,
                    )?;
                    push_block(
                        emitted,
                        access_block,
                        ProductionRankedTerminatorV1::Branch {
                            target: ranked_block_id(continuation)?,
                        },
                        resources,
                    )?;
                    push_block(
                        emitted,
                        failure,
                        ProductionRankedTerminatorV1::Trap,
                        resources,
                    )?;
                    current = continuation;
                }
                ProjectedBlockItemV1::Pipeline(_)
                | ProjectedBlockItemV1::GeneratedFromSemanticTerminator(_) => {
                    return Err(Error::Incomplete(
                        "nominal emitted CFG has unconnected later effects",
                    ));
                }
            }
        }
        match terminator {
            ProjectedCfgTerminatorV1::Branch(source) => {
                let destination = target(emitted, *source)?;
                push_block(
                    emitted,
                    current,
                    ProductionRankedTerminatorV1::Branch {
                        target: destination,
                    },
                    resources,
                )?;
            }
            ProjectedCfgTerminatorV1::AnalysisSplit {
                first_block,
                second_block,
            } => {
                let first = target(emitted, *first_block)?;
                let second = target(emitted, *second_block)?;
                if let Some(site) = emitted.controls.site(semantic) {
                    resources.work(64)?;
                    let term = checked_control::expected_terminator(site, &emitted.base)?;
                    emitted
                        .controls
                        .record(semantic, ranked_block_id(current)?)?;
                    push_block(emitted, current, term, resources)?;
                    continue;
                }
                push_block(
                    emitted,
                    current,
                    ProductionRankedTerminatorV1::AnalysisSplit {
                        control_dependencies: Vec::new(),
                        first_block: first,
                        second_block: second,
                    },
                    resources,
                )?;
            }
            ProjectedCfgTerminatorV1::AnalysisMultiSplit { blocks } => {
                // Exact original analysis_multi_split_v1 order: first target at
                // each node; last node's second edge is the final actual target.
                for index in 0..blocks.len() - 1 {
                    resources.work(32)?;
                    let block = plus(current, index)?;
                    let second = if index + 2 == blocks.len() {
                        target(emitted, blocks[index + 1])?
                    } else {
                        ranked_block_id(plus(block, 1)?)?
                    };
                    let first = target(emitted, blocks[index])?;
                    push_block(
                        emitted,
                        block,
                        ProductionRankedTerminatorV1::AnalysisSplit {
                            control_dependencies: Vec::new(),
                            first_block: first,
                            second_block: second,
                        },
                        resources,
                    )?;
                }
            }
            ProjectedCfgTerminatorV1::Return => push_block(
                emitted,
                current,
                ProductionRankedTerminatorV1::Return,
                resources,
            )?,
            ProjectedCfgTerminatorV1::Trap => push_block(
                emitted,
                current,
                ProductionRankedTerminatorV1::Trap,
                resources,
            )?,
            _ => {
                return Err(Error::Incomplete(
                    "nominal emitted CFG retained an unsupported source terminator",
                ));
            }
        }
    }
    resources.work(32)?;
    if resources.has_denial()
        || emitted.blocks.len() != expected
        || !emitted.operations.is_empty()
        || emitted.scratch.is_some()
        || emitted
            .operation_count
            .checked_add(expected)
            .is_none_or(|n| n > MAX_RANKED_BOUNDS_OPERATIONS)
    {
        return Err(resource(Resource::Accounting));
    }
    emitted.complete = true;
    Ok(())
}

#[cfg(test)]
mod arithmetic_tests {
    use super::*;
    #[test]
    fn block_expansion_arithmetic_never_wraps() {
        assert_eq!(plus(7, 9).unwrap(), 16);
        assert!(matches!(plus(usize::MAX,1),Err(Error::CanonicalAssertions(
            crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)))));
    }
}
