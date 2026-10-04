// Representation recipes from the existing original capability resolver.
// Neither a Plain source node nor an observed lowered binding creates a recipe.
#[derive(Clone, Debug)]
struct ExecutionCfgCarrierV29 {
    source_type: SemanticTypeIdV1,
    transport_type: SemanticTypeIdV1,
    binding: SemanticPromotedBindingV1,
    kernel_types: Box<[Type]>,
}

#[derive(Clone, Default)]
struct ExecutionCfgCarriersV29 {
    source_owner: Option<usize>,
    source_plan: Option<usize>,
    instance: Option<ProductionCallInstanceIdV1>,
    ledger: Option<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>,
    locals: Vec<(u32, ExecutionCfgCarrierV29)>,
}

fn execution_cfg_carrier_index_headers_v29() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            std::mem::size_of::<T>(),
            std::mem::size_of::<Result<T, ArgumentResourceV1>>(),
            std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
        ])
    }
    type Row = (u32, ExecutionCfgCarrierV29);
    type Lookup<'a> = (
        &'a ExecutionCfgCarriersV29,
        u32,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type Append<'a> = (
        &'a mut ExecutionCfgCarriersV29,
        u32,
        ExecutionCfgCarrierV29,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type Push<'a> = (&'a mut Vec<Row>, Row, &'a mut dyn SemanticEmissionBudgetV1);
    type Reserve<'a> = (usize, &'a mut dyn SemanticEmissionBudgetV1);
    argument_sum_v1(&[
        h::<Lookup<'_>>()?,
        h::<Append<'_>>()?,
        h::<Push<'_>>()?,
        h::<Reserve<'_>>()?,
        h::<Vec<Row>>()?,
        h::<&mut Vec<Row>>()?,
        h::<&Vec<Row>>()?,
        h::<&[Row]>()?,
        h::<&Row>()?,
        h::<Option<&Row>>()?,
        h::<&ExecutionCfgCarriersV29>()?,
        h::<Row>()?,
        h::<ExecutionCfgCarrierV29>()?,
        h::<&ExecutionCfgCarrierV29>()?,
        h::<Option<&ExecutionCfgCarrierV29>>()?,
        h::<Result<usize, usize>>()?,
        h::<Option<usize>>()?,
        h::<u32>()?,
        h::<&u32>()?,
        h::<usize>()?,
        h::<bool>()?,
        h::<()>()?,
    ])
}

impl std::fmt::Debug for ExecutionCfgCarriersV29 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ExecutionCfgCarriersV29")
            .field("source_owner", &self.source_owner)
            .field("source_plan", &self.source_plan)
            .field("instance", &self.instance)
            .field("ledger_present", &self.ledger.is_some())
            .field("locals", &self.locals)
            .finish()
    }
}

fn execution_cfg_plain_carrier_node_v29(row: &SourceReferenceNodeV29) -> bool {
    matches!(row.kind, SourceReferenceNodeKindV29::Plain(None))
        && row.value_origin.is_none()
        && row.storage.is_none()
        && row.inactive.is_none()
        && row.descriptor.is_none()
}

impl ExecutionCfgCarriersV29 {
    // The producer walks original local IDs once in increasing order. Keeping
    // that order explicit avoids retained per-insertion BTreeMap split credit.
    fn append(
        &mut self,
        local: u32,
        carrier: ExecutionCfgCarrierV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(2)?;
        if self
            .locals
            .last()
            .is_some_and(|(previous, _)| *previous >= local)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        emission_push_v1(&mut self.locals, (local, carrier), budget)
    }

    fn lookup(
        &self,
        local: u32,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<&ExecutionCfgCarrierV29>, ProductionSemanticKirErrorV1> {
        // The existing conservative logarithmic debit covers the standard
        // library binary search, including the final equal-key comparison.
        charge_execution_cfg_lookup_v29(self.locals.len(), budget)?;
        Ok(self
            .locals
            .binary_search_by_key(&local, |(key, _)| *key)
            .ok()
            .map(|index| &self.locals[index].1))
    }

    fn check_owner(
        &self,
        cursor: &ExecutionAvailabilityV29<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        cursor.check_ledger(budget)?;
        budget.charge_work(4)?;
        if self.source_owner != Some(std::ptr::from_ref(cursor.function).addr())
            || self.source_plan
                != cursor
                    .references
                    .map(|references| std::ptr::from_ref(references.plan).addr())
            || self.instance != Some(cursor.instance)
            || self.ledger != Some(budget.work_ledger_identity_v1())
        {
            return Err(execution_cfg_error_v29());
        }
        Ok(())
    }

    fn archived_use<'a>(
        &'a self,
        cursor: &ExecutionAvailabilityV29<'_>,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        definition: SsaValueV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<&'a ExecutionCfgCarrierV29>, ProductionSemanticKirErrorV1> {
        cursor.check_ledger(budget)?;
        let Some(carrier) = self.lookup(place.local().index(), budget)? else {
            return Ok(None);
        };
        self.check_owner(cursor, budget)?;
        let references = cursor.references.ok_or_else(execution_cfg_error_v29)?;
        references.check(budget)?;
        budget.source_reference_charge_v29(references.plan, 8)?;
        let local = place.local().index() as usize;
        if cursor.cfg.nominal_locals.get(local) != Some(&0)
            || (cursor.cfg.reference_locals.get(local) != Some(&true)
                && !matches!(
                    carrier.binding,
                    SemanticPromotedBindingV1::IndexWitness { .. }
                ))
            || cursor.function.locals().get(local).map(|local| local.ty())
                != Some(carrier.source_type)
            || matches!(carrier.binding, SemanticPromotedBindingV1::Ordinary)
        {
            return Err(execution_cfg_error_v29());
        }
        // A recipe authenticates representation only. The exact original Use
        // and SSA definition are checked independently, including same-block uses.
        cursor.check_claimed_original_use_v29(site, role, place, definition, budget)?;
        Ok(Some(carrier))
    }

    fn at<'a>(
        &'a self,
        cursor: &ExecutionAvailabilityV29<'_>,
        local: u32,
        node: usize,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<&'a ExecutionCfgCarrierV29>, ProductionSemanticKirErrorV1> {
        cursor.check_ledger(budget)?;
        let Some(carrier) = self.lookup(local, budget)? else {
            return Ok(None);
        };
        self.check_owner(cursor, budget)?;
        let references = cursor.references.ok_or_else(execution_cfg_error_v29)?;
        references.check(budget)?;
        budget.source_reference_charge_v29(references.plan, 8)?;
        let row = references
            .plan
            .nodes
            .get(node)
            .ok_or_else(execution_cfg_error_v29)?;
        if cursor.cfg.nominal_locals.get(local as usize) != Some(&0)
            || cursor.cfg.reference_locals.get(local as usize) != Some(&true)
            || !execution_cfg_plain_carrier_node_v29(row)
            || row.ty != carrier.source_type
            || cursor
                .function
                .locals()
                .get(local as usize)
                .map(|local| local.ty())
                != Some(row.ty)
            || matches!(carrier.binding, SemanticPromotedBindingV1::Ordinary)
        {
            return Err(execution_cfg_error_v29());
        }
        Ok(Some(carrier))
    }
}

impl ExecutionCfgCarrierV29 {
    fn types(
        &self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
        let mut allocation = CompilerCarrierAllocationV29::paid(budget)?;
        let mut values = allocation.reserve(self.kernel_types.len())?;
        for ty in &self.kernel_types {
            values.push(allocation.clone_type(ty)?);
        }
        Ok(values)
    }

    fn rebuild(
        &self,
        types: &[SemanticTypeDeclV1],
        values: &[ValueDef],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        self.binding.binding_from_transport_with_allocation_v29(
            types,
            self.transport_type,
            values,
            &mut CompilerCarrierAllocationV29::paid_with_representation(
                budget,
                ExecutionCfgRepresentationV29::OriginalSource,
            )?,
        )
    }
}

struct ExecutionCfgCarrierValuesV29<'a, 'b> {
    values: &'a mut Vec<ValueDef>,
    nodes: usize,
    budget: &'b mut dyn SemanticEmissionBudgetV1,
}

impl SemanticTransportVisitorV1 for ExecutionCfgCarrierValuesV29<'_, '_> {
    type Error = ProductionSemanticKirErrorV1;
    fn node(&mut self) -> Result<(), Self::Error> {
        execution_cfg_charge_node_v29(&mut self.nodes, self.budget)
    }
    fn component(
        &mut self,
        value: ValueId,
        ty: BorrowedTransportTypeV1<'_>,
    ) -> Result<(), Self::Error> {
        let ty = match ty {
            BorrowedTransportTypeV1::Existing(ty) => execution_cfg_clone_type_v29(ty, self.budget)?,
            BorrowedTransportTypeV1::Pointer {
                pointee,
                address_space,
                access,
            } => {
                let pointee = execution_cfg_clone_type_v29(pointee, self.budget)?;
                self.budget.reserve_storage(std::mem::size_of::<Type>())?;
                Type::pointer(pointee, address_space, access)
            }
        };
        emission_push_v1(self.values, ValueDef::new(value, ty), self.budget)
    }
    fn invalid(_: &'static str) -> Self::Error {
        execution_cfg_error_v29()
    }
    fn equal_types(&mut self, left: &Type, right: &Type) -> Result<bool, Self::Error> {
        invocation_equal_types_v1(left, right, self.budget)
    }
}

fn execution_cfg_carrier_values_v29(
    carrier: &ExecutionCfgCarrierV29,
    binding: &SemanticValueBindingV1,
    values: &mut Vec<ValueDef>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    carrier.binding.visit_transport_values(
        binding,
        &mut ExecutionCfgCarrierValuesV29 {
            values,
            nodes: 0,
            budget,
        },
    )?;
    budget.charge_work(values.len() + 1)?;
    if values.len() != carrier.kernel_types.len() {
        return Err(execution_cfg_error_v29());
    }
    for (value, ty) in values.iter().zip(&carrier.kernel_types) {
        if !invocation_equal_types_v1(&value.ty, ty, budget)? {
            return Err(execution_cfg_error_v29());
        }
    }
    Ok(())
}

fn with_execution_cfg_carrier_values_v29<R>(
    carrier: &ExecutionCfgCarrierV29,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
    consume: impl FnOnce(
        &[ValueDef],
        &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    let floor = budget.storage();
    let returned_headers = argument_sum_v1(&[
        std::mem::size_of::<R>(),
        std::mem::size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
    ])?;
    // The two catch closures contain only these borrowed words and the captured
    // consumer. Alignment slack is prepaid instead of relying on closure layout.
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<Vec<ValueDef>>(),
        std::mem::size_of::<ExecutionCfgCarrierValuesV29<'_, '_>>(),
        std::mem::size_of_val(&consume),
        argument_product_v1(2, std::mem::align_of_val(&consume))?,
        argument_product_v1(12, std::mem::size_of::<usize>())?,
        std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<Result<R, ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>(),
        std::mem::size_of::<std::thread::Result<Result<R, ProductionSemanticKirErrorV1>>>(),
        std::mem::size_of::<R>(),
    ])?)?;
    let mut values = Vec::new();
    let built = catch_unwind(AssertUnwindSafe(|| {
        execution_cfg_carrier_values_v29(carrier, binding, &mut values, budget)
    }));
    let storage = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let result = match built {
        Ok(Ok(())) => catch_unwind(AssertUnwindSafe(|| consume(&values, budget))),
        Ok(Err(error)) => Ok(Err(error)),
        Err(payload) => Err(payload),
    };
    drop(values);
    match result {
        Ok(result) => {
            // The output and its result envelope transfer to the caller. Their
            // credits remain in the surrounding function-owner lease.
            budget.release_storage(
                storage
                    .checked_sub(returned_headers)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            result
        }
        Err(payload) => {
            let _ = budget.release_storage(storage);
            resume_unwind(payload)
        }
    }
}

fn merge_execution_cfg_carrier_v29(
    carrier: &ExecutionCfgCarrierV29,
    held: &SemanticValueBindingV1,
    archived: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_execution_cfg_carrier_values_v29(carrier, held, budget, |held, budget| {
        with_execution_cfg_carrier_values_v29(carrier, archived, budget, |archived, budget| {
            budget.charge_work(held.len() + 1)?;
            if held.len() != archived.len() {
                return Err(execution_cfg_error_v29());
            }
            for (held, archived) in held.iter().zip(archived) {
                if held.id != archived.id
                    || !invocation_equal_types_v1(&held.ty, &archived.ty, budget)?
                {
                    return Err(execution_cfg_error_v29());
                }
            }
            Ok(())
        })
    })
}

#[allow(clippy::too_many_arguments)]
fn check_source_use_archive_v29(
    cursor: &ExecutionAvailabilityV29<'_>,
    carriers: &ExecutionCfgCarriersV29,
    locals: &[Option<SemanticValueBindingV1>],
    archive: &SemanticSsaBindingsV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    place: &SemanticPlaceV1,
    definition: SsaValueV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if let Some(carrier) = carriers.archived_use(cursor, site, role, place, definition, budget)? {
        charge_execution_cfg_lookup_v29(archive.len(), budget)?;
        let held = locals
            .get(place.local().index() as usize)
            .and_then(Option::as_ref)
            .ok_or_else(execution_cfg_error_v29)?;
        let original = archive
            .get(&definition)
            .ok_or_else(execution_cfg_error_v29)?;
        // Compare the complete holder before the independent place resolver
        // applies Some-edge dominance, projection, borrow, or storage checks.
        merge_execution_cfg_carrier_v29(carrier, held, original, budget)
    } else {
        budget.charge_work(1)?;
        let held = locals
            .get(place.local().index() as usize)
            .and_then(Option::as_ref)
            .ok_or_else(execution_cfg_error_v29)?;
        charge_execution_cfg_lookup_v29(archive.len(), budget)?;
        let original = archive
            .get(&definition)
            .ok_or_else(execution_cfg_error_v29)?;
        if cursor
            .cfg
            .nominal_locals
            .get(place.local().index() as usize)
            == Some(&0)
        {
            if execution_archive_needs_carrier_v29(held, &mut 0, budget)?
                || execution_archive_needs_carrier_v29(original, &mut 0, budget)?
            {
                // Observed shape can require a recipe in the non-nominal
                // carrier profile, never create one. Mixed nominal holders
                // retain their existing separate archive checks.
                return Err(execution_cfg_error_v29());
            }
        }
        if !place.projections().is_empty() {
            cursor.check_claimed_original_use_v29(site, role, place, definition, budget)?;
            if let Some(same) = execution_archive_pointer_holder_same_v29(
                held,
                original,
                place.projections(),
                budget,
            )? {
                if !same {
                    return Err(execution_availability_error_v29());
                }
                // This proves only the current holder's identity against this
                // instance's archive. The caller must still resolve the original
                // place and discharge its borrow, lifetime, and memory permission.
                return Ok(());
            }
        }
        check_execution_archive_v29(locals, archive, place, definition, budget)
    }
}

fn execution_archive_pointer_holder_same_v29(
    mut held: &SemanticValueBindingV1,
    mut archived: &SemanticValueBindingV1,
    projections: &[SemanticProjectionV1],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<bool>, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    for projection in projections {
        budget.charge_work(1)?;
        match (projection.kind(), held, archived) {
            (
                SemanticProjectionKindV1::Field(index),
                SemanticValueBindingV1::Aggregate(left),
                SemanticValueBindingV1::Aggregate(right),
            ) => {
                let (Some(left), Some(right)) =
                    (left.get(index as usize), right.get(index as usize))
                else {
                    return Ok(None);
                };
                held = left;
                archived = right;
            }
            (
                SemanticProjectionKindV1::Dereference,
                SemanticValueBindingV1::Value {
                    id: left_id,
                    ty: left @ Type::Pointer(left_pointer),
                },
                SemanticValueBindingV1::Value {
                    id: right_id,
                    ty: right @ Type::Pointer(right_pointer),
                },
            ) => {
                // Retained object holders keep their existing closed profile;
                // this route cannot waive its schema/address-space checks.
                if matches!(*left_pointer.pointee, Type::StorageObject(_))
                    || matches!(*right_pointer.pointee, Type::StorageObject(_))
                {
                    return Ok(None);
                }
                return Ok(Some(
                    left_id == right_id && invocation_equal_types_v1(left, right, budget)?,
                ));
            }
            (
                SemanticProjectionKindV1::Dereference,
                SemanticValueBindingV1::Value {
                    id: left_id,
                    ty: left @ Type::Slice(left_slice),
                },
                SemanticValueBindingV1::Value {
                    id: right_id,
                    ty: right @ Type::Slice(right_slice),
                },
            ) => {
                if matches!(*left_slice.element, Type::StorageObject(_))
                    || matches!(*right_slice.element, Type::StorageObject(_))
                {
                    return Ok(None);
                }
                // A Slice ValueId includes both pointer and length. This is
                // only holder identity; the original descriptor still owns
                // origin, permissions and every projected bounds obligation.
                return Ok(Some(
                    left_id == right_id && invocation_equal_types_v1(left, right, budget)?,
                ));
            }
            _ => return Ok(None),
        }
    }
    Ok(None)
}

fn execution_archive_needs_carrier_v29(
    binding: &SemanticValueBindingV1,
    nodes: &mut usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    use SemanticValueBindingV1 as B;
    execution_cfg_charge_node_v29(nodes, budget)?;
    match binding {
        B::Aggregate(fields) => {
            for field in fields {
                if execution_archive_needs_carrier_v29(field, nodes, budget)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        B::Enum { payloads, .. } => {
            for fields in payloads.values() {
                budget.charge_work(1)?;
                for field in fields {
                    if execution_archive_needs_carrier_v29(field, nodes, budget)? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
        B::MathContext
        | B::CollectiveContext
        | B::WorkgroupLdsScope
        | B::MatrixContext
        | B::DynamicLds { .. }
        | B::WaveLane { .. }
        | B::MatrixFragment { .. }
        | B::AccumulatorFragment { .. }
        | B::Gfx950LdsTransposeTile { .. }
        | B::WorkgroupPipeline { .. }
        | B::OptionPointer { .. }
        | B::IndexWitness { .. }
        | B::OptionIndexWitness { .. }
        | B::GridLeader { .. }
        | B::ComponentWitness { .. }
        | B::OptionComponentWitness { .. }
        | B::OptionGridLeader { .. } => Ok(true),
        B::Unit
        | B::Unmaterialized
        | B::Execution(_)
        | B::ExecutionBorrow(_)
        | B::ExecutionReferent(_)
        | B::MovedExecution
        | B::SourceReference(_)
        | B::SourceEnumTag(_)
        | B::SourceInactive(_)
        | B::Value { .. } => Ok(false),
    }
}

fn with_execution_cfg_local_values_v29<R>(
    cursor: &ExecutionAvailabilityV29<'_>,
    carriers: &ExecutionCfgCarriersV29,
    block: SemanticBlockIdV1,
    local: u32,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
    consume: impl FnOnce(
        &[ValueDef],
        &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    cursor.check_ledger(budget)?;
    if cursor.cfg.reference_locals.get(local as usize) == Some(&true) {
        let references = cursor.references.ok_or_else(execution_cfg_error_v29)?;
        let node = references
            .block_node(
                cursor.instance,
                block,
                SemanticLocalIdV1::from_index(local),
                budget,
            )?
            .ok_or_else(execution_cfg_error_v29)?;
        if cursor.cfg.source_enum_locals.get(local as usize) == Some(&true) {
            return with_source_enum_transport_tag_v55(references, node, binding, budget, consume);
        }
        if let Some(carrier) = carriers.at(cursor, local, node, budget)? {
            return with_execution_cfg_carrier_values_v29(carrier, binding, budget, consume);
        }
    }
    with_execution_cfg_values_and_references_v29(binding, cursor.references, budget, consume)
}
