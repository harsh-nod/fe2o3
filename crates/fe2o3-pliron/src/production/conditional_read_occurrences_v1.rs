//! Inert recipe-to-live read occurrences captured during ranked materialization.
//! Consumers must authenticate the owner/epoch and replay these identities before
//! using them. This roster is neither a bounds result nor conditional authority.
use super::*;
use crate::production_analysis::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1 as Limit,
    ProductionAnalysisResourcePhaseV1 as Phase, ProductionAnalysisResourceUpperBoundV1 as Bound,
};
use dialect_kernel::{
    AccessKindAttr, DYNAMIC_EXTENT, MAX_RANKED_MEMORY_RANK, RankedAccessOp, RankedViewOp,
};
use pliron::{
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    value::Value,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReadExtentV1 {
    Static(u64),
    Dynamic(Value),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReadCoordinateOccurrenceV1 {
    pub(super) recipe_index: ProductionRankedValueV1,
    pub(super) index: Value,
    pub(super) extent: ReadExtentV1,
}

#[derive(Debug)]
pub(super) struct ReadOccurrenceV1 {
    pub(super) block: u32,
    /// Recipe operation position, never a live operation ordinal.
    pub(super) recipe_operation: u32,
    pub(super) recipe_view: ProductionRankedValueV1,
    /// Exact operation returned by materialize_operation for this recipe.
    pub(super) operation: Ptr<Operation>,
    pub(super) view: Value,
    coordinates: [ReadCoordinateOccurrenceV1; MAX_RANKED_MEMORY_RANK],
    rank: usize,
}

impl ReadOccurrenceV1 {
    pub(super) fn coordinates(&self) -> &[ReadCoordinateOccurrenceV1] {
        &self.coordinates[..self.rank]
    }
}

fn resource(error: Limit) -> ProductionSessionErrorV1 {
    ProductionSessionErrorV1::AnalysisResourceLimit {
        phase: error.phase,
        producing_pass: None,
        resource: error.resource,
    }
}

fn capacity() -> ProductionSessionErrorV1 {
    resource(Limit {
        phase: Phase::MemoryBounds,
        resource: "conditional read occurrence capacity",
    })
}

fn malformed() -> ProductionSessionErrorV1 {
    ProductionSessionErrorV1::RankedRecipe(ProductionRankedKernelErrorV1::Materialization(
        "incomplete or mismatched emitted ranked read occurrence",
    ))
}

impl ProductionPlironSessionV1 {
    fn reserve_read_occurrences_v1(
        &mut self,
        work: usize,
        retained: usize,
    ) -> Result<(), ProductionSessionErrorV1> {
        let additional =
            Bound::checked_phase(Phase::MemoryBounds, work, retained, 0).map_err(resource)?;
        // Both occurrence rosters consume the same inherited session account.
        let cumulative = self
            .ownership_binding_resources
            .checked_then_retain(additional, Phase::MemoryBounds)
            .map_err(resource)?;
        self.analysis_resource_limits()
            .require(Phase::MemoryBounds, cumulative)
            .map_err(resource)?;
        self.ownership_binding_resources = cumulative;
        Ok(())
    }

    pub(super) fn prepare_read_occurrence_storage_v1(
        &mut self,
        kernel: &ProductionRankedKernelV1,
        tree_work: usize,
    ) -> Result<Vec<ReadOccurrenceV1>, ProductionSessionErrorV1> {
        // The validated tree bound covers the census and emission-time matching.
        self.reserve_read_occurrences_v1(tree_work, 0)?;
        let count = kernel
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .filter(|recipe| {
                matches!(
                    recipe,
                    ProductionRankedOperationV1::Access {
                        kind: AccessKindAttr::Read,
                        ..
                    }
                )
            })
            .count();
        // Dynamic extent lookup scans at most rank preceding dimensions. Fixed
        // arrays avoid any per-read or per-coordinate heap allocation.
        let per_read = MAX_RANKED_MEMORY_RANK
            .checked_mul(MAX_RANKED_MEMORY_RANK + 64)
            .and_then(|work| work.checked_add(64))
            .ok_or_else(capacity)?;
        let work = count.checked_mul(per_read).ok_or_else(capacity)?;
        let storage = count
            .checked_mul(std::mem::size_of::<ReadOccurrenceV1>())
            .ok_or_else(capacity)?;
        self.reserve_read_occurrences_v1(work, storage)?;
        let mut occurrences = Vec::new();
        occurrences
            .try_reserve_exact(count)
            .map_err(|_| capacity())?;
        if occurrences.capacity() != count {
            return Err(capacity());
        }
        Ok(occurrences)
    }
}

pub(super) fn record_read_occurrence_v1(
    occurrences: &mut Vec<ReadOccurrenceV1>,
    context: &Context,
    block: usize,
    recipe_operation: usize,
    recipe: &ProductionRankedOperationV1,
    emitted: Ptr<Operation>,
) -> Result<(), ProductionSessionErrorV1> {
    if !matches!(
        recipe,
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            ..
        }
    ) {
        return Ok(());
    }
    if occurrences.len() == occurrences.capacity() {
        return Err(capacity());
    }
    occurrences.push(read_occurrence_v1(
        context,
        block,
        recipe_operation,
        recipe,
        emitted,
    )?);
    Ok(())
}

fn read_occurrence_v1(
    context: &Context,
    block: usize,
    recipe_operation: usize,
    recipe: &ProductionRankedOperationV1,
    emitted: Ptr<Operation>,
) -> Result<ReadOccurrenceV1, ProductionSessionErrorV1> {
    let ProductionRankedOperationV1::Access {
        kind: AccessKindAttr::Read,
        view: recipe_view,
        indices,
    } = recipe
    else {
        return Err(malformed());
    };
    if !Operation::is_op::<RankedAccessOp>(emitted, context) {
        return Err(malformed());
    }
    let access = RankedAccessOp::from_operation(emitted);
    let rank = indices.len();
    if rank == 0
        || rank > MAX_RANKED_MEMORY_RANK
        || access.kind(context) != Some(AccessKindAttr::Read)
        || access.atomic_ordering(context).is_some()
        || access.atomic_scope(context).is_some()
        || emitted.deref(context).get_num_operands() != rank + 1
    {
        return Err(malformed());
    }
    let view = access.view(context);
    let definition = view.defining_op().ok_or_else(malformed)?;
    if !Operation::is_op::<RankedViewOp>(definition, context) {
        return Err(malformed());
    }
    let view_op = RankedViewOp::from_operation(definition);
    let view_type = view_op.view_type(context).ok_or_else(malformed)?;
    let view_type = view_type.deref(context);
    if view_op.result(context) != view
        || view_type.shape().len() != rank
        || view_op.get_operation().deref(context).get_num_operands()
            != view_type.dynamic_extent_count()
    {
        return Err(malformed());
    }
    let coordinate = |axis: usize| -> Result<_, ProductionSessionErrorV1> {
        let extent = match view_type.shape()[axis] {
            DYNAMIC_EXTENT => ReadExtentV1::Dynamic(
                view_op
                    .dynamic_extent(context, axis)
                    .ok_or_else(malformed)?,
            ),
            extent => ReadExtentV1::Static(extent),
        };
        Ok(ReadCoordinateOccurrenceV1 {
            recipe_index: indices[axis],
            index: emitted.deref(context).get_operand(axis + 1),
            extent,
        })
    };
    let mut coordinates = [coordinate(0)?; MAX_RANKED_MEMORY_RANK];
    for (axis, row) in coordinates[..rank].iter_mut().enumerate().skip(1) {
        *row = coordinate(axis)?;
    }
    Ok(ReadOccurrenceV1 {
        block: u32::try_from(block).map_err(|_| capacity())?,
        recipe_operation: u32::try_from(recipe_operation).map_err(|_| capacity())?,
        recipe_view: *recipe_view,
        operation: emitted,
        view,
        coordinates,
        rank,
    })
}

pub(super) fn require_complete_v1(
    occurrences: &Vec<ReadOccurrenceV1>,
) -> Result<(), ProductionSessionErrorV1> {
    if occurrences.len() != occurrences.capacity() {
        return Err(malformed());
    }
    Ok(())
}

/// Admission precedes either traversal. The retained census bounds both recipe
/// and expanded live operations; replay checks those limits while streaming.
pub(super) fn replay_bound_v1(census: ProductionAnalysisInputCensusV1) -> Result<Bound, Limit> {
    let phase = Phase::PipelineVerification;
    let failure = || Limit {
        phase,
        resource: "conditional read occurrence replay work",
    };
    let per_operation = MAX_RANKED_MEMORY_RANK
        .checked_mul(MAX_RANKED_MEMORY_RANK + 64)
        .and_then(|n| n.checked_add(128))
        .ok_or_else(failure)?;
    let work = census
        .operations
        .checked_add(census.blocks)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_mul(per_operation))
        .ok_or_else(failure)?;
    // Only iterators and one fixed-size row are temporary; no heap storage.
    Bound::checked_phase(phase, work, 0, 0)
}

/// Inert replay only. The caller must admit replay_bound_v1 on its inherited
/// production and canonical accounts before calling, and authenticate custody.
pub(super) fn replay_read_occurrences_v1(
    context: &Context,
    function: &FuncOp,
    recipe: &ProductionRankedKernelV1,
    occurrences: &[ReadOccurrenceV1],
    epoch: u64,
    census: ProductionAnalysisInputCensusV1,
) -> Result<(), ProductionSessionErrorV1> {
    let changed = || ProductionSessionErrorV1::RankedGraphChanged;
    let require_epoch = || {
        if context
            .ir_mutation_attempt_epoch()
            .map_err(|_| changed())?
            .value()
            != epoch
        {
            return Err(changed());
        }
        Ok(())
    };
    require_epoch()?;
    let result =
        (|| {
            let function_ptr = function.get_operation();
            let function_op = function_ptr.try_deref(context).map_err(|_| changed())?;
            if !Operation::is_op::<FuncOp>(function_ptr, context)
                || function_op.num_regions() != 1
                || occurrences.len() > census.operations
                || recipe.blocks().len() > census.blocks
            {
                return Err(changed());
            }
            let region = function.get_region(context);
            let region = region.try_deref(context).map_err(|_| changed())?;
            let mut live_blocks = region.iter(context);
            let mut rows = occurrences.iter();
            let mut recipe_left = census.operations;
            let mut live_left = census.operations;
            let step = |left: &mut usize| -> Result<(), ProductionSessionErrorV1> {
                *left = left.checked_sub(1).ok_or_else(changed)?;
                Ok(())
            };
            for (block_index, block_recipe) in recipe.blocks().iter().enumerate() {
                let block = live_blocks.next().ok_or_else(changed)?;
                let block = block.try_deref(context).map_err(|_| changed())?;
                let mut recipes = block_recipe.operations().iter().enumerate();
                for operation in block.iter(context) {
                    step(&mut live_left)?;
                    let live = operation.try_deref(context).map_err(|_| changed())?;
                    if live.num_regions() != 0 {
                        return Err(changed());
                    }
                    if !Operation::is_op::<RankedAccessOp>(operation, context)
                        || RankedAccessOp::from_operation(operation).kind(context)
                            != Some(AccessKindAttr::Read)
                    {
                        continue;
                    }
                    let (recipe_operation, read) = loop {
                        let (index, candidate) = recipes.next().ok_or_else(changed)?;
                        step(&mut recipe_left)?;
                        if matches!(
                            candidate,
                            ProductionRankedOperationV1::Access {
                                kind: AccessKindAttr::Read,
                                ..
                            }
                        ) {
                            break (index, candidate);
                        }
                    };
                    let row = rows.next().ok_or_else(changed)?;
                    // Compare only against pointers reached in this live body, never
                    // dereference a captured pointer or treat recipe positions as live.
                    if row.operation != operation {
                        return Err(changed());
                    }
                    let observed =
                        read_occurrence_v1(context, block_index, recipe_operation, read, operation)
                            .map_err(|_| changed())?;
                    if row.block != observed.block
                        || row.recipe_operation != observed.recipe_operation
                        || row.recipe_view != observed.recipe_view
                        || row.view != observed.view
                        || row.rank != observed.rank
                        || row.coordinates().iter().zip(observed.coordinates()).any(
                            |(left, right)| {
                                left.recipe_index != right.recipe_index
                                    || left.index != right.index
                                    || left.extent != right.extent
                            },
                        )
                    {
                        return Err(changed());
                    }
                }
                for (_, remaining) in recipes {
                    step(&mut recipe_left)?;
                    if matches!(
                        remaining,
                        ProductionRankedOperationV1::Access {
                            kind: AccessKindAttr::Read,
                            ..
                        }
                    ) {
                        return Err(changed());
                    }
                }
            }
            if live_blocks.next().is_some() || rows.next().is_some() {
                return Err(changed());
            }
            Ok(())
        })();
    require_epoch()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::production_analysis::{
        LivePlironStructuralIdentityProviderV1, PlironStructuralIdentityProviderV1,
        ProductionAnalysisResourceContractV1,
    };

    fn recipe() -> ProductionRankedKernelV1 {
        use ProductionRankedOperationV1 as O;
        use ProductionRankedTerminatorV1 as T;
        let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
        let indices = [2, 3, 4].map(ProductionRankedValueV1::Argument).to_vec();
        ProductionRankedKernelV1::new(
            "read_occurrences",
            5,
            vec![
                ProductionRankedBlockV1::new(
                    vec![
                        O::View {
                            result: ProductionRankedValueIdV1::new(0),
                            element_width: 32,
                            writable: true,
                            shape: vec![4, DYNAMIC_EXTENT, DYNAMIC_EXTENT],
                            dynamic_extents: vec![
                                ProductionRankedValueV1::Argument(0),
                                ProductionRankedValueV1::Argument(1),
                            ],
                            allocation_origin: 1,
                            noalias_class: 1,
                        },
                        O::SemanticExpression {
                            result: ProductionRankedValueIdV1::new(1),
                            expression: ProductionSemanticExpressionV2::Constant {
                                scalar: ProductionSemanticScalarTypeV2::Bool,
                                bits: 1,
                            },
                            numerical_contract: ProductionNumericalContractV2::exact_for(
                                ProductionSemanticScalarTypeV2::Bool,
                            ),
                        },
                        O::Access {
                            kind: AccessKindAttr::Read,
                            view,
                            indices: indices.clone(),
                        },
                    ],
                    T::Branch { target: 1 },
                ),
                ProductionRankedBlockV1::new(
                    vec![
                        O::Access {
                            kind: AccessKindAttr::Read,
                            view,
                            indices: indices.clone(),
                        },
                        O::Access {
                            kind: AccessKindAttr::Write,
                            view,
                            indices,
                        },
                    ],
                    T::Return,
                ),
            ],
        )
        .unwrap()
    }

    fn session() -> ProductionPlironSessionV1 {
        ProductionPlironSessionV1::new_ranked_v1(ProductionSessionLimitsV1::default()).unwrap()
    }

    fn construct(session: &mut ProductionPlironSessionV1, name: &str) -> StageIdentityV1 {
        let registered = session
            .register_construction(ProductionConstructionV1::ranked_kernel(name, recipe()).unwrap())
            .unwrap();
        session.construct_registered(registered).unwrap().0.identity
    }

    fn replay_input(
        session: &ProductionPlironSessionV1,
        stage: StageIdentityV1,
    ) -> (ProductionAnalysisInputCensusV1, u64) {
        let context = &session.inner.context;
        let function =
            FuncOp::from_operation(session.constructed_roots[&stage].ranked_function.unwrap());
        let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let capture = provider
            .capture_with_resource_limits_v1(session.analysis_resource_limits())
            .ok()
            .unwrap();
        (
            capture.input_census,
            context.ir_mutation_attempt_epoch().unwrap().value(),
        )
    }

    fn replay(
        session: &ProductionPlironSessionV1,
        stage: StageIdentityV1,
        census: ProductionAnalysisInputCensusV1,
        epoch: u64,
    ) -> Result<(), ProductionSessionErrorV1> {
        let mut resources =
            ProductionAnalysisResourceContractV1::new(session.analysis_resource_limits());
        resources
            .admit_retained(
                Phase::PipelineVerification,
                session.ownership_binding_resources,
            )
            .unwrap();
        resources
            .admit_retained(
                Phase::PipelineVerification,
                replay_bound_v1(census).unwrap(),
            )
            .unwrap();
        let record = &session.constructed_roots[&stage];
        replay_read_occurrences_v1(
            &session.inner.context,
            &FuncOp::from_operation(record.ranked_function.unwrap()),
            record.ranked_kernel.as_ref().unwrap(),
            &record.read_occurrences,
            epoch,
            census,
        )
    }

    #[test]
    fn replay_accepts_expanded_operations_and_rejects_roster_corruption() {
        let cases = [
            "missing",
            "duplicate",
            "reordered",
            "block",
            "recipe operation",
            "recipe view",
            "recipe index",
            "live view",
            "live index",
            "static extent",
            "dynamic extent",
            "extent kind",
            "rank",
            "operation",
        ];
        for (case, name) in cases.iter().enumerate() {
            let mut session = session();
            let stage = construct(&mut session, "roster_replay");
            let (census, epoch) = replay_input(&session, stage);
            replay(&session, stage, census, epoch).unwrap();
            let record = session.constructed_roots.get_mut(&stage).unwrap();
            let first = &record.read_occurrences[0];
            let duplicate = read_occurrence_v1(
                &session.inner.context,
                first.block as usize,
                first.recipe_operation as usize,
                &record.ranked_kernel.as_ref().unwrap().blocks()[first.block as usize].operations()
                    [first.recipe_operation as usize],
                first.operation,
            )
            .unwrap();
            let rows = &mut record.read_occurrences;
            let index = rows[0].coordinates[0].index;
            match case {
                0 => {
                    rows.pop();
                }
                1 => rows.push(duplicate),
                2 => rows.swap(0, 1),
                3 => rows[0].block += 1,
                4 => rows[0].recipe_operation += 1,
                5 => rows[0].recipe_view = ProductionRankedValueV1::Argument(0),
                6 => rows[0].coordinates[0].recipe_index = ProductionRankedValueV1::Argument(0),
                7 => rows[0].view = index,
                8 => rows[0].coordinates[0].index = rows[0].view,
                9 => rows[0].coordinates[0].extent = ReadExtentV1::Static(5),
                10 => rows[0].coordinates[1].extent = ReadExtentV1::Dynamic(index),
                11 => rows[0].coordinates[1].extent = ReadExtentV1::Static(4),
                12 => rows[0].rank = MAX_RANKED_MEMORY_RANK + 1,
                13 => rows[0].operation = rows[1].operation,
                _ => unreachable!(),
            }
            assert!(
                matches!(
                    replay(&session, stage, census, epoch),
                    Err(ProductionSessionErrorV1::RankedGraphChanged)
                ),
                "{name}"
            );
            assert_eq!(
                session
                    .inner
                    .context
                    .ir_mutation_attempt_epoch()
                    .unwrap()
                    .value(),
                epoch
            );
        }
    }

    #[test]
    fn replay_rejects_foreign_and_other_function_pointers_without_dereferencing_them() {
        let mut foreign = session();
        let foreign_stage = construct(&mut foreign, "foreign");
        let foreign_pointer =
            foreign.constructed_roots[&foreign_stage].read_occurrences[0].operation;
        let mut session = session();
        let stage = construct(&mut session, "original");
        let other_stage = construct(&mut session, "other_function");
        let other_pointer = session.constructed_roots[&other_stage].read_occurrences[0].operation;
        let (census, epoch) = replay_input(&session, stage);
        replay(&session, stage, census, epoch).unwrap();
        for pointer in [foreign_pointer, other_pointer] {
            session
                .constructed_roots
                .get_mut(&stage)
                .unwrap()
                .read_occurrences[0]
                .operation = pointer;
            assert!(matches!(
                replay(&session, stage, census, epoch),
                Err(ProductionSessionErrorV1::RankedGraphChanged)
            ));
        }
    }

    #[test]
    fn replay_rechecks_live_view_index_and_extent_even_with_current_epoch() {
        for case in 0..3 {
            let mut session = session();
            let stage = construct(&mut session, "mutated_live_read");
            let (census, epoch) = replay_input(&session, stage);
            let row = &session.constructed_roots[&stage].read_occurrences[0];
            let context = &session.inner.context;
            let (operation, operand, replacement) = match case {
                0 => (row.operation, 0, row.coordinates[0].index),
                1 => (row.operation, 1, row.coordinates[1].index),
                _ => (row.view.defining_op().unwrap(), 0, row.coordinates[0].index),
            };
            let original = operation.deref(context).get_operand(operand);
            Operation::replace_operand(operation, context, operand, replacement);
            assert!(matches!(
                replay(&session, stage, census, epoch),
                Err(ProductionSessionErrorV1::RankedGraphChanged)
            ));
            let changed_epoch = context.ir_mutation_attempt_epoch().unwrap().value();
            assert!(matches!(
                replay(&session, stage, census, changed_epoch),
                Err(ProductionSessionErrorV1::RankedGraphChanged)
            ));
            Operation::replace_operand(operation, context, operand, original);
            assert!(matches!(
                replay(&session, stage, census, epoch),
                Err(ProductionSessionErrorV1::RankedGraphChanged)
            ));
            replay(
                &session,
                stage,
                census,
                context.ir_mutation_attempt_epoch().unwrap().value(),
            )
            .unwrap();
        }
    }

    #[test]
    fn replay_census_caps_scans_and_bound_is_checked_without_new_storage() {
        let mut session = session();
        let stage = construct(&mut session, "bounded_replay");
        let (census, epoch) = replay_input(&session, stage);
        let bound = replay_bound_v1(census).unwrap();
        assert!(bound.work_upper_bound() > census.operations);
        assert_eq!(bound.retained_storage_upper_bound(), 0);
        assert_eq!(bound.peak_storage_upper_bound(), 0);
        for too_small in [
            ProductionAnalysisInputCensusV1 {
                blocks: 0,
                ..census
            },
            ProductionAnalysisInputCensusV1 {
                operations: 2,
                ..census
            },
        ] {
            assert!(matches!(
                replay(&session, stage, too_small, epoch),
                Err(ProductionSessionErrorV1::RankedGraphChanged)
            ));
        }
        assert!(
            replay_bound_v1(ProductionAnalysisInputCensusV1 {
                operations: usize::MAX,
                ..census
            })
            .is_err()
        );
    }

    #[test]
    fn expanded_operation_preserves_exact_read_pointer_and_live_coordinates() {
        let mut session = session();
        let stage = construct(&mut session, "expanded_reads");
        let record = &session.constructed_roots[&stage];
        let context = &session.inner.context;
        let function = FuncOp::from_operation(record.ranked_function.unwrap());
        let blocks = function
            .get_region(context)
            .deref(context)
            .iter(context)
            .collect::<Vec<_>>();
        let arguments = blocks[0].deref(context).arguments().collect::<Vec<_>>();
        let mut live_reads = Vec::new();
        for (block, pointer) in blocks.iter().enumerate() {
            for (ordinal, operation) in pointer.deref(context).iter(context).enumerate() {
                if Operation::is_op::<RankedAccessOp>(operation, context)
                    && RankedAccessOp::from_operation(operation).kind(context)
                        == Some(AccessKindAttr::Read)
                {
                    live_reads.push((block, ordinal, operation));
                }
            }
        }
        assert_eq!(record.read_occurrences.len(), 2);
        assert_eq!(record.read_occurrences.len(), live_reads.len());
        require_complete_v1(&record.read_occurrences).unwrap();
        let kernel = record.ranked_kernel.as_ref().unwrap();
        for (row, &(block, _, pointer)) in record.read_occurrences.iter().zip(&live_reads) {
            assert_eq!(row.block as usize, block);
            assert_eq!(row.operation, pointer);
            let ProductionRankedOperationV1::Access {
                kind,
                view,
                indices,
            } = &kernel.blocks()[block].operations()[row.recipe_operation as usize]
            else {
                panic!("roster must identify an exact recipe read")
            };
            assert_eq!(*kind, AccessKindAttr::Read);
            assert_eq!(*view, row.recipe_view);
            let access = RankedAccessOp::from_operation(pointer);
            assert_eq!(row.view, access.view(context));
            assert_eq!(row.coordinates().len(), 3);
            for (axis, coordinate) in row.coordinates().iter().enumerate() {
                assert_eq!(coordinate.recipe_index, indices[axis]);
                assert_eq!(coordinate.index, arguments[axis + 2]);
                assert_eq!(
                    coordinate.index,
                    pointer.deref(context).get_operand(axis + 1)
                );
                assert_eq!(
                    coordinate.extent,
                    match axis {
                        0 => ReadExtentV1::Static(4),
                        _ => ReadExtentV1::Dynamic(arguments[axis - 1]),
                    }
                );
            }
        }
        // The SemanticExpression emits its expression plus a typed root marker.
        let first = &record.read_occurrences[0];
        assert!(live_reads[0].1 > first.recipe_operation as usize);
        let wrong = blocks[0]
            .deref(context)
            .iter(context)
            .nth(first.recipe_operation as usize)
            .unwrap();
        assert_ne!(wrong, first.operation);
        assert_ne!(
            record.read_occurrences[0].operation,
            record.read_occurrences[1].operation
        );
        assert_eq!(
            record.read_occurrences[0].view,
            record.read_occurrences[1].view
        );
        assert_eq!(
            record.read_occurrences[0].coordinates(),
            record.read_occurrences[1].coordinates()
        );
    }

    #[test]
    fn occurrence_capacity_never_grows_and_missing_rows_are_rejected() {
        let mut session = session();
        let stage = construct(&mut session, "complete_reads");
        let record = &session.constructed_roots[&stage];
        let row = &record.read_occurrences[0];
        let recipe = &record.ranked_kernel.as_ref().unwrap().blocks()[row.block as usize]
            .operations()[row.recipe_operation as usize];
        let mut unadmitted = Vec::new();
        assert!(matches!(
            record_read_occurrence_v1(
                &mut unadmitted,
                &session.inner.context,
                row.block as usize,
                row.recipe_operation as usize,
                recipe,
                row.operation,
            ),
            Err(ProductionSessionErrorV1::AnalysisResourceLimit {
                phase: Phase::MemoryBounds,
                resource: "conditional read occurrence capacity",
                ..
            })
        ));
        assert_eq!(unadmitted.capacity(), 0);
        let rows = &mut session
            .constructed_roots
            .get_mut(&stage)
            .unwrap()
            .read_occurrences;
        let last = rows.pop().unwrap();
        assert!(require_complete_v1(rows).is_err());
        rows.push(last);
        require_complete_v1(rows).unwrap();
    }

    #[test]
    fn occurrence_admission_uses_inherited_ledger_before_graph_creation() {
        let mut calibration = session();
        construct(&mut calibration, "prior_reads");
        let prefix = calibration.ownership_binding_resources;
        construct(&mut calibration, "next_reads");
        let exact = calibration.ownership_binding_resources;
        assert!(exact.work_upper_bound() > prefix.work_upper_bound());
        assert!(exact.retained_storage_upper_bound() > prefix.retained_storage_upper_bound());
        for (work_short, storage_short) in [(0, 0), (1, 0), (0, 1)] {
            let mut session = session();
            construct(&mut session, "prior_reads");
            assert_eq!(session.ownership_binding_resources, prefix);
            let roots_before = session.inner.operations.len();
            let epoch = session
                .inner
                .context
                .ir_mutation_attempt_epoch()
                .unwrap()
                .value();
            session.analysis_resource_limits = ProductionAnalysisResourceLimitsV1::new(
                exact.work_upper_bound() - work_short,
                exact.peak_storage_upper_bound() - storage_short,
            );
            let registered = session
                .register_construction(
                    ProductionConstructionV1::ranked_kernel("next_reads", recipe()).unwrap(),
                )
                .unwrap();
            let result = session.construct_registered(registered);
            if work_short == 0 && storage_short == 0 {
                let (stage, _) = result.unwrap();
                assert_eq!(session.ownership_binding_resources, exact);
                assert_eq!(
                    session.constructed_roots[&stage.identity]
                        .read_occurrences
                        .len(),
                    2
                );
            } else {
                assert!(
                    matches!(
                        result,
                        Err(ProductionSessionErrorV1::AnalysisResourceLimit {
                            phase: Phase::MemoryBounds,
                            ..
                        })
                    ),
                    "{result:?}"
                );
                assert!(session.is_poisoned());
                assert_eq!(session.inner.operations.len(), roots_before);
                assert_eq!(
                    session
                        .inner
                        .context
                        .ir_mutation_attempt_epoch()
                        .unwrap()
                        .value(),
                    epoch
                );
                assert_eq!(
                    session
                        .ownership_binding_resources
                        .retained_storage_upper_bound(),
                    prefix.retained_storage_upper_bound()
                );
                assert!(
                    session.ownership_binding_resources.work_upper_bound()
                        >= prefix.work_upper_bound()
                );
            }
        }
    }

    #[test]
    fn generic_constructors_initialize_empty_read_rosters() {
        let mut session = session();
        let empty = ProductionRankedKernelV1::new(
            "empty",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![],
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        for construction in [
            ProductionConstructionV1::builtin_module("builtin").unwrap(),
            ProductionConstructionV1::ranked_kernel("ranked", empty).unwrap(),
        ] {
            let registered = session.register_construction(construction).unwrap();
            let (stage, _) = session.construct_registered(registered).unwrap();
            let rows = &session.constructed_roots[&stage.identity].read_occurrences;
            assert!(rows.is_empty());
            assert_eq!(rows.capacity(), 0);
        }
    }
}
