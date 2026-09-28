// Private unfinished source obligations. The original-source discriminator does
// not consult mutable emitter claims and cannot itself grant physical admission.
use super::*;

include!("production_source_allocation_input_v18.rs");
include!("production_optimized_source_allocation_v18.rs");
include!("production_optimized_source_typed_memory_v18.rs");
include!("production_optimized_source_currentness_v18.rs");
include!("production_optimized_source_private_memory_v18.rs");
include!("production_source_issued_role_replay_v18.rs");
#[cfg(test)]
#[path = "production_source_issued_roles_v29_tests.rs"]
mod issued_role_tests_v29;

pub(super) fn assemble_original_zero_raw_v29(
    instances: &ProductionCallInstancePlanV1<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    limits: ProductionSemanticKirLimitsV1,
    frame: Option<&scoped_slot_relocation_v29::FramePermitV29<'_, '_>>,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingScopedRootEmissionV29, ProductionSemanticKirErrorV1> {
    require_original_zero_raw_v29(instances, references, budget)?;
    assemble_pending_scoped_root_body_v29(instances, emitted, limits, frame, references, budget)
}

pub(super) fn assemble_pending_raw_scoped_root_v29(
    original: &SourceRootPreparationV29<'_, '_, '_>,
    instances: &ProductionCallInstancePlanV1<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    limits: ProductionSemanticKirLimitsV1,
    frame: Option<&scoped_slot_relocation_v29::FramePermitV29<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingRawAssemblyV29, ProductionSemanticKirErrorV1> {
    let references = original.check_raw(instances, budget)?;
    let pending = assemble_pending_scoped_root_body_v29(
        instances,
        emitted,
        limits,
        frame,
        Some(references.plan),
        budget,
    )?;
    Ok(PendingRawAssemblyV29 { pending })
}

// This shared assembler is private to the opaque-owner module. Its raw result
// cannot be captured from a root callback before the consuming physical census.
fn assemble_pending_scoped_root_body_v29(
    instances: &ProductionCallInstancePlanV1<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    limits: ProductionSemanticKirLimitsV1,
    frame: Option<&scoped_slot_relocation_v29::FramePermitV29<'_, '_>>,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingScopedRootEmissionV29, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let mut next_block = pending_scope_preflight_v29(instances, emitted, limits, budget)?;
    check_scoped_defined_call_phases_with_references_v29(instances, emitted, references, budget)?;
    let result = with_production_instance_correspondence_v1(instances, budget, |map, budget| {
        // Validate every shared-emitter result before taking any caller slot.
        for (index, row) in emitted.iter().enumerate() {
            let id = instances
                .id_at(index)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            if instances.instance_reachable(id) == Some(false) {
                if row.is_some() {
                    return Err(InstanceCorrespondenceErrorV1::Source);
                }
                continue;
            }
            map.append_lowered(
                instances
                    .id_at(index)
                    .ok_or(InstanceCorrespondenceErrorV1::Source)?,
                row.as_ref().ok_or(InstanceCorrespondenceErrorV1::Source)?,
                budget,
            )?;
        }
        let mut sidecars = InstanceRowsV1::new();
        let mut functions = InstanceRowsV1::new();
        let mut retained = 0;
        let mut scratch = 0;
        // append_lowered already censused reachable emitted instances. With
        // only the root, no callee operations will move; final census remains.
        let mut storage_transport = references
            .filter(|plan| plan.has_storage_demands && map.seeds.rows.len() > 1)
            .map(|plan| ScopedStorageTransportV29::new(plan, map, emitted, budget, &mut scratch))
            .transpose()?;
        // The optional owner header is live even on the no-query/root-only path.
        call_splice_charge_storage_v1(
            std::mem::size_of::<Option<ScopedLaneQueryTransportV29>>(),
            budget,
            &mut scratch,
        )?;
        let mut lane_transport = if map.seeds.rows.len() > 1 {
            references
                .map(|plan| {
                    ScopedLaneQueryTransportV29::new(plan, map, emitted, budget, &mut scratch)
                })
                .transpose()?
                .flatten()
        } else {
            None
        };
        #[cfg(test)]
        if let (Some(observer), Some(references)) = (SCOPED_LANE_OBSERVER_V29.get(), references) {
            observer(references, map, emitted, lane_transport.as_ref(), budget)
                .map_err(instance_anchor_error_v1)?;
        }
        sidecars.reserve(emitted.len(), budget, &mut retained)?;
        functions.reserve(emitted.len(), budget, &mut scratch)?;
        budget.charge_work(emitted.len())?;
        for row in emitted.iter_mut() {
            let Some(row) = row.take() else {
                functions.rows.push(None);
                continue;
            };
            let (function, metadata) = PendingInstanceSidecarsV29::split(row);
            functions.rows.push(Some(function));
            sidecars.rows.push(metadata);
        }
        // One original-control index is retained by every later consumer.
        let active_instances = pending_active_instance_index_v1(instances, &sidecars.rows, budget)
            .map_err(|error| match error {
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => {
                    InstanceCorrespondenceErrorV1::Resource(error)
                }
                _ => InstanceCorrespondenceErrorV1::Source,
            })?;
        for index in (1..functions.rows.len()).rev() {
            budget.charge_work(4)?;
            let instance = instances
                .id_at(index)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            if instances.instance_reachable(instance) == Some(false) {
                if functions.rows[index].is_some() {
                    return Err(InstanceCorrespondenceErrorV1::Source);
                }
                continue;
            }
            let call = instances
                .incoming(instance)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            let caller = call.occurrence().caller.index();
            if caller >= index {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            let caller_function = functions.rows[caller]
                .take()
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            let callee_function = functions.rows[index]
                .take()
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            let entry = BlockId(next_block);
            let continuation = BlockId(next_block + 1);
            let mut expanded = map.splice_with_scoped_queries_v29(
                call,
                caller_function,
                callee_function,
                entry,
                continuation,
                frame,
                &sidecars.rows,
                &active_instances,
                storage_transport.as_ref(),
                lane_transport.as_ref(),
                budget,
            )?;
            if let Some(transport) = &mut storage_transport {
                transport.join(call.occurrence().caller, instance, budget)?;
            }
            if let Some(transport) = &mut lane_transport {
                transport.join(call.occurrence().caller, instance, budget)?;
            }
            next_block += 2;
            merge_pending_scope_capabilities_v29(
                &mut expanded.caller.required_capabilities,
                &mut expanded.callee_required_capabilities,
                budget,
            )?;
            functions.rows[caller] = Some(expanded.caller);
        }
        let function = functions.rows[0]
            .take()
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        drop(storage_transport);
        drop(lane_transport);
        drop(functions);
        budget.release_storage(scratch)?;
        let coordinates = map.take_owned_coordinates_v1(&function, budget)?;
        // Calculate all fallible accounting before moving the coordinate owner.
        let additional_storage_bytes = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if additional_storage_bytes < active_instances.storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok::<_, InstanceCorrespondenceErrorV1>(PendingScopedRootEmissionV29 {
            function,
            sidecars,
            active_instances,
            coordinates,
            slot_relocation: None,
            additional_storage_bytes,
        })
    });
    match result {
        Ok(pending) => {
            if let Err(error) = replay_pending_instance_asserts_v1(&pending, instances, budget) {
                drop(pending);
                let extra = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                budget.release_storage(extra)?;
                return Err(error);
            }
            Ok(pending)
        }
        Err(error) => {
            // Map cleanup and all failed payload drops precede this refund.
            let extra = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.release_storage(extra)?;
            Err(pending_scope_correspondence_error_v29(error))
        }
    }
}

// Retained reconstruction facts only. There is deliberately no public checked
// access query or completion constructor on this table. Full canonical/source
// replay and the final joint consumer must check the immutable candidate.
pub(super) struct PendingSourceMemoryV29 {
    source: ExecutionCallSourceV29,
    issued: PendingSourceIssuedRolesV29,
    accesses: Vec<PendingSourceMemoryAccessV29>,
    projects: Vec<PendingSourceObjectProjectV29>,
    alternatives: Vec<PendingSourceMemoryAlternativeV29>,
    effects: Vec<PendingSourceMemoryEffectV29>,
    initial: Vec<bool>,
    lifetimes: Vec<SourceAddressLifetimeV29>,
    kills: Vec<SourceAddressKillV29>,
    births: Vec<SourceAddressBirthV29>,
    indices: Vec<PendingSourceIndexV29>,
    index_failures: Vec<SourceIndexFailureV29>,
    index_guards: Vec<PendingSourceIndexGuardV29>,
    retained_storage: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceMemoryActivationV29 {
    Invocation,
    StorageLive {
        block: SemanticBlockIdV1,
        statement: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingSourceMemoryAccessV29 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    physical: SourceAddressAccessV29,
    object_location: Option<SourceStaticObjectLocationV29>,
    safe_object: Option<SourceSafeObjectOriginV29>,
    alternatives: std::ops::Range<usize>,
}

// A source activation site is a locator, never a dynamic incarnation. Keep it
// coupled to its original object/backing and validate currentness separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceMemoryAlternativeV29 {
    instance: ProductionCallInstanceIdV1,
    local: SemanticLocalIdV1,
    slot: usize,
    activation: SourceMemoryActivationV29,
    formation: Option<SourceReferenceSiteV29>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PendingSourceMemoryEffectV29 {
    Formation(SourceReferenceSiteV29),
    Boundary {
        instance: ProductionCallInstanceIdV1,
        anchor: usize,
    },
    FailureRead {
        instance: ProductionCallInstanceIdV1,
        anchor: usize,
    },
    Invocation(ProductionCallInstanceIdV1),
    Return {
        instance: ProductionCallInstanceIdV1,
        block: SemanticBlockIdV1,
    },
}

pub(super) fn pending_memory_matches_v29(
    left: Option<&PendingSourceMemoryV29>,
    right: Option<&PendingSourceMemoryV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    let (left, right) = match (left, right) {
        (None, None) => return Ok(true),
        (Some(left), Some(right)) => (left, right),
        _ => return Ok(false),
    };
    budget.charge_work(12)?;
    if left.source != right.source
        || left.accesses.len() != right.accesses.len()
        || left.projects.len() != right.projects.len()
        || left.alternatives.len() != right.alternatives.len()
        || left.effects.len() != right.effects.len()
        || left.initial.len() != right.initial.len()
        || left.lifetimes.len() != right.lifetimes.len()
        || left.kills.len() != right.kills.len()
        || left.births.len() != right.births.len()
        || left.indices.len() != right.indices.len()
        || left.index_failures.len() != right.index_failures.len()
        || left.index_guards.len() != right.index_guards.len()
    {
        return Ok(false);
    }
    if !pending_issued_roles_match_v29(&left.issued, &right.issued, budget)? {
        return Ok(false);
    }
    // Fixed-width source keys and physical locators only. The entire source
    // reconstruction compares these, but final physical checks still run on
    // the immutable canonical function. This is not another executable graph.
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(
            left.projects.len(),
            std::mem::size_of::<PendingSourceObjectProjectV29>(),
        )?,
        argument_product_v1(
            left.accesses.len(),
            std::mem::size_of::<PendingSourceMemoryAccessV29>(),
        )?,
        argument_product_v1(
            left.alternatives.len(),
            std::mem::size_of::<PendingSourceMemoryAlternativeV29>(),
        )?,
        argument_product_v1(
            left.effects.len(),
            std::mem::size_of::<PendingSourceMemoryEffectV29>(),
        )?,
        argument_product_v1(left.initial.len(), std::mem::size_of::<bool>())?,
        argument_product_v1(
            left.lifetimes.len(),
            std::mem::size_of::<SourceAddressLifetimeV29>(),
        )?,
        argument_product_v1(
            left.kills.len(),
            std::mem::size_of::<SourceAddressKillV29>(),
        )?,
        argument_product_v1(
            left.births.len(),
            std::mem::size_of::<SourceAddressBirthV29>(),
        )?,
        argument_product_v1(
            left.indices.len(),
            std::mem::size_of::<PendingSourceIndexV29>(),
        )?,
        argument_product_v1(
            left.index_failures.len(),
            std::mem::size_of::<SourceIndexFailureV29>(),
        )?,
        argument_product_v1(
            left.index_guards.len(),
            std::mem::size_of::<PendingSourceIndexGuardV29>(),
        )?,
    ])?)?;
    Ok(left.accesses == right.accesses
        && left.projects == right.projects
        && left.alternatives == right.alternatives
        && left.effects == right.effects
        && left.initial == right.initial
        && left.lifetimes == right.lifetimes
        && left.kills == right.kills
        && left.births == right.births
        && left.indices == right.indices
        && left.index_failures == right.index_failures
        && left.index_guards == right.index_guards)
}

// A scoped physical-only result for one immutable source/canonical owner. It
// cannot establish source arithmetic, ranked effects or execution permission.
pub(super) struct CheckedSourceMemoryV29<'scope> {
    correspondence: &'scope ProductionSourceCorrespondenceV18<'scope>,
    root: usize,
    pending: &'scope PendingSourceMemoryV29,
    required: usize,
}

pub(super) struct CheckedSourceMemoryAccessV29<'scope> {
    owner: &'scope CheckedSourceMemoryV29<'scope>,
    row: &'scope PendingSourceMemoryAccessV29,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    pointer: ValueId,
}

#[cfg(test)]
thread_local! {
    pub(super) static PHYSICAL_REFUND_OBSERVER_V29: std::cell::Cell<Option<fn(&mut ArgumentBudgetV1<'_>)>> = const { std::cell::Cell::new(None) };
    pub(super) static SOURCE_OBJECT_PAYLOAD_QUERY_SCRATCH_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    pub(super) static SOURCE_OBJECT_PAYLOAD_ROW_SCRATCH_V29: std::cell::Cell<(usize, usize, usize)> = const { std::cell::Cell::new((0, 0, 0)) };
}

fn physical_discard_headers_v29<T, E>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Result<T, E>>(),
        std::mem::size_of::<std::panic::AssertUnwindSafe<Result<T, E>>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

fn immutable_memory_error_v29(
    error: ProductionSemanticKirErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
        other => ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(other),
        ),
    }
}

fn immutable_memory_gap_v29(
    root: &ScopedModuleRootV29,
    block: BlockId,
    operation: usize,
    gap: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    budget.charge_work(1)?;
    let mut mapped = operation;
    for insertion in &root.insertions {
        budget.charge_work(3)?;
        if insertion.before.block == block
            && ((insertion.before.first as usize) < operation
                || (!gap && insertion.before.first as usize == operation))
        {
            mapped = argument_sum_v1(&[mapped, 1])?;
        }
    }
    let original = u32::try_from(operation).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let terminal = terminal_failure_ordinal_v18(
        root.terminal_failures.as_ref(),
        block,
        original,
        gap,
        budget,
    )
    .map_err(immutable_memory_error_v29)?;
    let terminal_delta = terminal
        .checked_sub(original)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    Ok(argument_sum_v1(&[mapped, terminal_delta as usize])?)
}

#[cfg(test)]
pub(super) fn test_immutable_memory_gap_v29(
    root: &ScopedModuleRootV29,
    block: BlockId,
    operation: usize,
    gap: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    immutable_memory_gap_v29(root, block, operation, gap, budget)
}

fn immutable_memory_access_v29(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    row: &PendingSourceMemoryAccessV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, ValueId)> {
    correspondence.query(budget)?;
    let owner = correspondence.source.root_row(root)?;
    let sidecar = correspondence
        .source
        .sidecar(root, row.instance.index(), budget)?;
    let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("physical census has no original anchors"),
    )?;
    let anchor = anchors
        .rows
        .get(row.anchor)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "physical census original anchor",
        ))?;
    if anchors.subject.instance != row.instance
        || !matches!(
            anchor.kind,
            ScopedMemoryAnchorKindV29::Access { .. } | ScopedMemoryAnchorKindV29::Object(_)
        )
    {
        return correspondence
            .source
            .missing("physical census changed original access");
    }
    let key = TileAttachmentKeyV29 {
        root,
        family: TileAttachmentFamilyV29::MemoryAnchor,
        instance: row.instance.index(),
        row: row.anchor,
        field: TileAttachmentFieldV29::MemoryPosition,
        component: 0,
        part: 0,
    };
    let [position] = correspondence.attachment_range(key, budget)? else {
        return correspondence
            .source
            .missing("physical census access position cardinality");
    };
    let ProductionSourceOperationV18::Operation(operation) =
        correspondence.mapped_source_operation(position.location, budget)?
    else {
        return correspondence
            .source
            .missing("physical census access is not an operation");
    };
    let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
        u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    );
    let block = correspondence
        .inventory
        .block_for_id(function, row.physical.block, budget)
        .map_err(|error| {
            ProductionSourceOwnedViewErrorV18::from(
                fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
            )
        })?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "physical census original block",
        ))?;
    let expected = immutable_memory_gap_v29(
        owner,
        row.physical.block,
        row.physical.operation,
        false,
        budget,
    )?;
    if operation.block != block.coordinate || operation.operation as usize != expected {
        return correspondence
            .source
            .missing("physical census lifecycle access mapping");
    }
    if matches!(anchor.kind, ScopedMemoryAnchorKindV29::Object(_)) {
        let object = correspondence.retained_object_payload_at_v29(
            root,
            row.instance.index(),
            row.anchor,
            operation,
            budget,
        )?;
        check_immutable_static_object_value_v29(correspondence, root, &object, budget)?;
        let pointer = match object.actual.operation {
            ScopedObjectOperationV29::ReadValue { address, .. }
            | ScopedObjectOperationV29::WriteValue { address, .. } => address,
            _ => {
                return correspondence
                    .source
                    .missing("physical census requires a whole typed value effect");
            }
        };
        return Ok((operation, pointer));
    }
    let [pointer] = correspondence.attachment_range(
        TileAttachmentKeyV29 {
            field: TileAttachmentFieldV29::MemoryPointer,
            ..key
        },
        budget,
    )?
    else {
        return correspondence
            .source
            .missing("physical census pointer cardinality");
    };
    let pointer = correspondence.attachment_value(root, pointer.location, budget)?;
    budget.charge_work(4)?;
    let actual = correspondence
        .inventory
        .functions()
        .get(owner.function_ordinal)
        .and_then(|row| row.function.body.as_ref())
        .and_then(|body| body.blocks.get(operation.block.block as usize))
        .and_then(|block| block.operations.get(operation.operation as usize))
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "physical census actual access",
        ))?;
    if !matches!(&actual.kind, OperationKind::Load { pointer: actual, .. }
        | OperationKind::Store { pointer: actual, .. } if *actual == pointer)
    {
        return correspondence
            .source
            .missing("physical census actual pointer differs");
    }
    Ok((operation, pointer))
}

impl CheckedSourceMemoryV29<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.correspondence.observe_custody(budget)?;
        if budget.storage() < self.required {
            self.correspondence.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.correspondence
            .retain_query(self.observe_custody(budget))?;
        self.correspondence.query(budget)
    }

    pub(super) fn visit_effects(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(
            PendingSourceMemoryEffectV29,
            &mut ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        self.correspondence.retain_query((|| {
            self.check(budget)?;
            for effect in &self.pending.effects {
                budget.charge_work(1)?;
                // The copied source key is a locator only; completion is tied
                // to this borrowed owner, never to the copied enum value.
                visit(*effect, budget)?;
                self.check(budget)?;
            }
            Ok(())
        })())
    }

    pub(super) fn access(
        &self,
        original_instance: usize,
        original_anchor: usize,
        actual_operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        actual_pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<CheckedSourceMemoryAccessV29<'_>>> {
        self.correspondence.retain_query((|| {
            self.check(budget)?;
            let mut found = None;
            for row in &self.pending.accesses {
                budget.charge_work(2)?;
                if row.instance.index() != original_instance || row.anchor != original_anchor {
                    continue;
                }
                if found.is_some() {
                    return self
                        .correspondence
                        .source
                        .missing("duplicate physical access identity");
                }
                found = Some(row);
            }
            let row = found.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "missing physical access identity",
            ))?;
            // General ordinary backing activation is still an explicit pending
            // obligation; an empty raw-alternative range is never its proof.
            if row.alternatives.is_empty() {
                return Ok(None);
            }
            let (operation, pointer) =
                immutable_memory_access_v29(self.correspondence, self.root, row, budget)?;
            if operation != actual_operation || pointer != actual_pointer {
                return self
                    .correspondence
                    .source
                    .missing("physical access query changed its actual operation or pointer");
            }
            Ok(Some(CheckedSourceMemoryAccessV29 {
                owner: self,
                row,
                operation,
                pointer,
            }))
        })())
    }
}

impl CheckedSourceMemoryAccessV29<'_> {
    pub(super) fn operation_pointer(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, ValueId)> {
        self.owner.check(budget)?;
        Ok((self.operation, self.pointer))
    }

    pub(super) fn visit_alternatives(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(
            usize,
            SemanticLocalIdV1,
            usize,
            Option<(SemanticBlockIdV1, usize)>,
            &mut ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        self.owner.correspondence.retain_query((|| {
            self.owner.check(budget)?;
            let rows = self
                .owner
                .pending
                .alternatives
                .get(self.row.alternatives.clone())
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "physical activation interval",
                ))?;
            if rows.is_empty() {
                return self
                    .owner
                    .correspondence
                    .source
                    .missing("missing physical activation alternatives");
            }
            for row in rows {
                budget.charge_work(2)?;
                let activation = match row.activation {
                    SourceMemoryActivationV29::Invocation => None,
                    SourceMemoryActivationV29::StorageLive { block, statement } => {
                        Some((block, statement))
                    }
                };
                visit(
                    row.instance.index(),
                    row.local,
                    row.slot,
                    activation,
                    budget,
                )?;
                self.owner.check(budget)?;
            }
            Ok(())
        })())
    }
}

pub(super) fn immutable_index_header_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Vec<(usize, usize, usize)>>(),
        std::mem::size_of::<(
            Vec<SourceIndexLocationV29>,
            Vec<SourceIndexGuardLocationV29>,
        )>(),
        std::mem::size_of::<
            SourceOwnedResultV18<(
                Vec<SourceIndexLocationV29>,
                Vec<SourceIndexGuardLocationV29>,
            )>,
        >(),
    ])
}

fn immutable_index_locations_v29(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    pending: &PendingSourceMemoryV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(
    Vec<SourceIndexLocationV29>,
    Vec<SourceIndexGuardLocationV29>,
)> {
    budget.reserve_storage(immutable_index_header_v29()?)?;
    let owner = correspondence.source.root_row(root)?;
    let mut access_index =
        emission_vec_v1(pending.accesses.len(), budget).map_err(immutable_memory_error_v29)?;
    for (ordinal, row) in pending.accesses.iter().enumerate() {
        budget.charge_work(1)?;
        access_index.push((row.instance.index(), row.anchor, ordinal));
    }
    call_splice_sort_work_v1(access_index.len(), budget)
        .map_err(source_address_call_error_v29)
        .map_err(immutable_memory_error_v29)?;
    access_index.sort_unstable_by_key(|row| (row.0, row.1));
    for pair in access_index.windows(2) {
        budget.charge_work(2)?;
        if (pair[0].0, pair[0].1) == (pair[1].0, pair[1].1) {
            return correspondence
                .source
                .missing("duplicate index memory access identity");
        }
    }
    let mut output =
        emission_vec_v1(pending.indices.len(), budget).map_err(immutable_memory_error_v29)?;
    for original in &pending.indices {
        budget.charge_work(4)?;
        let mut source = *original;
        source.operation =
            immutable_memory_gap_v29(owner, source.block, source.operation, false, budget)?;
        let load =
            match (source.load_anchor, source.index_slot) {
                (None, None) => None,
                (Some(anchor), Some(slot)) => {
                    budget.charge_work(call_splice_search_work_v1(access_index.len()))?;
                    let access = access_index
                        .binary_search_by_key(&(source.instance.index(), anchor), |row| {
                            (row.0, row.1)
                        })
                        .ok()
                        .map(|at| &pending.accesses[access_index[at].2])
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "missing original retained index load",
                        ))?;
                    let (operation, _) =
                        immutable_memory_access_v29(correspondence, root, access, budget)?;
                    let sidecar =
                        correspondence
                            .source
                            .sidecar(root, source.instance.index(), budget)?;
                    let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("retained index source anchors"),
                    )?;
                    let row = anchors.rows.get(anchor).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("retained index source anchor"),
                    )?;
                    // Physical replay already checked this exact mapped operation.
                    // Both original scalar and typed index reads retain the same
                    // independently authenticated source-use coordinates.
                    let payload = match scoped_original_index_payload_v29(anchors, row, budget) {
                        Ok(payload) => payload,
                        Err(error) => return Err(immutable_memory_error_v29(error)),
                    };
                    let Some((result, read)) = payload else {
                        return correspondence.source.missing("retained index payload role");
                    };
                    let (function, _) =
                        correspondence
                            .source
                            .instance(root, source.instance.index(), budget)?;
                    let ssa = &correspondence.source.owner.inner.source.owner;
                    let source_occurrences =
                        ssa.occurrences_v1()
                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                "retained index original occurrence owner",
                            ))?;
                    let occurrences = source_occurrences.function(function).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "retained index original occurrences",
                        ),
                    )?;
                    let original_function = ssa
                        .source_semantic()
                        .functions()
                        .get(function.index() as usize)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "retained index original function",
                        ))?;
                    check_scoped_index_read_v29(original_function, &occurrences, read, budget)
                        .map_err(immutable_memory_error_v29)?;
                    if result != source.original
                        || read.event != source.event
                        || access.physical.slot != slot
                    {
                        return correspondence
                            .source
                            .missing("retained index source/load substitution");
                    }
                    Some(operation)
                }
                _ => {
                    return correspondence
                        .source
                        .missing("retained index incomplete load recipe");
                }
            };
        output.push(SourceIndexLocationV29 { source, load });
    }
    let mut guards =
        emission_vec_v1(pending.index_guards.len(), budget).map_err(immutable_memory_error_v29)?;
    for original in &pending.index_guards {
        budget.charge_work(call_splice_search_work_v1(access_index.len()))?;
        let access = access_index
            .binary_search_by_key(&(original.instance.index(), original.load_anchor), |row| {
                (row.0, row.1)
            })
            .ok()
            .map(|at| &pending.accesses[access_index[at].2])
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "guard original load access",
            ))?;
        let (load, _) = immutable_memory_access_v29(correspondence, root, access, budget)?;
        let sidecar = correspondence
            .source
            .sidecar(root, original.instance.index(), budget)?;
        let anchor = sidecar
            .scoped_memory_anchors
            .as_ref()
            .and_then(|rows| rows.rows.get(original.load_anchor))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "guard original load anchor",
            ))?;
        let ScopedMemoryAnchorKindV29::Access {
            payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
            ..
        } = anchor.kind
        else {
            return correspondence
                .source
                .missing("guard changed load payload role");
        };
        let (function, _) =
            correspondence
                .source
                .instance(root, original.instance.index(), budget)?;
        let ssa = &correspondence.source.owner.inner.source.owner;
        let source_occurrences =
            ssa.occurrences_v1()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "guard source occurrence owner",
                ))?;
        let occurrences = source_occurrences.function(function).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("guard source occurrences"),
        )?;
        let declaration = ssa
            .source_semantic()
            .functions()
            .get(function.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "guard source declaration",
            ))?;
        let (site, place, length, _) = source_index_guard_original_v29(
            declaration,
            &occurrences,
            ssa.source_semantic().types(),
            original.assertion,
            original.condition_event,
            original.comparison_event,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        let slot = owner.source_slots.slots.get(original.slot).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("guard source slot"),
        )?;
        let scalar = slot.scalar_array().map_err(immutable_memory_error_v29)?;
        let local = slot.legacy_local().map_err(immutable_memory_error_v29)?;
        if result != original.value
            || read.site != site
            || read.role != ExecutionOperandV29::RvalueOperand(0)
            || read.prefix != 0
            || read.ty != place.ty()
            || !matches!(read.occurrence, ScopedMemoryOccurrenceV29::Retained { .. })
            || access.physical.slot != original.slot
            || slot.instance != original.instance
            || local != place.local().index()
            || scalar.element_type != place.ty()
            || original.length != length
            || scalar.length != 1
        {
            return correspondence
                .source
                .missing("guard source read substitution");
        }
        check_scoped_payload_occurrence_v29(
            &occurrences,
            site,
            ExecutionOperandV29::RvalueOperand(0),
            place,
            read.occurrence,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        let capture = sidecar.instance_assert_origins.as_ref().ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("guard original assertion"),
        )?;
        budget.charge_work(capture.records.len())?;
        let mut records = capture
            .records
            .iter()
            .filter(|row| row.site.semantic_block == original.assertion);
        let record = records
            .next()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "guard assertion record",
            ))?;
        if records.next().is_some()
            || record.block != original.block
            || record.physical_success != original.success
            || !matches!(record.outcome, PendingAssertOutcomeV1::Emitted { condition, failure }
                if condition == original.condition && failure == original.failure)
        {
            return correspondence
                .source
                .missing("guard actual assertion substitution");
        }
        let mut source = *original;
        if let Some(terminal) = checked_terminal_assertion_v18(
            owner.terminal_failures.as_ref(),
            source.instance,
            record,
            budget,
        )
        .map_err(immutable_memory_error_v29)?
        {
            if terminal.original_failure != source.failure {
                return correspondence.source.missing("guard failure substitution");
            }
            source.failure = terminal.actual_failure;
        }
        guards.push(SourceIndexGuardLocationV29 { source, load });
    }
    let bytes = argument_product_v1(
        access_index.capacity(),
        std::mem::size_of::<(usize, usize, usize)>(),
    )?;
    drop(access_index);
    budget.release_storage(bytes)?;
    Ok((output, guards))
}

include!("production_scoped_static_object_replay_v29.rs");

fn check_immutable_source_memory_v29(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    memory: Option<&fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    correspondence.query(budget)?;
    let owner = correspondence.source.root_row(root)?;
    if !correspondence
        .inventory
        .belongs_to(&correspondence.source.owner.inner.pending.graph)
    {
        return correspondence
            .source
            .missing("physical census foreign immutable graph");
    }
    let pending = owner.source_slots.pending_memory.as_ref().ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("physical source obligations remain pending"),
    )?;
    if let Some(memory) = memory
        && !memory.belongs_to(correspondence.inventory)
    {
        return correspondence
            .source
            .missing("retained source index has a foreign memory-version owner");
    }
    if !pending.indices.is_empty() && memory.is_none() {
        return correspondence
            .source
            .missing("retained source index memory versions remain pending");
    }
    budget.charge_work(5)?;
    if pending.source.semantic != owner.coordinates.semantic_sha256
        || pending.source.ssa != owner.coordinates.ssa
        || pending.source.root != owner.coordinates.root
        || pending.initial.len() != owner.source_slots.slots.len()
    {
        return correspondence
            .source
            .missing("physical census changed original source");
    }
    check_immutable_issued_roles_v18(correspondence, root, &pending.issued, budget)?;
    let function = correspondence
        .inventory
        .functions()
        .get(owner.function_ordinal)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "physical census root function",
        ))?
        .function;
    let mut accesses =
        emission_vec_v1(pending.accesses.len(), budget).map_err(immutable_memory_error_v29)?;
    for row in &pending.accesses {
        let (operation, _) = immutable_memory_access_v29(correspondence, root, row, budget)?;
        accesses.push(SourceAddressAccessV29 {
            block: row.physical.block,
            operation: operation.operation as usize,
            slot: row.physical.slot,
        });
    }
    let mut kills =
        emission_vec_v1(pending.kills.len(), budget).map_err(immutable_memory_error_v29)?;
    for row in &pending.kills {
        kills.push(SourceAddressKillV29 {
            gap: immutable_memory_gap_v29(owner, row.block, row.gap, true, budget)?,
            ..*row
        });
    }
    let mut lifetimes =
        emission_vec_v1(pending.lifetimes.len(), budget).map_err(immutable_memory_error_v29)?;
    for row in &pending.lifetimes {
        lifetimes.push(SourceAddressLifetimeV29 {
            gap: immutable_memory_gap_v29(owner, row.block, row.gap, true, budget)?,
            ..*row
        });
    }
    let mut births =
        emission_vec_v1(pending.births.len(), budget).map_err(immutable_memory_error_v29)?;
    for row in &pending.births {
        births.push(SourceAddressBirthV29 {
            operation: immutable_memory_gap_v29(owner, row.block, row.operation, false, budget)?,
            ..*row
        });
    }
    let mut failures = emission_vec_v1(pending.index_failures.len(), budget)
        .map_err(immutable_memory_error_v29)?;
    for row in &pending.index_failures {
        budget.charge_work(5)?;
        let sidecar = correspondence
            .source
            .sidecar(root, row.instance.index(), budget)?;
        let anchor = sidecar
            .scoped_memory_anchors
            .as_ref()
            .and_then(|rows| rows.rows.get(row.anchor))
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "missing retained index diagnostic",
            ))?;
        let (function, _) = correspondence
            .source
            .instance(root, row.instance.index(), budget)?;
        let ssa = &correspondence.source.owner.inner.source.owner;
        let source_occurrences =
            ssa.occurrences_v1()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "index diagnostic occurrence owner",
                ))?;
        let occurrences = source_occurrences.function(function).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("index diagnostic original occurrences"),
        )?;
        let original = ssa
            .source_semantic()
            .functions()
            .get(function.index() as usize)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "index diagnostic original function",
            ))?;
        let place = checked_scoped_failure_read_v29(original, &occurrences, anchor, budget)
            .map_err(immutable_memory_error_v29)?;
        let slot = owner.source_slots.slots.get(row.slot).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("index diagnostic source slot"),
        )?;
        let local = match slot.origin.identity {
            ScopedAllocationIdentityV29::LegacyLocal(local)
            | ScopedAllocationIdentityV29::OriginalObject { local, .. } => local,
            _ => {
                return correspondence
                    .source
                    .missing("failure diagnostic source representation");
            }
        };
        if slot.instance != row.instance
            || local != place.local().index()
            || slot.origin.semantic_type != place.ty()
            || !place.projections().is_empty()
            || source_failure_operand_moved_v29(original, anchor, budget)
                .map_err(immutable_memory_error_v29)?
                != row.move_after
            || matches!(slot.representation, ScopedSlotRepresentationV29::ScalarArray(scalar)
                if scalar.element_type != place.ty() || scalar.length != 1
                    || !matches!(scalar.element.element, PrivateRetainedElementFactsV1::Scalar(_)))
        {
            return correspondence
                .source
                .missing("index diagnostic source substitution");
        }
        failures.push(SourceIndexFailureV29 {
            gap: immutable_memory_gap_v29(owner, row.block, row.gap, true, budget)?,
            ..*row
        });
    }
    // These are the actual final operations, not an erased or reconstructed
    // shadow graph. All memory/operand/edge/return uses are checked again.
    let slots = &owner.source_slots.slots;
    let prepared = SourceAddressMemoryV29::prepare_inventory(
        correspondence.inventory,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        ),
        slots,
        &accesses,
        budget,
    )
    .map_err(immutable_memory_error_v29)?;
    let indexed = source_array_geometry_v29(slots, !pending.indices.is_empty(), budget)?;
    let (graph, geometry) = if !indexed {
        (
            prepared
                .solve(slots, &accesses, &kills, budget)
                .map_err(immutable_memory_error_v29)?,
            SourceAddressGeometryV29::Scalar,
        )
    } else {
        let pending = prepared
            .solve_pending_indices(slots, &accesses, &kills, budget)
            .map_err(immutable_memory_error_v29)?;
        (pending.graph, SourceAddressGeometryV29::PendingIndices)
    };
    check_immutable_static_object_projects_v29(correspondence, root, pending, &graph, budget)?;
    for (source, access) in pending.accesses.iter().zip(&accesses) {
        budget.charge_work(2)?;
        let actual = graph.blocks[graph
            .block(access.block, budget)
            .map_err(immutable_memory_error_v29)?]
        .1
        .operations
        .get(access.operation)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "final object access coordinate",
        ))?;
        let value = source_address_value_access_v29(actual)
            .map_err(immutable_memory_error_v29)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "final object access effect",
            ))?;
        let location = if value.object {
            Some(
                graph
                    .object_location(value.pointer, budget)
                    .map_err(immutable_memory_error_v29)?,
            )
        } else {
            None
        };
        if location != source.object_location {
            return correspondence
                .source
                .missing("final object access changed original static location");
        }
    }
    check_source_address_currentness_geometry_v29(
        function,
        &graph,
        slots,
        &accesses,
        &kills,
        &pending.initial,
        &lifetimes,
        &births,
        &failures,
        geometry,
        budget,
    )
    .map_err(immutable_memory_error_v29)?;
    if !indexed {
        scoped_slot_uses_v29::check_expanded_scalar_addresses_with_failures_v29(
            function, &graph, slots, &accesses, &kills, &failures, budget,
        )
        .map_err(immutable_memory_error_v29)?;
    } else {
        let (selected, guards) =
            immutable_index_locations_v29(correspondence, root, pending, budget)?;
        let function_coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        scoped_slot_uses_v29::check_expanded_index_addresses_v29(
            function,
            &graph,
            slots,
            &accesses,
            &kills,
            &lifetimes,
            &failures,
            &selected,
            &guards,
            correspondence.inventory,
            memory,
            function_coordinate,
            budget,
        )
        .map_err(immutable_memory_error_v29)?;
        drop((selected, guards));
    }
    drop((graph, accesses, kills, lifetimes, births, failures));
    Ok(())
}

pub(super) fn with_checked_source_memory_v29<'work, T, E>(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    original_root: usize,
    memory: Option<&fe2o3_kernel_analysis::CanonicalKirMemorySsaV18<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl for<'scope> FnOnce(
        &CheckedSourceMemoryV29<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<ProductionSourceOwnedViewErrorV18>,
{
    correspondence.query(budget)?;
    let floor = budget.storage();
    scoped_source_attempt_v29(correspondence.source.cleanup, budget, floor, |budget| {
        let floor = budget.storage();
        correspondence.source.retain_construction(|| {
            // Exactly one rejected output or superseded error can require
            // destruction. Pay its initial drop and bounded payload retries
            // before the callback can exhaust the shared work allowance.
            budget.charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)?;
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<CheckedSourceMemoryV29<'_>>(),
                std::mem::size_of::<std::thread::Result<Result<T, E>>>(),
                argument_product_v1(5, std::mem::size_of::<Vec<usize>>())?,
                physical_discard_headers_v29::<T, E>()?,
            ])?)?;
            check_immutable_source_memory_v29(correspondence, original_root, memory, budget)
        })?;
        let construction_storage = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let pending = correspondence
            .source
            .root_row(original_root)?
            .source_slots
            .pending_memory
            .as_ref()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "physical census lost pending owner",
            ))?;
        let view = CheckedSourceMemoryV29 {
            correspondence,
            root: original_root,
            pending,
            required: budget.storage(),
        };
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(&view, budget)));
        let first = correspondence.source.guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            view.check(budget)
        } else {
            view.observe_custody(budget)
        };
        drop(view);
        let value = match caught {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(Err(error)) => {
                let selected = match first {
                    Some(first) => {
                        let selected = first.error();
                        source_reference_discard_v29(Err::<T, E>(error));
                        selected.into()
                    }
                    None => error,
                };
                return Err(SourceConsumerErrorV18(selected));
            }
            Ok(Ok(value)) => match postflight {
                Ok(()) => value,
                Err(error) => {
                    source_reference_discard_v29(Ok::<T, E>(value));
                    return Err(SourceConsumerErrorV18(error.into()));
                }
            },
        };
        // The source scope owns no C2 arena; all solver scratch has dropped.
        // Only the delta captured before the consumer may be released. The
        // consumer's separately retained output is never part of this refund.
        #[cfg(test)]
        if let Some(observe) = PHYSICAL_REFUND_OBSERVER_V29.get() {
            observe(budget);
        }
        let settlement = correspondence.source.retain_construction(|| {
            budget
                .release_storage(construction_storage)
                .map_err(Into::into)
        });
        if let Err(error) = settlement {
            correspondence.source.cleanup.deny_refund();
            source_reference_discard_v29(Ok::<T, E>(value));
            return Err(SourceConsumerErrorV18(error.into()));
        }
        Ok(value)
    })
    .map_err(|error: SourceConsumerErrorV18<E>| error.0)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceRootKindV29 {
    Ordinary,
    ExpandedRaw,
}

struct SourceRootTransactionV29<'root, 'source> {
    instances: &'root ExecutionInstancesV29<'source>,
    references: Option<SourceReferenceEmissionV29<'root, 'source>>,
    limits: ProductionSemanticKirLimitsV1,
    kind: Option<SourceRootKindV29>,
}

pub(super) struct SourceRootPreparationV29<'borrow, 'root, 'source> {
    owner: &'borrow SourceRootTransactionV29<'root, 'source>,
}

// No caller can extract this graph or choose the discriminator. The original
// emission owner remains in the transaction until the consuming final census.
pub(super) struct UnfinishedSourceRootV29 {
    source: ExecutionCallSourceV29,
    emission_owner: Option<usize>,
    kind: SourceRootKindV29,
    pending: PendingScopedRootEmissionV29,
    payload: PrivateArrayPayloadV1,
    slots: OwnedScopedSourceSlotsV29,
}

pub(super) struct PendingRawAssemblyV29 {
    pending: PendingScopedRootEmissionV29,
}

impl PendingRawAssemblyV29 {
    pub(super) fn relocate(
        self,
        original: &SourceRootPreparationV29<'_, '_, '_>,
        instances: &ExecutionInstancesV29<'_>,
        slots: &OwnedScopedSourceSlotsV29,
        relocation: scoped_slot_relocation_v29::RelocationV29,
        floor: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        original.check_raw(instances, budget)?;
        let pending = relocation.complete_pending(self.pending, instances, slots, floor, budget)?;
        Ok(Self { pending })
    }
}

type SourceRootOutputV29 = (
    PendingScopedRootEmissionV29,
    PrivateArrayPayloadV1,
    OwnedScopedSourceSlotsV29,
);

fn source_root_header_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<SourceRootTransactionV29<'_, '_>>(),
        std::mem::size_of::<SourceRootPreparationV29<'_, '_, '_>>(),
        std::mem::size_of::<UnfinishedSourceRootV29>(),
        std::mem::size_of::<PendingRawAssemblyV29>(),
        std::mem::size_of::<Result<PendingRawAssemblyV29, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<Result<UnfinishedSourceRootV29, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<
            Result<SourceRootOutputV29, source_storage_v29::RecordedStorageFailureV29<'_>>,
        >(),
        std::mem::size_of::<
            std::thread::Result<
                Result<SourceRootOutputV29, source_storage_v29::RecordedStorageFailureV29<'_>>,
            >,
        >(),
        std::mem::size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>(),
    ])
}

impl<'root, 'source> SourceRootPreparationV29<'_, 'root, 'source> {
    pub(super) fn references(&self) -> Option<&SourceReferenceEmissionV29<'root, 'source>> {
        self.owner.references.as_ref()
    }

    pub(super) fn check_raw(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&SourceReferenceEmissionV29<'root, 'source>, ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if !std::ptr::eq(instances, self.owner.instances)
            || self.owner.kind != Some(SourceRootKindV29::ExpandedRaw)
        {
            return Err(source_raw_physical_error_v29());
        }
        let references = self
            .references()
            .ok_or_else(source_raw_physical_error_v29)?;
        references.plan.check_owner(instances, budget)?;
        references.check(budget)?;
        if !original_census(instances, Some(references.plan), budget)?.requires_physical() {
            return Err(source_raw_physical_error_v29());
        }
        Ok(references)
    }

    pub(super) fn assemble(
        &self,
        emitted: &mut [Option<LoweredFunctionResultV1>],
        slots: OwnedScopedSourceSlotsV29,
        identities: Option<&ExecutionIdentityPlanV1<'_, '_>>,
        payload: PrivateArrayPayloadV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<UnfinishedSourceRootV29, ProductionSemanticKirErrorV1> {
        let instances = self.owner.instances;
        let limits = self.owner.limits;
        let kind = self.owner.kind.ok_or_else(source_raw_physical_error_v29)?;
        let pending = match kind {
            SourceRootKindV29::Ordinary => {
                let relocation = scoped_slot_relocation_v29::prepare_with_identities_v1(
                    instances,
                    emitted,
                    &slots,
                    limits.max_operations,
                    self.references(),
                    identities,
                    budget,
                )?;
                relocation.assemble_with_references(
                    limits,
                    self.references().map(|row| row.plan),
                    budget,
                )?
            }
            SourceRootKindV29::ExpandedRaw => {
                let relocation = scoped_slot_relocation_v29::prepare_pending_raw_v29(
                    self,
                    instances,
                    emitted,
                    &slots,
                    limits.max_operations,
                    identities,
                    budget,
                )?;
                relocation.assemble_pending(self, limits, budget)?.pending
            }
        };
        Ok(UnfinishedSourceRootV29 {
            source: ExecutionCallSourceV29::from_instances(instances, budget)?,
            emission_owner: self
                .references()
                .map(|row| std::ptr::from_ref(row) as usize),
            kind,
            pending,
            payload,
            slots,
        })
    }
}

impl SourceRootTransactionV29<'_, '_> {
    fn complete_candidate(
        &self,
        unfinished: UnfinishedSourceRootV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceRootOutputV29, ProductionSemanticKirErrorV1> {
        let UnfinishedSourceRootV29 {
            source,
            emission_owner,
            kind,
            mut pending,
            payload,
            mut slots,
        } = unfinished;
        budget.charge_work(3)?;
        if source != ExecutionCallSourceV29::from_instances(self.instances, budget)?
            || emission_owner
                != self
                    .references
                    .as_ref()
                    .map(|row| std::ptr::from_ref(row) as usize)
            || self.kind != Some(kind)
        {
            return Err(source_raw_physical_error_v29());
        }
        let references = self.references.as_ref();
        let plan = references.map(|row| row.plan);
        let raw = original_census(self.instances, plan, budget)?.requires_physical();
        if raw != (kind == SourceRootKindV29::ExpandedRaw) {
            return Err(source_raw_physical_error_v29());
        }
        if let Some(references) = references {
            references.check(budget)?;
            if kind == SourceRootKindV29::Ordinary {
                references.finish(budget)?;
            }
        }
        let plan = plan.ok_or_else(source_raw_physical_error_v29)?;
        check_root_execution_archives_v29(&pending, self.instances, plan, budget)?;
        check_pending_object_completion_v29(
            &pending,
            self.instances,
            plan,
            &slots,
            kind == SourceRootKindV29::ExpandedRaw,
            budget,
        )?;
        #[cfg(test)]
        if let Some(observe) = ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.get() {
            observe(&mut pending, self.instances, plan, budget)?;
        }
        if kind == SourceRootKindV29::ExpandedRaw
            || ordinary_scalar_memory_profile_v29(plan, &slots, budget)?
        {
            let references = references.ok_or_else(source_raw_physical_error_v29)?;
            let relocation = pending
                .slot_relocation
                .as_ref()
                .ok_or_else(source_raw_physical_error_v29)?;
            let parts = ScopedDeferredScalarViewV29::for_pending_root(
                self.instances,
                &pending,
                relocation,
                budget,
            )?;
            let memory = check_expanded_source_memory_v29(
                self.instances,
                references,
                &pending,
                &slots,
                Some(&parts),
                budget,
            )?;
            drop(parts);
            references.finish_source_claims_v29(budget)?;
            slots.retained_storage =
                argument_sum_v1(&[slots.retained_storage, memory.retained_storage])?;
            if slots.pending_memory.replace(memory).is_some() {
                return Err(source_raw_physical_error_v29());
            }
        }
        #[cfg(not(test))]
        discard_root_execution_archives_v29(&mut pending, self.instances, plan, budget)?;
        Ok((pending, payload, slots))
    }
}

pub(super) fn with_original_source_root_v29<'root, 'source, 'view, 'arena, 'work>(
    instances: &'root ExecutionInstancesV29<'source>,
    plan: Option<&'root SourceReferencePlanV29<'root, 'source>>,
    root: &source_storage_v29::SourceStorageRootV29<'view, 'arena, 'source>,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'work>,
    emit: impl FnOnce(
        &SourceRootPreparationV29<'_, 'root, 'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<UnfinishedSourceRootV29, ProductionSemanticKirErrorV1>,
) -> Result<SourceRootOutputV29, source_storage_v29::SourceStorageRootCallbackErrorV29<'view>> {
    let mut owner = SourceRootTransactionV29 {
        instances,
        references: None,
        limits,
        kind: None,
    };
    let ledger = budget.work_ledger_identity_v1();
    let slot = std::ptr::from_ref(budget) as usize;
    let floor = budget.storage();
    let mut paid = None;
    let mut growth = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let assembly = (|| {
            let header = source_root_header_v29()?;
            budget.charge_work(argument_sum_v1(&[
                12,
                if plan.and_then(|row| row.storage_root.as_ref()).is_some() {
                    source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29
                } else {
                    0
                },
            ])?)?;
            let required = argument_sum_v1(&[floor, header])?;
            budget.reserve_storage(header)?;
            paid = Some((header, required));
            if let Some(custody) = plan.and_then(|row| row.storage_root.as_ref()) {
                growth = Some(
                    custody
                        .capture_retained_growth()
                        .ok_or(ArgumentResourceV1::Accounting)?,
                );
            }
            owner.references = source_reference_optional_emission_v29(plan, budget)?;
            owner.kind = Some(
                if original_census(instances, plan, budget)?.requires_physical() {
                    SourceRootKindV29::ExpandedRaw
                } else {
                    SourceRootKindV29::Ordinary
                },
            );
            emit(&SourceRootPreparationV29 { owner: &owner }, budget)
        })();
        let assembly = assembly.map_err(|error| root.record_callback_failure(error));
        #[cfg(test)]
        if owner.references.is_some()
            && let Some(observe) = SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29.get()
        {
            observe(
                owner.references.as_mut(),
                instances,
                budget,
                assembly.is_ok(),
            );
        }
        assembly.and_then(|unfinished| {
            owner
                .complete_candidate(unfinished, budget)
                .map_err(|error| root.record_callback_failure(error))
        })
    }));
    if result.is_err() {
        // Select the established C1 panic diagnostic before abort cleanup can
        // observe a later custody failure. The outer owner disposes the payload.
        root.record_callback_failure(source_reference_error_v29(
            "source reference callback panicked",
        ));
    }
    let successful = matches!(&result, Ok(Ok(_)));
    let cleanup = match owner.references.take() {
        Some(references) if successful => {
            let owned = references.owned;
            let retained = references.plan.retains_custody(instances, budget)
                && budget.storage() >= references.floor
                && references
                    .plan
                    .storage_root
                    .as_ref()
                    .is_none_or(|custody| custody.retains_after_refund(owned, budget));
            if !retained && let Some(custody) = references.plan.storage_root.as_ref() {
                custody.deny_active_root_refund();
            }
            drop(references);
            if retained {
                budget.release_storage(owned).map_err(Into::into)
            } else {
                Err(ArgumentResourceV1::Accounting.into())
            }
        }
        Some(references) => {
            #[cfg(test)]
            if let Some(observe) = SOURCE_REFERENCE_ABORT_OBSERVER_V29.get() {
                observe(&references, budget);
            }
            let result = references.abort_scope(instances, budget);
            #[cfg(test)]
            if let Some(observe) = SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29.get() {
                observe(None, instances, budget, false);
            }
            result
        }
        None => Ok(()),
    };
    drop(owner);
    let result = match (result, cleanup) {
        (Ok(Ok(value)), Err(error)) => {
            drop(value);
            Ok(Err(root.record_callback_failure(error)))
        }
        (result, _) => result,
    };
    // The nested reference owner and transaction are gone before releasing the
    // lexical header. Candidate payload and live C2 growth remain paid.
    let refunded = match paid {
        Some((header, required)) => scoped_emission_refund_v29(
            plan,
            ledger,
            slot,
            floor,
            required,
            growth,
            Some(header),
            budget,
        ),
        None => true,
    };
    match result {
        Ok(Ok(value)) if !refunded => {
            drop(value);
            Err(root
                .record_callback_failure(ArgumentResourceV1::Accounting.into())
                .into())
        }
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(error.into()),
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

// This check cannot mint a checked source/ranked owner. It retains pending
// reconstruction rows only after the source-coordinate and physical-memory
// obligations implemented here have been checked on this same candidate.
pub(super) fn check_expanded_source_memory_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    pending: &PendingScopedRootEmissionV29,
    slots: &OwnedScopedSourceSlotsV29,
    parts: Option<&ScopedDeferredScalarViewV29<'_, '_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingSourceMemoryV29, ProductionSemanticKirErrorV1> {
    references.plan.check_owner(instances, budget)?;
    references.check(budget)?;
    let plan = references.plan;
    let root = plan.storage_root.as_ref();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget
        .prepared_input_slot_v1()
        .ok_or(ArgumentResourceV1::Accounting)?;
    let floor = budget.storage();
    let header = argument_sum_v1(&[
        std::mem::size_of::<PendingSourceMemoryV29>(),
        std::mem::size_of::<PendingSourceIssuedRolesV29>(),
        std::mem::size_of::<Result<PendingSourceIssuedRolesV29, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<(
            Vec<SourceAddressAccessSourceV29>,
            PendingSourceIssuedRolesV29,
        )>(),
        std::mem::size_of::<
            Result<
                (
                    Vec<SourceAddressAccessSourceV29>,
                    PendingSourceIssuedRolesV29,
                ),
                ProductionSemanticKirErrorV1,
            >,
        >(),
        std::mem::size_of::<SourceAddressMemoryV29<'_>>(),
        std::mem::size_of::<SourceAddressLifetimesV29>(),
        std::mem::size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>(),
        std::mem::size_of::<Result<PendingSourceMemoryV29, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<
            std::thread::Result<Result<PendingSourceMemoryV29, ProductionSemanticKirErrorV1>>,
        >(),
        argument_product_v1(5, std::mem::size_of::<Vec<usize>>())?,
    ])?;
    // Prepay the fixed exit checks. No remaining traversal work can replace a
    // selected error or prevent checking the original concrete cleanup floor.
    budget.charge_work(argument_sum_v1(&[
        4,
        8,
        if root.is_some() {
            source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29
        } else {
            0
        },
    ])?)?;
    let required = argument_sum_v1(&[floor, header])?;
    budget.reserve_storage(header)?;
    let mut growth = None;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(root) = root {
            growth = Some(
                root.capture_retained_growth()
                    .ok_or(ArgumentResourceV1::Accounting)?,
            );
        }
        check_expanded_source_memory_inner_v29(instances, references, pending, slots, parts, budget)
    }));
    match result {
        Ok(Ok(value)) => {
            let output = value.retained_storage;
            let checked_required = required.checked_add(output);
            let refund = budget
                .storage()
                .checked_sub(floor)
                .and_then(|owned| owned.checked_sub(output));
            if checked_required.is_some_and(|checked_required| {
                scoped_emission_refund_v29(
                    Some(plan),
                    ledger,
                    slot,
                    floor,
                    checked_required,
                    growth,
                    refund,
                    budget,
                )
            }) {
                Ok(value)
            } else {
                // Output must die before its caller observes the refusal. A
                // denied concrete custody check never authorizes a later refund.
                drop(value);
                if let Some(root) = root {
                    root.deny_active_root_refund();
                }
                Err(ArgumentResourceV1::Accounting.into())
            }
        }
        Ok(Err(error)) => {
            let refund = budget.storage().checked_sub(floor);
            scoped_emission_refund_v29(
                Some(plan),
                ledger,
                slot,
                floor,
                required,
                growth,
                refund,
                budget,
            );
            Err(error)
        }
        Err(payload) => {
            let refund = budget.storage().checked_sub(floor);
            scoped_emission_refund_v29(
                Some(plan),
                ledger,
                slot,
                floor,
                required,
                growth,
                refund,
                budget,
            );
            std::panic::resume_unwind(payload)
        }
    }
}

#[cfg(test)]
pub(super) fn test_ordinary_activation_census_v29(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    original.query(budget)?;
    let owner = original.source.root_row(root)?;
    let pending = owner
        .source_slots
        .pending_memory
        .as_ref()
        .expect("ordinary private accesses require a real completed source census");
    assert!(!pending.accesses.is_empty());
    assert!(
        !pending.lifetimes.is_empty(),
        "helper invocation lifetimes are retained"
    );
    assert_eq!(pending.initial.len(), owner.source_slots.slots.len());
    for access in &pending.accesses {
        assert!(!access.alternatives.is_empty());
        for alternative in &pending.alternatives[access.alternatives.clone()] {
            let slot = &owner.source_slots.slots[alternative.slot];
            assert_eq!(alternative.slot, access.physical.slot);
            assert_eq!(alternative.instance, slot.instance);
            assert_eq!(alternative.local.index(), slot.legacy_local().unwrap());
            assert!(alternative.formation.is_none());
        }
    }
    Ok(pending.accesses.len())
}

// Scalar-element arrays use the existing indexed history at immutable replay.
// Descriptor effects are classified from their original source separately;
// the physical origin equations must still prove they cannot alias these slots.
fn ordinary_scalar_memory_profile_v29(
    _plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    for slot in &slots.slots {
        budget.charge_work(1)?;
        if !ordinary_scalar_memory_representation_v29(slot.representation)
            && !ordinary_original_array_representation_v29(slot.origin.source, slot.representation)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn ordinary_scalar_memory_representation_v29(representation: ScopedSlotRepresentationV29) -> bool {
    matches!(representation, ScopedSlotRepresentationV29::ScalarArray(scalar)
        if scalar.length == 1 && scalar.bytes == scalar.element.size)
}

fn ordinary_original_array_representation_v29(
    source: ScopedAllocationSourceV29,
    representation: ScopedSlotRepresentationV29,
) -> bool {
    matches!((source, representation), (
        ScopedAllocationSourceV29::OriginalArray { .. }, ScopedSlotRepresentationV29::ScalarArray(scalar))
        if scalar.length != 0 && scalar.element.size != 0
            && matches!(scalar.element.element, PrivateRetainedElementFactsV1::Scalar(_))
            && scalar.length.checked_mul(scalar.element.size) == Some(scalar.bytes))
}

fn source_array_geometry_v29(
    slots: &[ScopedSourceSlotV29],
    selected: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ArgumentResourceV1> {
    budget.charge_work(slots.len())?;
    Ok(selected
        || slots.iter().any(|slot| {
            matches!(slot.representation,
        ScopedSlotRepresentationV29::ScalarArray(scalar) if scalar.length != 1)
        }))
}

#[cfg(test)]
#[test]
fn ordinary_scalar_profile_does_not_admit_object_or_multi_element_geometry() {
    // Profile classification only, not a source-allocation or currentness proof.
    let scalar = ScopedScalarArraySlotV29 {
        element_type: SemanticTypeIdV1::from_index(0),
        element: PrivateRetainedSlotFactsV1 {
            element: PrivateRetainedElementFactsV1::Scalar(ScalarType::U32),
            size: 4,
            alignment: 4,
        },
        length: 1,
        bytes: 4,
        count: None,
    };
    assert!(ordinary_scalar_memory_representation_v29(
        ScopedSlotRepresentationV29::ScalarArray(scalar)
    ));
    assert!(!ordinary_scalar_memory_representation_v29(
        ScopedSlotRepresentationV29::ScalarArray(ScopedScalarArraySlotV29 {
            length: 4,
            bytes: 16,
            ..scalar
        })
    ));
    assert!(!ordinary_scalar_memory_representation_v29(
        ScopedSlotRepresentationV29::ScalarArray(ScopedScalarArraySlotV29 { bytes: 8, ..scalar })
    ));
    assert!(!ordinary_scalar_memory_representation_v29(
        ScopedSlotRepresentationV29::Object {
            schema: fe2o3_kernel_ir::StorageLayoutIdV1(0),
            bytes: 4,
            alignment: 4,
        }
    ));
}

fn check_expanded_source_memory_inner_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    pending: &PendingScopedRootEmissionV29,
    slots: &OwnedScopedSourceSlotsV29,
    parts: Option<&ScopedDeferredScalarViewV29<'_, '_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingSourceMemoryV29, ProductionSemanticKirErrorV1> {
    let original = original_census(instances, Some(references.plan), budget)?;
    if (!original.requires_physical()
        && !ordinary_scalar_memory_profile_v29(references.plan, slots, budget)?)
        || slots.pending_memory.is_some()
    {
        return Err(source_raw_physical_error_v29());
    }
    let source_index = SourceAddressSourceIndexV29::new(instances, pending, budget)?;
    // Validate every object payload even when no physical value access is
    // retained. Diagnostic reads are joined to their own history stream below.
    for sidecar in &pending.sidecars.rows {
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?;
        for row in &anchors.rows {
            budget.charge_work(1)?;
            source_address_object_payload_v29(anchors, row, budget)?;
        }
    }
    let (sources, issued) =
        source_address_accesses_retained_v29(instances, references, &source_index, slots, budget)?;
    let index_failures =
        source_index_failures_v29(instances, references.plan, &source_index, slots, budget)?;
    check_source_object_effect_census_v29(
        instances,
        references.plan,
        &source_index,
        slots,
        &sources,
        &index_failures,
        budget,
    )?;
    let mut accesses = emission_vec_v1(sources.len(), budget)?;
    budget.charge_work(sources.len())?;
    accesses.extend(sources.iter().map(|row| row.physical));
    budget.charge_work(slots.slots.len())?;
    let graph = if slots.slots.iter().any(|slot| {
        matches!(
            slot.representation,
            ScopedSlotRepresentationV29::Object { .. }
        )
    }) {
        let layouts = references
            .plan
            .storage_root
            .as_ref()
            .ok_or_else(source_raw_physical_error_v29)?
            .source_layouts(instances, budget)?;
        let rows = layouts.rows(instances.owner(), budget)?;
        SourceAddressMemoryV29::prepare_with_layouts(
            &pending.function,
            &slots.slots,
            parts,
            &accesses,
            &rows,
            budget,
        )?
    } else {
        SourceAddressMemoryV29::prepare(&pending.function, &slots.slots, parts, &accesses, budget)?
    };
    let boundaries =
        source_address_boundaries_v29(instances, &source_index, slots, &graph, budget)?;
    let lifetimes = source_address_lifetimes_v29(
        instances,
        references.plan,
        &source_index,
        slots,
        &graph,
        &boundaries,
        budget,
    )?;
    let indices =
        source_index_recipes_v29(instances, references, &source_index, slots, &graph, budget)?;
    let index_guards = if original.retained_indices == 0 {
        Vec::new()
    } else {
        source_index_guards_v29(instances, &source_index, slots, &graph, budget)?
    };
    budget.charge_work(indices.len())?;
    if indices
        .iter()
        .filter(|row| row.load_anchor.is_some())
        .count()
        != original.retained_indices
    {
        return Err(source_raw_physical_error_v29());
    }
    let indexed = source_array_geometry_v29(&slots.slots, !indices.is_empty(), budget)?;
    let (graph, geometry) = if !indexed {
        (
            graph.solve(&slots.slots, &accesses, &lifetimes.kills, budget)?,
            SourceAddressGeometryV29::Scalar,
        )
    } else {
        let pending =
            graph.solve_pending_indices(&slots.slots, &accesses, &lifetimes.kills, budget)?;
        (pending.graph, SourceAddressGeometryV29::PendingIndices)
    };
    let births = check_source_address_formations_v29(
        instances,
        references,
        pending,
        slots,
        &graph,
        &source_index,
        budget,
    )?;
    #[cfg(test)]
    let payload_work_before = budget.work();
    let payload_index = SourceObjectPayloadIndexV29::new(instances, &source_index, budget)?;
    let projects = check_source_static_object_projects_v29(
        instances,
        references.plan,
        &source_index,
        slots,
        &graph,
        &payload_index,
        budget,
    )?;
    #[cfg(test)]
    let payload_floor = budget.storage();
    // This unit query only borrows the retained graph, index and source rows.
    // Its completed query frames must end before currentness and pending rows.
    source_object_activation_scratch_v29(instances, references.plan, budget, |budget| {
        #[cfg(test)]
        let query_floor = budget.storage();
        #[cfg(test)]
        let row_reclaimed_before = SOURCE_OBJECT_PAYLOAD_ROW_SCRATCH_V29.get().1;
        check_source_address_payloads_v29(
            instances,
            references,
            slots,
            &source_index,
            &graph,
            &sources,
            &payload_index,
            budget,
        )?;
        #[cfg(test)]
        {
            let (calls, bytes) = SOURCE_OBJECT_PAYLOAD_QUERY_SCRATCH_V29.get();
            SOURCE_OBJECT_PAYLOAD_QUERY_SCRATCH_V29.set((
                calls.checked_add(1).unwrap(),
                bytes
                    .checked_add(budget.storage().checked_sub(query_floor).unwrap())
                    .unwrap()
                    .checked_add(
                        SOURCE_OBJECT_PAYLOAD_ROW_SCRATCH_V29
                            .get()
                            .1
                            .checked_sub(row_reclaimed_before)
                            .unwrap(),
                    )
                    .unwrap(),
            ));
        }
        Ok(())
    })?;
    #[cfg(test)]
    assert_eq!(budget.storage(), payload_floor);
    payload_index.discard(budget)?;
    #[cfg(test)]
    {
        let (calls, work) = SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29.get();
        SOURCE_OBJECT_PAYLOAD_PASS_WORK_V29.set((
            calls.checked_add(1).unwrap(),
            work.checked_add(budget.work() - payload_work_before)
                .unwrap(),
        ));
    }
    check_source_address_currentness_geometry_v29(
        &pending.function,
        &graph,
        &slots.slots,
        &accesses,
        &lifetimes.kills,
        &lifetimes.initial,
        &lifetimes.lifetimes,
        &births,
        &index_failures,
        geometry,
        budget,
    )?;
    if !indexed {
        scoped_slot_uses_v29::check_expanded_scalar_addresses_with_failures_v29(
            &pending.function,
            &graph,
            &slots.slots,
            &accesses,
            &lifetimes.kills,
            &index_failures,
            budget,
        )?;
    }
    let retained = retain_pending_memory_v29(
        instances,
        references.plan,
        issued,
        &sources,
        &boundaries,
        slots,
        lifetimes,
        births,
        projects,
        &graph,
        indices,
        index_failures,
        index_guards,
        budget,
    )?;
    // No retained row points into scratch or the archive. The outer lexical
    // owner drops this scratch before its genuine growth-aware settlement.
    drop((graph, boundaries, accesses, sources));
    source_index.discard(budget)?;
    Ok(retained)
}

include!("production_source_direct_object_activation_v29.rs");
include!("production_source_safe_object_activation_v29.rs");
include!("production_source_object_activation_scratch_v29.rs");
include!("production_source_access_query_scratch_v29.rs");
include!("production_source_pending_alternative_capacity_v29.rs");

fn retain_pending_memory_v29(
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    issued: PendingSourceIssuedRolesV29,
    sources: &[SourceAddressAccessSourceV29],
    boundaries: &[SourceAddressBoundaryV29],
    slots: &OwnedScopedSourceSlotsV29,
    lifetimes: SourceAddressLifetimesV29,
    births: Vec<SourceAddressBirthV29>,
    projects: Vec<PendingSourceObjectProjectV29>,
    graph: &SourceAddressMemoryV29<'_>,
    indices: Vec<PendingSourceIndexV29>,
    index_failures: Vec<SourceIndexFailureV29>,
    index_guards: Vec<PendingSourceIndexGuardV29>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingSourceMemoryV29, ProductionSemanticKirErrorV1> {
    budget.charge_work(indices.len())?;
    let ordinary_indices = indices.iter().any(|row| row.load_anchor.is_some())
        || !original_census(instances, Some(plan), budget)?.requires_physical();
    // This is only a may-roster of original activation boundaries for each
    // exact object. It never proves that any one site reaches an access. The
    // immutable all-path lifetime equations remain mandatory for every row.
    let mut ordinary_activations = emission_vec_v1(
        if ordinary_indices {
            argument_sum_v1(&[slots.slots.len(), boundaries.len()])?
        } else {
            0
        },
        budget,
    )?;
    if ordinary_indices {
        // `initial` describes the expanded root entry, where helper locals are
        // not live yet. Their invocation activation is the checked call
        // preheader in `lifetimes`, not a source StorageLive statement.
        for (slot, original) in slots.slots.iter().enumerate() {
            budget.charge_work(2)?;
            if instances.instance_reachable(original.instance) != Some(true) {
                return Err(source_raw_physical_error_v29());
            }
            ordinary_activations.push((slot, SourceMemoryActivationV29::Invocation));
        }
        for row in boundaries {
            budget.charge_work(3)?;
            if row.cause != ScopedMemoryKillV29::StorageLive {
                continue;
            }
            let (block, statement) = scoped_memory_site_key_v29(row.frame.site);
            let statement = statement.ok_or_else(source_raw_physical_error_v29)?;
            if slots
                .slots
                .get(row.slot)
                .is_none_or(|slot| slot.instance != row.instance)
            {
                return Err(source_raw_physical_error_v29());
            }
            ordinary_activations.push((
                row.slot,
                SourceMemoryActivationV29::StorageLive {
                    block: SemanticBlockIdV1::from_index(block),
                    statement: statement as usize,
                },
            ));
        }
        call_splice_sort_work_v1(ordinary_activations.len(), budget)
            .map_err(source_address_call_error_v29)?;
        ordinary_activations.sort_unstable_by_key(|(slot, activation)| match activation {
            SourceMemoryActivationV29::Invocation => (*slot, 0, 0, 0),
            SourceMemoryActivationV29::StorageLive { block, statement } => {
                (*slot, 1, block.index(), *statement)
            }
        });
        budget.charge_work(ordinary_activations.len())?;
        ordinary_activations.dedup();
    }
    let mut offsets = emission_vec_v1(instances.instances().len(), budget)?;
    let mut limits = emission_vec_v1(instances.instances().len(), budget)?;
    for ordinal in 0..instances.instances().len() {
        budget.charge_work(2)?;
        let id = instances
            .id_at(ordinal)
            .ok_or_else(source_raw_physical_error_v29)?;
        let function = instances
            .instance(id)
            .ok_or_else(source_raw_physical_error_v29)?
            .declaration();
        let mut rows = emission_vec_v1(function.blocks().len(), budget)?;
        let mut next = 1;
        for block in function.blocks() {
            budget.charge_work(1)?;
            rows.push(next);
            next = argument_sum_v1(&[next, block.statements().len()])?;
        }
        if next >= u32::MAX as usize {
            return Err(ArgumentResourceV1::Arithmetic.into());
        }
        offsets.push(rows);
        limits.push(next as u32);
    }
    budget.reserve_storage(pending_alternative_capacity_headers_v29()?)?;
    let alternative_capacity = pending_alternative_capacity_v29(
        plan,
        sources,
        slots,
        &limits,
        &ordinary_activations,
        ordinary_indices,
        budget,
    )?;
    let mut output = PendingSourceMemoryV29 {
        source: ExecutionCallSourceV29::from_instances(instances, budget)?,
        issued,
        accesses: emission_vec_v1(sources.len(), budget)?,
        projects,
        alternatives: emission_vec_v1(alternative_capacity, budget)?,
        effects: Vec::new(),
        initial: lifetimes.initial,
        lifetimes: lifetimes.lifetimes,
        kills: lifetimes.kills,
        births,
        indices,
        index_failures,
        index_guards,
        retained_storage: 0,
    };
    // The copied loop result outlives the closed query scratch. Actual retained
    // alternatives still pay their complete actual vector capacity above.
    source_reference_emission_prepay_v29::<Option<PendingSourceMemoryAlternativeV29>>(budget)?;
    source_reference_emission_prepay_v29::<PendingSourceMemoryAlternativeV29>(budget)?;
    for source in sources {
        budget.charge_work(4)?;
        let first = output.alternatives.len();
        let mut object_alternative = None;
        if source.raw.is_none() && (source.direct_object.is_some() || source.safe_object.is_some())
        {
            source_object_activation_scratch_v29(instances, plan, budget, |budget| {
                if source.direct_object.is_some() {
                    object_alternative = source_direct_object_invocation_v29(
                        instances, plan, slots, source, budget,
                    )?;
                    #[cfg(test)]
                    if object_alternative.is_some() {
                        test_direct_object_invocation_v29(instances, plan, slots, source, budget)?;
                    }
                }
                if object_alternative.is_none() && source.safe_object.is_some() {
                    object_alternative =
                        source_safe_object_invocation_v29(instances, plan, slots, source, budget)?;
                    #[cfg(test)]
                    if object_alternative.is_some() {
                        test_safe_object_invocation_v29(instances, plan, slots, source, budget)?;
                    }
                }
                Ok(())
            })?;
        }
        if let Some(access) = source.raw {
            let set = plan
                .raw_sets
                .get(access.set)
                .ok_or_else(source_raw_physical_error_v29)?;
            for choice in plan
                .raw_choices
                .get(set.first..argument_sum_v1(&[set.first, set.count])?)
                .ok_or_else(source_raw_physical_error_v29)?
            {
                budget.charge_work(2)?;
                let origin = plan
                    .raw_origins
                    .get(choice.origin)
                    .ok_or_else(source_raw_physical_error_v29)?;
                if choice.expired {
                    return Err(source_raw_physical_error_v29());
                }
                let limit = *limits
                    .get(origin.instance.index())
                    .ok_or_else(source_raw_physical_error_v29)?;
                let singleton = [origin.generation];
                let atoms = if origin.generation < limit {
                    singleton.as_slice()
                } else {
                    let set = plan
                        .epoch_sets
                        .get((origin.generation - limit) as usize)
                        .ok_or_else(source_raw_physical_error_v29)?;
                    if set.instance != origin.instance || set.local != origin.local || set.count < 2
                    {
                        return Err(source_raw_physical_error_v29());
                    }
                    plan.epoch_members
                        .get(set.first..argument_sum_v1(&[set.first, set.count])?)
                        .ok_or_else(source_raw_physical_error_v29)?
                };
                for &atom in atoms {
                    budget.charge_work(5)?;
                    let activation = if atom == 0 {
                        SourceMemoryActivationV29::Invocation
                    } else {
                        if atom >= limit {
                            return Err(source_raw_physical_error_v29());
                        }
                        let blocks = &offsets[origin.instance.index()];
                        budget.charge_work(call_splice_search_work_v1(blocks.len()))?;
                        let block = blocks
                            .partition_point(|offset| *offset <= atom as usize)
                            .checked_sub(1)
                            .ok_or_else(source_raw_physical_error_v29)?;
                        let statement = atom as usize - blocks[block];
                        let block_id = SemanticBlockIdV1::from_index(
                            u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        );
                        let function = instances
                            .instance(origin.instance)
                            .ok_or_else(source_raw_physical_error_v29)?
                            .declaration();
                        if instances.block_reachable(origin.instance, block_id) != Some(true)
                            || !matches!(function.blocks().get(block).and_then(|row| row.statements().get(statement)).map(|row| row.kind()), Some(SemanticStatementKindV1::StorageLive(local)) if *local == origin.local)
                        {
                            return Err(source_raw_physical_error_v29());
                        }
                        SourceMemoryActivationV29::StorageLive {
                            block: block_id,
                            statement,
                        }
                    };
                    let original_slot = match slots
                        .slots
                        .get(source.physical.slot)
                        .map(|row| row.representation)
                    {
                        Some(ScopedSlotRepresentationV29::Object { .. }) => {
                            source_address_object_slot_v29(
                                instances,
                                plan,
                                slots,
                                origin.instance,
                                origin.local,
                                origin.generation,
                                origin.ty,
                                budget,
                            )?
                        }
                        _ => source_address_original_slot_v29(
                            instances,
                            slots,
                            origin.instance,
                            origin.local,
                            origin.ty,
                            budget,
                        )?,
                    };
                    if original_slot != source.physical.slot {
                        return Err(source_raw_physical_error_v29());
                    }
                    push_pending_alternative_v29(
                        &mut output.alternatives,
                        PendingSourceMemoryAlternativeV29 {
                            instance: origin.instance,
                            local: origin.local,
                            slot: original_slot,
                            activation,
                            formation: Some(origin.site),
                        },
                        alternative_capacity,
                        budget,
                    )?;
                }
            }
        } else if let Some(alternative) = object_alternative {
            push_pending_alternative_v29(
                &mut output.alternatives,
                alternative,
                alternative_capacity,
                budget,
            )?;
        } else if ordinary_indices
            && matches!(
                slots
                    .slots
                    .get(source.physical.slot)
                    .ok_or_else(source_raw_physical_error_v29)?
                    .representation,
                ScopedSlotRepresentationV29::ScalarArray(_)
            )
        {
            // A retained scalar index does not supply activation alternatives
            // for unrelated typed objects. Their direct accesses remain pending.
            let slot = slots
                .slots
                .get(source.physical.slot)
                .ok_or_else(source_raw_physical_error_v29)?;
            budget.charge_work(argument_product_v1(
                2,
                call_splice_search_work_v1(ordinary_activations.len()),
            )?)?;
            let first = ordinary_activations.partition_point(|row| row.0 < source.physical.slot);
            let end = ordinary_activations.partition_point(|row| row.0 <= source.physical.slot);
            if first == end {
                return Err(source_raw_physical_error_v29());
            }
            for (_, activation) in &ordinary_activations[first..end] {
                budget.charge_work(2)?;
                push_pending_alternative_v29(
                    &mut output.alternatives,
                    PendingSourceMemoryAlternativeV29 {
                        instance: slot.instance,
                        local: SemanticLocalIdV1::from_index(slot.legacy_local()?),
                        slot: source.physical.slot,
                        activation: *activation,
                        formation: None,
                    },
                    alternative_capacity,
                    budget,
                )?;
            }
        }
        // A direct activation may-set is still pending until the complete
        // immutable range/history/currentness census. Empty is never proof.
        let actual = graph.blocks[graph.block(source.physical.block, budget)?]
            .1
            .operations
            .get(source.physical.operation)
            .ok_or_else(source_raw_physical_error_v29)?;
        let value =
            source_address_value_access_v29(actual)?.ok_or_else(source_raw_physical_error_v29)?;
        let object_location = if value.object {
            Some(graph.object_location(value.pointer, budget)?)
        } else {
            None
        };
        output.accesses.push(PendingSourceMemoryAccessV29 {
            instance: source.instance,
            anchor: source.anchor,
            physical: source.physical,
            object_location,
            safe_object: source.safe_object,
            alternatives: first..output.alternatives.len(),
        });
    }
    for origin in &plan.raw_origins {
        budget.charge_work(1)?;
        emission_push_v1(
            &mut output.effects,
            PendingSourceMemoryEffectV29::Formation(origin.site),
            budget,
        )?;
    }
    for boundary in boundaries {
        budget.charge_work(1)?;
        emission_push_v1(
            &mut output.effects,
            PendingSourceMemoryEffectV29::Boundary {
                instance: boundary.instance,
                anchor: boundary.anchor,
            },
            budget,
        )?;
    }
    for failure in &output.index_failures {
        budget.charge_work(1)?;
        emission_push_v1(
            &mut output.effects,
            PendingSourceMemoryEffectV29::FailureRead {
                instance: failure.instance,
                anchor: failure.anchor,
            },
            budget,
        )?;
    }
    for ordinal in 0..instances.instances().len() {
        budget.charge_work(1)?;
        let instance = instances
            .id_at(ordinal)
            .ok_or_else(source_raw_physical_error_v29)?;
        if instances.instance_reachable(instance) != Some(true) {
            continue;
        }
        emission_push_v1(
            &mut output.effects,
            PendingSourceMemoryEffectV29::Invocation(instance),
            budget,
        )?;
        for exit in instances
            .returns(instance)
            .ok_or_else(source_raw_physical_error_v29)?
        {
            budget.charge_work(1)?;
            if instances.block_reachable(instance, exit.block) == Some(true) {
                emission_push_v1(
                    &mut output.effects,
                    PendingSourceMemoryEffectV29::Return {
                        instance,
                        block: exit.block,
                    },
                    budget,
                )?;
            }
        }
    }
    #[cfg(test)]
    PENDING_ALTERNATIVE_CAPACITY_V29.set((
        alternative_capacity,
        output.alternatives.capacity(),
        output.alternatives.len(),
    ));
    output.retained_storage = argument_sum_v1(&[
        output.issued.retained_storage()?,
        argument_product_v1(
            output.projects.capacity(),
            std::mem::size_of::<PendingSourceObjectProjectV29>(),
        )?,
        argument_product_v1(
            output.accesses.capacity(),
            std::mem::size_of::<PendingSourceMemoryAccessV29>(),
        )?,
        argument_product_v1(
            output.alternatives.capacity(),
            std::mem::size_of::<PendingSourceMemoryAlternativeV29>(),
        )?,
        argument_product_v1(
            output.effects.capacity(),
            std::mem::size_of::<PendingSourceMemoryEffectV29>(),
        )?,
        argument_product_v1(output.initial.capacity(), std::mem::size_of::<bool>())?,
        argument_product_v1(
            output.lifetimes.capacity(),
            std::mem::size_of::<SourceAddressLifetimeV29>(),
        )?,
        argument_product_v1(
            output.kills.capacity(),
            std::mem::size_of::<SourceAddressKillV29>(),
        )?,
        argument_product_v1(
            output.births.capacity(),
            std::mem::size_of::<SourceAddressBirthV29>(),
        )?,
        argument_product_v1(
            output.indices.capacity(),
            std::mem::size_of::<PendingSourceIndexV29>(),
        )?,
        argument_product_v1(
            output.index_failures.capacity(),
            std::mem::size_of::<SourceIndexFailureV29>(),
        )?,
        argument_product_v1(
            output.index_guards.capacity(),
            std::mem::size_of::<PendingSourceIndexGuardV29>(),
        )?,
    ])?;
    let mut scratch = argument_sum_v1(&[
        argument_product_v1(
            ordinary_activations.capacity(),
            std::mem::size_of::<(usize, SourceMemoryActivationV29)>(),
        )?,
        argument_product_v1(offsets.capacity(), std::mem::size_of::<Vec<usize>>())?,
        argument_product_v1(limits.capacity(), std::mem::size_of::<u32>())?,
    ])?;
    for rows in &offsets {
        budget.charge_work(1)?;
        scratch = argument_sum_v1(&[
            scratch,
            argument_product_v1(rows.capacity(), std::mem::size_of::<usize>())?,
        ])?;
    }
    drop((offsets, limits, ordinary_activations));
    budget.release_storage(scratch)?;
    Ok(output)
}

#[derive(Clone, Copy)]
struct OriginalRawCensusV29 {
    raw_types: usize,
    formations: usize,
    retained_indices: usize,
    typed_objects: usize,
}

impl OriginalRawCensusV29 {
    fn requires_physical(self) -> bool {
        self.raw_types != 0
            || self.formations != 0
            || self.retained_indices != 0
            || self.typed_objects != 0
    }
}

fn original_census(
    instances: &ExecutionInstancesV29<'_>,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<OriginalRawCensusV29, ProductionSemanticKirErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    if let Some(plan) = references {
        budget.source_reference_owner_v29(plan)?;
        if !std::ptr::eq(plan.instances, instances) {
            return Err(source_raw_physical_error_v29());
        }
    }
    let slot = budget
        .prepared_input_slot_v1()
        .ok_or(ArgumentResourceV1::Accounting)?;
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let types = instances.owner().source_semantic().types();
    let owned = argument_sum_v1(&[
        argument_product_v1(types.len(), std::mem::size_of::<bool>())?,
        argument_product_v1(types.len(), std::mem::size_of::<SemanticTypeIdV1>())?,
        std::mem::size_of::<(Vec<bool>, Vec<SemanticTypeIdV1>)>(),
        std::mem::size_of::<Result<OriginalRawCensusV29, ProductionSemanticKirErrorV1>>(),
    ])?;
    let required = argument_sum_v1(&[floor, owned])?;
    budget.reserve_storage(owned)?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut seen = argument_vec_v1::<bool>(types.len())?;
        let mut pending = argument_vec_v1::<SemanticTypeIdV1>(types.len())?;
        budget.charge_work(types.len())?;
        seen.resize(types.len(), false);
        let push = |ty: SemanticTypeIdV1,
                    seen: &mut [bool],
                    pending: &mut Vec<SemanticTypeIdV1>,
                    budget: &mut dyn SemanticEmissionBudgetV1|
         -> Result<(), ProductionSemanticKirErrorV1> {
            budget.charge_work(3)?;
            let present = seen
                .get_mut(ty.index() as usize)
                .ok_or_else(source_raw_physical_error_v29)?;
            if !*present {
                if pending.len() == pending.capacity() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                *present = true;
                pending.push(ty);
            }
            Ok(())
        };
        let mut census = OriginalRawCensusV29 {
            raw_types: 0,
            formations: 0,
            retained_indices: 0,
            typed_objects: 0,
        };
        for index in 0..instances.instances().len() {
            budget.charge_work(2)?;
            let instance = instances
                .id_at(index)
                .ok_or_else(source_raw_physical_error_v29)?;
            match instances.instance_reachable(instance) {
                Some(false) => continue,
                Some(true) => {}
                None => return Err(source_raw_physical_error_v29()),
            }
            let function = instances
                .instance(instance)
                .ok_or_else(source_raw_physical_error_v29)?
                .declaration();
            let occurrences = instances
                .occurrences(instance)
                .ok_or_else(source_raw_physical_error_v29)?;
            for event in occurrences.events() {
                budget.charge_work(3)?;
                if !event.is_reachable()
                    || event.is_promoted()
                    || !matches!(event.role(), ExecutionEventV29::ProjectionIndexUse(_))
                {
                    continue;
                }
                let block =
                    SemanticBlockIdV1::from_index(scoped_memory_site_key_v29(event.site()).0);
                match instances.block_reachable(instance, block) {
                    Some(false) => continue,
                    Some(true) => {}
                    None => return Err(source_raw_physical_error_v29()),
                }
                if event.resolved().is_some() {
                    return Err(source_raw_physical_error_v29());
                }
                census.retained_indices = argument_sum_v1(&[census.retained_indices, 1])?;
            }
            for local in function.locals() {
                push(local.ty(), &mut seen, &mut pending, budget)?;
            }
            for (block, source) in function.blocks().iter().enumerate() {
                budget.charge_work(2)?;
                let block = SemanticBlockIdV1::from_index(
                    u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                );
                match instances.block_reachable(instance, block) {
                    Some(false) => continue,
                    Some(true) => {}
                    None => return Err(source_raw_physical_error_v29()),
                }
                for statement in source.statements() {
                    budget.charge_work(1)?;
                    if matches!(statement.kind(), SemanticStatementKindV1::Assign(assign)
                        if matches!(assign.value().kind(), SemanticRvalueKindV1::AddressOf { .. }))
                    {
                        census.formations = argument_sum_v1(&[census.formations, 1])?;
                    }
                }
            }
        }
        // Original type IDs bound this traversal, including pointer cycles and
        // homogeneous arrays. No byte extent or array length is expanded.
        while let Some(ty) = pending.pop() {
            budget.charge_work(2)?;
            let declaration = types
                .get(ty.index() as usize)
                .ok_or_else(source_raw_physical_error_v29)?;
            if matches!(
                declaration.rust_type_kind(),
                fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Execution(_)
            ) {
                continue;
            }
            match declaration.shape() {
                SemanticTypeShapeV1::Pointer(pointer) => {
                    if pointer.kind() == SemanticPointerKindV1::Raw {
                        census.raw_types = argument_sum_v1(&[census.raw_types, 1])?;
                    } else {
                        push(pointer.pointee(), &mut seen, &mut pending, budget)?;
                    }
                }
                SemanticTypeShapeV1::Tuple(fields)
                | SemanticTypeShapeV1::Aggregate(fields)
                | SemanticTypeShapeV1::Union(fields) => {
                    for &field in fields.fields() {
                        push(field, &mut seen, &mut pending, budget)?;
                    }
                }
                SemanticTypeShapeV1::Array { element, .. }
                | SemanticTypeShapeV1::Slice { element } => {
                    push(*element, &mut seen, &mut pending, budget)?
                }
                SemanticTypeShapeV1::Enum { variants, .. } => {
                    for variant in variants {
                        budget.charge_work(1)?;
                        for &field in variant.fields().fields() {
                            push(field, &mut seen, &mut pending, budget)?;
                        }
                    }
                }
                SemanticTypeShapeV1::Unit
                | SemanticTypeShapeV1::Never
                | SemanticTypeShapeV1::Scalar(_)
                | SemanticTypeShapeV1::ValidityScalar(_)
                | SemanticTypeShapeV1::FunctionPointer { .. }
                | SemanticTypeShapeV1::Opaque => {}
            }
        }
        if let Some(plan) = references.filter(|plan| !plan.cells.rows.is_empty()) {
            census.typed_objects = budget.source_physical_object_count_v29(plan)?;
        }
        drop(pending);
        drop(seen);
        Ok(census)
    }));
    // Only this closed, fixed scratch allocation is released. No callback or
    // domain mutation runs above; all success/error/unwind backing is now gone.
    let allowed = budget
        .storage()
        .checked_sub(owned)
        .is_some_and(|after| after >= floor)
        && budget.permits_prepared_input_refund_v1(references, slot, ledger, required, owned);
    let cleanup = if allowed {
        budget.release_storage(owned)
    } else {
        if let Some(root) = references.and_then(|plan| plan.storage_root.as_ref()) {
            root.deny_active_root_refund();
        }
        Err(ArgumentResourceV1::Accounting.into())
    };
    match result {
        Ok(Ok(census)) => {
            cleanup?;
            Ok(census)
        }
        Ok(Err(error)) => {
            let _ = cleanup;
            Err(error)
        }
        Err(payload) => {
            let _ = cleanup;
            resume_unwind(payload)
        }
    }
}

pub(super) fn require_original_zero_raw_v29(
    instances: &ExecutionInstancesV29<'_>,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if original_census(instances, references, budget)?.requires_physical() {
        return Err(source_reference_error_v29(
            "original raw source requires consuming expanded physical admission",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod header_tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn physical_rejected_envelopes_and_four_payload_attempts_are_prepaid() {
        type Payload = Box<dyn std::any::Any + Send>;
        type Rejected = Result<[u64; 5], [u64; 7]>;
        let expected = size_of::<Rejected>()
            + size_of::<std::panic::AssertUnwindSafe<Rejected>>()
            + size_of::<[Option<Payload>; 2]>()
            + size_of::<std::panic::AssertUnwindSafe<[Option<Payload>; 2]>>()
            + 2 * size_of::<Payload>()
            + size_of::<std::panic::AssertUnwindSafe<Payload>>()
            + 2 * size_of::<Result<(), Payload>>()
            + size_of::<std::ops::Range<usize>>()
            + size_of::<usize>()
            + size_of::<bool>();
        assert_eq!(
            physical_discard_headers_v29::<[u64; 5], [u64; 7]>().unwrap(),
            expected
        );
        assert_eq!(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29, 5);
    }

    #[allow(dead_code)]
    enum Kind {
        Ordinary,
        ExpandedRaw,
    }
    #[allow(dead_code)]
    struct Transaction<'a, 'source> {
        instances: &'a ExecutionInstancesV29<'source>,
        references: Option<SourceReferenceEmissionV29<'a, 'source>>,
        limits: ProductionSemanticKirLimitsV1,
        kind: Option<Kind>,
    }
    #[allow(dead_code)]
    struct Preparation<'a, 'root, 'source> {
        owner: &'a Transaction<'root, 'source>,
    }
    #[allow(dead_code)]
    struct Unfinished {
        source: ExecutionCallSourceV29,
        emission_owner: Option<usize>,
        kind: Kind,
        pending: PendingScopedRootEmissionV29,
        payload: PrivateArrayPayloadV1,
        slots: OwnedScopedSourceSlotsV29,
    }
    #[allow(dead_code)]
    struct RawAssembly {
        pending: PendingScopedRootEmissionV29,
    }

    #[test]
    fn original_root_header_uses_independent_owned_envelopes() {
        assert_eq!(
            size_of::<Transaction<'_, '_>>(),
            size_of::<SourceRootTransactionV29<'_, '_>>()
        );
        assert_eq!(
            size_of::<Preparation<'_, '_, '_>>(),
            size_of::<SourceRootPreparationV29<'_, '_, '_>>()
        );
        assert_eq!(
            size_of::<Unfinished>(),
            size_of::<UnfinishedSourceRootV29>()
        );
        assert_eq!(size_of::<RawAssembly>(), size_of::<PendingRawAssemblyV29>());
        type Output = (
            PendingScopedRootEmissionV29,
            PrivateArrayPayloadV1,
            OwnedScopedSourceSlotsV29,
        );
        type Selected<'a> = Result<Output, source_storage_v29::RecordedStorageFailureV29<'a>>;
        let expected = size_of::<Transaction<'_, '_>>()
            + size_of::<Preparation<'_, '_, '_>>()
            + size_of::<Unfinished>()
            + size_of::<RawAssembly>()
            + size_of::<Result<RawAssembly, ProductionSemanticKirErrorV1>>()
            + size_of::<Result<Unfinished, ProductionSemanticKirErrorV1>>()
            + size_of::<Selected<'_>>()
            + size_of::<std::thread::Result<Selected<'_>>>()
            + size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>();
        assert_eq!(source_root_header_v29().unwrap(), expected);
    }
}

#[cfg(test)]
mod literal_array_history_tests {
    use super::*;
    include!("production_source_literal_array_history_v29_tests.rs");
}
