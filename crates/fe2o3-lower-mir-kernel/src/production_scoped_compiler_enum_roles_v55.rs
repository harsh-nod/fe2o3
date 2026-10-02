// Compiler spill roles are locators into an original enum value and the exact
// emitted access. They are not source object origins or reference authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedCompilerEnumRoleV55 {
    Store {
        site: ExecutionSiteV29,
        source: Option<ScopedMemoryStoreSourceV29>,
        value: ValueId,
    },
    Load {
        block: SemanticBlockIdV1,
        result: ValueId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedCompilerEnumAccessV55 {
    anchor: usize,
    local: SemanticLocalIdV1,
    variant: u32,
    field: u32,
    component: u32,
    pointer: ValueId,
    role: ScopedCompilerEnumRoleV55,
}

fn scoped_compiler_enum_error_v55() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("compiler enum access lacks its exact retained payload role")
}

struct CheckedScopedCompilerEnumAccessV55<'a> {
    record: &'a ScopedCompilerEnumAccessV55,
    anchor: &'a ScopedMemoryAnchorV29,
    spill: &'a ExecutionEnumSpillV48,
    binding: &'a SemanticValueBindingV1,
}

fn scoped_compiler_enum_key_v55(row: &ExecutionEnumSpillV48) -> (u32, u32, u32, usize) {
    (row.local, row.variant, row.field, row.component)
}

// This rejoins original-source and compiler-storage identities independently.
// The physical consumer still checks every pointer use and the actual Alloca.
fn check_scoped_compiler_enum_access_v55<'a>(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    anchors: &'a ScopedMemoryAnchorsV29,
    archive: &'a ExecutionArchiveV29,
    record: &'a ScopedCompilerEnumAccessV55,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<CheckedScopedCompilerEnumAccessV55<'a>, ProductionSemanticKirErrorV1> {
    archive.check_original_v29(instances, instance, budget)?;
    budget.charge_work(13)?;
    if anchors.subject != archive.subject {
        return Err(scoped_compiler_enum_error_v55());
    }
    charge_execution_cfg_lookup_v29(anchors.compiler_enum.len(), budget)?;
    let role = anchors
        .compiler_enum
        .binary_search_by_key(&record.anchor, |row| row.anchor)
        .map_err(|_| scoped_compiler_enum_error_v55())?;
    if anchors.compiler_enum[role] != *record
        || role
            .checked_sub(1)
            .and_then(|i| anchors.compiler_enum.get(i))
            .is_some_and(|row| row.anchor >= record.anchor)
        || anchors
            .compiler_enum
            .get(role + 1)
            .is_some_and(|row| row.anchor <= record.anchor)
    {
        return Err(scoped_compiler_enum_error_v55());
    }
    let anchor = anchors
        .rows
        .get(record.anchor)
        .ok_or_else(scoped_compiler_enum_error_v55)?;
    if anchor.kind
        != (ScopedMemoryAnchorKindV29::Access {
            pointer: record.pointer,
            payload: None,
        })
    {
        return Err(scoped_compiler_enum_error_v55());
    }
    charge_execution_cfg_lookup_v29(archive.enum_spills.len(), budget)?;
    let key = (
        record.local.index(),
        record.variant,
        record.field,
        record.component as usize,
    );
    let ordinal = archive
        .enum_spills
        .binary_search_by_key(&key, scoped_compiler_enum_key_v55)
        .map_err(|_| scoped_compiler_enum_error_v55())?;
    let spill = &archive.enum_spills[ordinal];
    if spill.pointer != record.pointer
        || ordinal
            .checked_sub(1)
            .and_then(|i| archive.enum_spills.get(i))
            .is_some_and(|row| scoped_compiler_enum_key_v55(row) >= key)
        || archive
            .enum_spills
            .get(ordinal + 1)
            .is_some_and(|row| scoped_compiler_enum_key_v55(row) <= key)
    {
        return Err(scoped_compiler_enum_error_v55());
    }
    let function = instances
        .instance(instance)
        .ok_or_else(scoped_compiler_enum_error_v55)?
        .declaration();
    let types = instances.owner().source_semantic().types();
    let source_type = function
        .locals()
        .get(record.local.index() as usize)
        .ok_or_else(scoped_compiler_enum_error_v55)?
        .ty();
    let SemanticTypeShapeV1::Enum { variants, .. } = types
        .get(source_type.index() as usize)
        .ok_or_else(scoped_compiler_enum_error_v55)?
        .shape()
    else {
        return Err(scoped_compiler_enum_error_v55());
    };
    let field_type = variants
        .get(record.variant as usize)
        .and_then(|row| row.fields().fields().get(record.field as usize))
        .ok_or_else(scoped_compiler_enum_error_v55)?;
    if source_type != spill.source_type || *field_type != spill.field_type {
        return Err(scoped_compiler_enum_error_v55());
    }
    let ScopedCompilerEnumRoleV55::Store { site, value, .. } = record.role else {
        // Restoration needs an exact original block-entry value relation.
        // A matching private allocation alone cannot establish that relation.
        return Err(scoped_compiler_enum_error_v55());
    };
    let OperationKind::Store {
        pointer,
        value: actual,
        access,
    } = operation.kind
    else {
        return Err(scoped_compiler_enum_error_v55());
    };
    if pointer != record.pointer
        || actual != value
        || access != MemoryAccess::new(AddressSpace::Private, spill.alignment)
        || !operation.results.is_empty()
    {
        return Err(scoped_compiler_enum_error_v55());
    }
    let binding = match site {
        ExecutionSiteV29::Statement { .. } => {
            let Some(SemanticStatementKindV1::Assign(assignment)) =
                scoped_source_statement_v29(function, site)
            else {
                return Err(scoped_compiler_enum_error_v55());
            };
            if assignment.destination().local() != record.local
                || !assignment.destination().projections().is_empty()
                || assignment.destination().ty() != source_type
                || anchor.source
                    != Some(ScopedMemoryFrameV29::operand(
                        site,
                        Some(ExecutionOperandV29::Destination),
                    ))
            {
                return Err(scoped_compiler_enum_error_v55());
            }
            archive.lookup_rvalue_original_v30(instances, instance, site, source_type, budget)?
        }
        ExecutionSiteV29::Terminator { .. } => {
            let destination = scoped_source_call_destination_v29(function, site)
                .ok_or_else(scoped_compiler_enum_error_v55)?;
            if destination.local() != record.local
                || destination.ty() != source_type
                || !destination.projections().is_empty()
                || anchor.source
                    != Some(ScopedMemoryFrameV29 {
                        site,
                        role: Some(ScopedMemoryRoleV29::CallResult),
                    })
            {
                return Err(scoped_compiler_enum_error_v55());
            }
            let occurrences = instances
                .occurrences(instance)
                .ok_or_else(scoped_compiler_enum_error_v55)?;
            let mut definition = None;
            for event in occurrences.events() {
                budget.charge_work(5)?;
                if event.site() != site
                    || event.role() != ExecutionEventV29::DestinationDefine
                    || event.event().variable().get() != record.local.index()
                {
                    continue;
                }
                let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved() else {
                    return Err(scoped_compiler_enum_error_v55());
                };
                if !event.is_reachable()
                    || !event.is_promoted()
                    || variable.get() != record.local.index()
                    || definition.replace(value).is_some()
                {
                    return Err(scoped_compiler_enum_error_v55());
                }
            }
            archive.lookup_original_v29(
                instances,
                instance,
                definition.ok_or_else(scoped_compiler_enum_error_v55)?,
                budget,
            )?
        }
    };
    Ok(CheckedScopedCompilerEnumAccessV55 {
        record,
        anchor,
        spill,
        binding,
    })
}

struct ScopedCompilerEnumComponentV55<'a, 'budget, 'limits> {
    expected: &'a ExecutionEnumSpillV48,
    value: ValueId,
    ordinal: usize,
    matched: bool,
    budget: &'budget mut ArgumentBudgetV1<'limits>,
}

impl SemanticTransportVisitorV1 for ScopedCompilerEnumComponentV55<'_, '_, '_> {
    type Error = ProductionSemanticKirErrorV1;

    fn node(&mut self) -> Result<(), Self::Error> {
        self.budget.charge_work(1).map_err(Into::into)
    }

    fn component(
        &mut self,
        value: ValueId,
        ty: BorrowedTransportTypeV1<'_>,
    ) -> Result<(), Self::Error> {
        self.budget.charge_work(4)?;
        if self.ordinal == self.expected.component {
            let BorrowedTransportTypeV1::Existing(ty) = ty else {
                return Err(scoped_compiler_enum_error_v55());
            };
            // Pointer-bearing transport needs its original loan/custody proof.
            if !matches!(ty, Type::Scalar(_) | Type::Vector(_))
                || value != self.value
                || !enum_spill_types_equal_v48(ty, &self.expected.element, self.budget)?
            {
                return Err(scoped_compiler_enum_error_v55());
            }
            self.matched = true;
        }
        self.ordinal = argument_sum_v1(&[self.ordinal, 1])?;
        Ok(())
    }

    fn invalid(_: &'static str) -> Self::Error {
        scoped_compiler_enum_error_v55()
    }

    fn equal_types(&mut self, left: &Type, right: &Type) -> Result<bool, Self::Error> {
        enum_spill_types_equal_v48(left, right, self.budget)
    }
}

fn check_scoped_compiler_enum_scalar_value_v55(
    checked: &CheckedScopedCompilerEnumAccessV55<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(7)?;
    let ScopedCompilerEnumRoleV55::Store {
        value,
        source: None,
        ..
    } = checked.record.role
    else {
        return Err(scoped_compiler_enum_error_v55());
    };
    let SemanticValueBindingV1::Enum {
        variant, payloads, ..
    } = checked.binding
    else {
        return Err(scoped_compiler_enum_error_v55());
    };
    if variant.is_some_and(|variant| variant != checked.record.variant) {
        return Err(scoped_compiler_enum_error_v55());
    }
    charge_execution_cfg_lookup_v29(payloads.len(), budget)?;
    let field = payloads
        .get(&checked.record.variant)
        .and_then(|fields| fields.get(checked.record.field as usize))
        .ok_or_else(scoped_compiler_enum_error_v55)?;
    let mut visitor = ScopedCompilerEnumComponentV55 {
        expected: checked.spill,
        value,
        ordinal: 0,
        matched: false,
        budget,
    };
    if let SemanticValueBindingV1::Enum {
        discriminant,
        discriminant_ty,
        variant: Some(variant),
        payloads,
        ..
    } = field
    {
        visitor.node()?;
        visitor.component(
            *discriminant,
            BorrowedTransportTypeV1::Existing(discriminant_ty),
        )?;
        charge_execution_cfg_lookup_v29(payloads.len(), visitor.budget)?;
        for field in payloads
            .get(variant)
            .ok_or_else(scoped_compiler_enum_error_v55)?
        {
            field.visit_values_v1(&mut visitor)?;
        }
    } else {
        field.visit_values_v1(&mut visitor)?;
    }
    if !visitor.matched {
        return Err(scoped_compiler_enum_error_v55());
    }
    Ok(())
}

impl SemanticFunctionLoweringV1<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn record_scoped_enum_spill_store_v55(
        &mut self,
        site: ExecutionSiteV29,
        local: SemanticLocalIdV1,
        variant: u32,
        field: u32,
        component: u32,
        position: usize,
        pointer: ValueId,
        value: ValueId,
        source: Option<ScopedMemoryStoreSourceV29>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.record_scoped_enum_spill_access_v55(
            local,
            variant,
            field,
            component,
            position,
            pointer,
            ScopedCompilerEnumRoleV55::Store {
                site,
                source,
                value,
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_scoped_enum_spill_load_v55(
        &mut self,
        block: SemanticBlockIdV1,
        local: SemanticLocalIdV1,
        variant: u32,
        field: u32,
        component: u32,
        position: usize,
        pointer: ValueId,
        result: ValueId,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.record_scoped_enum_spill_access_v55(
            local,
            variant,
            field,
            component,
            position,
            pointer,
            ScopedCompilerEnumRoleV55::Load { block, result },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_scoped_enum_spill_access_v55(
        &mut self,
        local: SemanticLocalIdV1,
        variant: u32,
        field: u32,
        component: u32,
        position: usize,
        pointer: ValueId,
        role: ScopedCompilerEnumRoleV55,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return Ok(());
        }
        self.with_emission_budget_v1(|this, budget| {
            let recorder = this
                .scoped_memory
                .as_mut()
                .ok_or_else(scoped_compiler_enum_error_v55)?;
            if recorder.anchors.subject.ledger != budget.work_ledger_identity_v1() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.charge_work(8)?;
            let anchor = recorder
                .anchors
                .rows
                .len()
                .checked_sub(1)
                .ok_or_else(scoped_compiler_enum_error_v55)?;
            let row = &recorder.anchors.rows[anchor];
            if row.position != position
                || Some(row.block) != recorder.block
                || row.kind
                    != (ScopedMemoryAnchorKindV29::Access {
                        pointer,
                        payload: None,
                    })
                || recorder
                    .anchors
                    .compiler_enum
                    .last()
                    .is_some_and(|previous| previous.anchor >= anchor)
            {
                return Err(scoped_compiler_enum_error_v55());
            }
            match role {
                ScopedCompilerEnumRoleV55::Store { site, .. } => {
                    let frame = match site {
                        ExecutionSiteV29::Statement { .. } => ScopedMemoryFrameV29::operand(
                            site,
                            Some(ExecutionOperandV29::Destination),
                        ),
                        ExecutionSiteV29::Terminator { .. } => ScopedMemoryFrameV29 {
                            site,
                            role: Some(ScopedMemoryRoleV29::CallResult),
                        },
                    };
                    if row.source != Some(frame) {
                        return Err(scoped_compiler_enum_error_v55());
                    }
                }
                ScopedCompilerEnumRoleV55::Load { block, .. } => {
                    if row.source.is_some()
                        || row.block != recorder.anchors.placement.block(block.index())?
                    {
                        return Err(scoped_compiler_enum_error_v55());
                    }
                }
            }
            emission_push_v1(
                &mut recorder.anchors.compiler_enum,
                ScopedCompilerEnumAccessV55 {
                    anchor,
                    local,
                    variant,
                    field,
                    component,
                    pointer,
                    role,
                },
                budget,
            )
        })
    }
}
