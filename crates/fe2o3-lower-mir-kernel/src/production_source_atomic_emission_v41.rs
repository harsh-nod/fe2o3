// Private physical consumers of original V41 custody. Generic/RW describes
// representation only; the source planner still rejects every ordinary deref.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceAtomicPhysicalReceiptV41 {
    source: usize,
    base: ValueId,
    result: ValueId,
    block: BlockId,
    first: usize,
    end: usize,
}

fn source_atomic_custody_type_v41(
    plan: &SourceReferencePlanV29<'_, '_>,
    custody: usize,
    ty: SemanticTypeIdV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    source_reference_owned_prepay_v29::<Type>(plan, budget)?;
    for (node, row) in plan.nodes.iter().enumerate() {
        budget.source_reference_charge_v29(plan, 2)?;
        if row.atomic_custody == Some(custody) && row.ty == ty {
            return source_atomic_node_type_v41(plan, node, budget)?
                .ok_or_else(source_atomic_view_error_v41);
        }
    }
    Err(source_atomic_view_error_v41())
}

impl SourceReferenceEmissionV29<'_, '_> {
    fn claim_atomic_physical_v41(
        &self,
        index: usize,
        receipt: SourceAtomicPhysicalReceiptV41,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        budget.source_reference_charge_v29(self.plan, 5)?;
        let slot = self
            .atomic_receipts
            .get(index)
            .ok_or(ArgumentResourceV1::Accounting)?;
        // Re-emission of a source occurrence is not an alternative authority.
        if slot.get().is_some() || receipt.first > receipt.end {
            return Err(source_atomic_view_error_v41());
        }
        slot.set(Some(receipt));
        Ok(())
    }

    fn atomic_use_v41(
        &self,
        site: SourceReferenceSiteV29,
        atomic: &SemanticAtomicRmwV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<(usize, SourceAtomicUseV41)>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        source_reference_owned_prepay_v29::<Option<(usize, SourceAtomicUseV41)>>(
            self.plan, budget,
        )?;
        let declaration = self
            .plan
            .instances
            .instance(site.instance)
            .ok_or(ArgumentResourceV1::Accounting)?
            .declaration();
        let original = site.statement.and_then(|s| {
            declaration
                .blocks()
                .get(site.block.index() as usize)
                .and_then(|b| b.statements().get(s))
        });
        if !matches!(original.map(|row| row.kind()),
            Some(SemanticStatementKindV1::AtomicRmw(value)) if std::ptr::eq(value, atomic))
        {
            return Err(source_atomic_view_error_v41());
        }
        for (index, row) in self.plan.atomic_uses.iter().enumerate() {
            budget.source_reference_charge_v29(self.plan, 9)?;
            if row.site != site {
                continue;
            }
            if row.source != atomic as *const SemanticAtomicRmwV1 as usize
                || row.address != atomic.address() as *const SemanticPlaceV1 as usize
                || row.value != atomic.value() as *const SemanticOperandV1 as usize
                || row.destination != atomic.destination() as *const SemanticPlaceV1 as usize
                || row.operation != atomic.operation()
                || row.access != atomic.access()
                || self
                    .plan
                    .nodes
                    .get(row.node)
                    .and_then(|node| node.atomic_custody)
                    != Some(row.custody)
            {
                return Err(source_atomic_view_error_v41());
            }
            source_atomic_node_type_v41(self.plan, row.node, budget)?
                .ok_or_else(source_atomic_view_error_v41)?;
            return Ok(Some((index, *row)));
        }
        Ok(None)
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn try_lower_atomic_capture_v41(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        projection: usize,
        binding: &SemanticValueBindingV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some((references, instance)) = self.execution.as_ref().and_then(|cursor| {
            cursor
                .references
                .map(|references| (references, cursor.instance))
        }) else {
            return Ok(None);
        };
        let site = SourceReferenceSiteV29 {
            instance,
            block,
            statement: statement.map(|s| s as usize),
        };
        let selected =
            self.with_emission_budget_v1(|_, budget| {
                references.check(budget)?;
                source_reference_owned_prepay_v29::<
                    Option<(usize, SourceAtomicCaptureV41, Type, Type)>,
                >(references.plan, budget)?;
                // Only the original final field read can capture the global root.
                if projection.checked_add(1) != Some(place.projections().len()) {
                    return Ok(None);
                }
                for (index, row) in references.plan.atomic_captures.iter().enumerate() {
                    budget.source_reference_charge_v29(references.plan, 8)?;
                    if row.site != site || row.source != place as *const SemanticPlaceV1 as usize {
                        continue;
                    }
                    let fact = references
                        .plan
                        .atomic_custody
                        .get(row.custody)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    let field = place
                        .projections()
                        .get(projection)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    if field.kind() != SemanticProjectionKindV1::Field(fact.field)
                        || field.result_type() != references.plan.nodes[row.node].ty
                        || fact.view.is_some()
                    {
                        return Err(source_atomic_view_error_v41());
                    }
                    let source = source_atomic_root_type_v41(references.plan, fact.parent, budget)?
                        .ok_or_else(source_atomic_view_error_v41)?;
                    let target = source_atomic_node_type_v41(references.plan, row.node, budget)?
                        .ok_or_else(source_atomic_view_error_v41)?;
                    return Ok(Some((index, *row, source, target)));
                }
                Ok(None)
            })?;
        let Some((index, row, expected, target)) = selected else {
            return Ok(None);
        };
        let SemanticValueBindingV1::Value { id: base, ty } = binding else {
            return Err(source_atomic_view_error_v41());
        };
        if *ty != expected {
            return Err(source_atomic_view_error_v41());
        }
        let base = *base;
        let first = operations.len();
        let result_type = self
            .with_emission_budget_v1(|_, budget| emission_binding_clone_type_v1(&target, budget))?;
        let result = self.emit(
            operations,
            result_type,
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value: base,
                to: target,
            },
        )?;
        let SemanticValueBindingV1::Value { id, .. } = &result else {
            return Err(source_atomic_view_error_v41());
        };
        let result_id = *id;
        let physical_block = self.kernel_block_id_v1(block)?;
        self.with_emission_budget_v1(|_, budget| {
            source_reference_owned_prepay_v29::<SourceAtomicPhysicalReceiptV41>(
                references.plan,
                budget,
            )?;
            references.claim_atomic_physical_v41(
                index,
                SourceAtomicPhysicalReceiptV41 {
                    source: row.source,
                    base,
                    result: result_id,
                    block: physical_block,
                    first,
                    end: operations.len(),
                },
                budget,
            )
        })?;
        Ok(Some(result))
    }

    fn try_lower_atomic_formation_v41(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some((references, instance)) = self.execution.as_ref().and_then(|cursor| {
            cursor
                .references
                .map(|references| (references, cursor.instance))
        }) else {
            return Ok(None);
        };
        let selected = self.with_emission_budget_v1(|this, budget| {
            references.check(budget)?;
            let site = execution_site_v29(block, statement);
            let Some(SemanticStatementKindV1::Assign(assign)) =
                scoped_source_statement_v29(this.function, site)
            else {
                return Ok(None);
            };
            if !std::ptr::eq(assign.value().kind(), value)
                || assign.value().result_type() != result_type
            {
                return Err(source_atomic_view_error_v41());
            }
            let Some(row) = references.plan.atomic_view_formation_v41(
                instance,
                site,
                assign.value(),
                budget,
            )?
            else {
                return Ok(None);
            };
            source_reference_owned_prepay_v29::<
                Option<(usize, SourceAtomicViewFormationV41, Type, Type)>,
            >(references.plan, budget)?;
            let mut selected = None;
            for (index, candidate) in references.plan.atomic_formations.iter().enumerate() {
                budget.source_reference_charge_v29(references.plan, 1)?;
                if *candidate == row {
                    if selected.replace(index).is_some() {
                        return Err(source_atomic_view_error_v41());
                    }
                }
            }
            let index = selected.ok_or(ArgumentResourceV1::Accounting)?;
            let input =
                source_atomic_custody_type_v41(references.plan, row.input, row.input_type, budget)?;
            let output = source_atomic_custody_type_v41(
                references.plan,
                row.output,
                row.output_type,
                budget,
            )?;
            if !invocation_equal_types_v1(&input, &output, budget)? {
                return Err(source_atomic_view_error_v41());
            }
            Ok(Some((index, row, input, output)))
        })?;
        let Some((index, row, input_type, output_type)) = selected else {
            return Ok(None);
        };
        let first = operations.len();
        let binding = match value {
            SemanticRvalueKindV1::Cast { operand, .. } => {
                self.lower_rvalue_operand_v29(block, statement, 0, operand, operations)?
            }
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            }
            | SemanticRvalueKindV1::AddressOf { place, .. } => {
                self.use_source_place_v29(block, statement, place)?;
                let local = self.require_local(block, statement, place.local().index())?;
                self.with_emission_budget_v1(|this, budget| {
                    let binding = this.locals[local]
                        .as_ref()
                        .ok_or_else(source_atomic_view_error_v41)?;
                    clone_execution_cfg_binding_v29(binding, &mut 0, budget)
                })?
            }
            _ => {
                return Err(source_atomic_view_error_v41());
            }
        };
        let SemanticValueBindingV1::Value { id, ty } = binding else {
            return Err(source_atomic_view_error_v41());
        };
        if ty != input_type {
            return Err(source_atomic_view_error_v41());
        }
        let physical_block = self.kernel_block_id_v1(block)?;
        self.with_emission_budget_v1(|_, budget| {
            source_reference_owned_prepay_v29::<SourceAtomicPhysicalReceiptV41>(
                references.plan,
                budget,
            )?;
            let slot = argument_sum_v1(&[references.plan.atomic_captures.len(), index])?;
            references.claim_atomic_physical_v41(
                slot,
                SourceAtomicPhysicalReceiptV41 {
                    source: row.source,
                    base: id,
                    result: id,
                    block: physical_block,
                    first,
                    end: operations.len(),
                },
                budget,
            )
        })?;
        Ok(Some(SemanticValueBindingV1::Value {
            id,
            ty: output_type,
        }))
    }

    fn try_lower_atomic_address_v41(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        atomic: &SemanticAtomicRmwV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some((references, instance)) = self.execution.as_ref().and_then(|cursor| {
            cursor
                .references
                .map(|references| (references, cursor.instance))
        }) else {
            return Ok(None);
        };
        let site = SourceReferenceSiteV29 {
            instance,
            block,
            statement: statement.map(|s| s as usize),
        };
        let selected = self
            .with_emission_budget_v1(|_, budget| references.atomic_use_v41(site, atomic, budget))?;
        let Some((_, row)) = selected else {
            return Ok(None);
        };
        self.use_source_place_with_role_v29(
            block,
            statement,
            ExecutionOperandV29::AtomicAddress,
            atomic.address(),
        )?;
        let local = self.require_local(block, statement, atomic.address().local().index())?;
        self.with_emission_budget_v1(|this, budget| {
            let expected = source_atomic_node_type_v41(references.plan, row.node, budget)?
                .ok_or_else(source_atomic_view_error_v41)?;
            let binding = this.locals[local]
                .as_ref()
                .ok_or_else(source_atomic_view_error_v41)?;
            let SemanticValueBindingV1::Value { ty, .. } = binding else {
                return Err(source_atomic_view_error_v41());
            };
            if !invocation_equal_types_v1(ty, &expected, budget)? {
                return Err(source_atomic_view_error_v41());
            }
            clone_execution_cfg_binding_v29(binding, &mut 0, budget).map(Some)
        })
    }

    fn claim_atomic_operation_v41(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        atomic: &SemanticAtomicRmwV1,
        pointer: ValueId,
        value: ValueId,
        access: MemoryAccess,
        result: &SemanticValueBindingV1,
        operations: &[Operation],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some((references, instance)) = self.execution.as_ref().and_then(|cursor| {
            cursor
                .references
                .map(|references| (references, cursor.instance))
        }) else {
            return Ok(());
        };
        let site = SourceReferenceSiteV29 {
            instance,
            block,
            statement: statement.map(|s| s as usize),
        };
        let physical_block = self.kernel_block_id_v1(block)?;
        self.with_emission_budget_v1(|_, budget| {
            let Some((index, row)) = references.atomic_use_v41(site, atomic, budget)? else {
                return Ok(());
            };
            budget.source_reference_charge_v29(references.plan, 12)?;
            let SemanticValueBindingV1::Value { id, ty } = result else {
                return Err(source_atomic_view_error_v41());
            };
            let operation = operations.last().ok_or_else(source_atomic_view_error_v41)?;
            check_atomic_operation_v41(atomic, operation, pointer, value, *id, ty, access, budget)?;
            source_reference_owned_prepay_v29::<SourceAtomicPhysicalReceiptV41>(
                references.plan,
                budget,
            )?;
            let slot = argument_sum_v1(&[
                references.plan.atomic_captures.len(),
                references.plan.atomic_formations.len(),
                index,
            ])?;
            references.claim_atomic_physical_v41(
                slot,
                SourceAtomicPhysicalReceiptV41 {
                    source: row.source,
                    base: pointer,
                    result: *id,
                    block: physical_block,
                    first: operations
                        .len()
                        .checked_sub(1)
                        .ok_or(ArgumentResourceV1::Accounting)?,
                    end: operations.len(),
                },
                budget,
            )
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn check_atomic_operation_v41(
    atomic: &SemanticAtomicRmwV1,
    operation: &Operation,
    pointer: ValueId,
    value: ValueId,
    result: ValueId,
    ty: &Type,
    access: MemoryAccess,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(14)?;
    let OperationKind::Atomic(emitted) = &operation.kind else {
        return Err(source_atomic_view_error_v41());
    };
    let [output] = operation.results.as_slice() else {
        return Err(source_atomic_view_error_v41());
    };
    if output.id != result
        || output.ty != *ty
        || emitted.pointer != pointer
        || emitted.kind
            != lower_atomic_rmw_kind(
                atomic.operation(),
                ty.as_scalar().ok_or_else(source_atomic_view_error_v41)?,
            )
            .ok_or_else(source_atomic_view_error_v41)?
        || emitted.scope
            != lower_atomic_scope(atomic.access().scope())
                .ok_or_else(source_atomic_view_error_v41)?
        || emitted.ordering != lower_atomic_ordering(atomic.access().ordering())
        || emitted.value != Some(value)
        || emitted.access != access
        || emitted.compare.is_some()
        || emitted.failure_ordering.is_some()
    {
        return Err(source_atomic_view_error_v41());
    }
    Ok(())
}
