// Inert source recipes in the existing recorder. These are not physical origin,
// initialization, lifetime, active-variant or memory read-from certificates.
type ScopedObjectOperationV29 = fe2o3_kernel_ir::StorageOperationV1;
type ScopedObjectProjectionV29 = fe2o3_kernel_ir::StorageProjectionV1;
include!("production_source_object_completion_v29.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedObjectPathV29 {
    first: usize,
    count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectComponentV29 {
    Original {
        projection: SemanticProjectionV1,
        // An IndexUse's original SSA/event identity, not the index-local number.
        selector: Option<ScopedMemoryOccurrenceV29>,
    },
    View {
        projection: ScopedObjectViewProjectionV29,
        ty: SemanticTypeIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectViewProjectionV29 {
    Field(u32),
    Variant(u32),
    // The actual SSA index remains in the full Storage operation. A generated
    // transfer loop has no original Rust IndexUse to fabricate here.
    ArrayElement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectSnapshotRoleV29 {
    Operand(ExecutionOperandV29),
    CallResult,
    Return,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectIdentityV29 {
    Local {
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        generation: u32,
    },
    Snapshot {
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        role: ScopedObjectSnapshotRoleV29,
        ordinal: u32,
    },
    Reference {
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        dereference_prefix: u32,
    },
    EntryValue {
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        argument: u32,
    },
    InlineArgument {
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        argument: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectSourceV29 {
    EntryArgument {
        local: SemanticLocalIdV1,
    },
    ProjectionIndex(ScopedMemoryIndexReadV29),
    Place {
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        local: SemanticLocalIdV1,
        prefix: u32,
    },
    AggregateComponent {
        site: ExecutionSiteV29,
        operand: u32,
        destination: SemanticLocalIdV1,
        variant: Option<u32>,
    },
    EntryComponent {
        local: SemanticLocalIdV1,
        argument: u32,
        abi_piece: u32,
        byte_offset: u64,
        bit_width: u32,
    },
    OperandSnapshot {
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        ordinal: u32,
    },
    CallResultSnapshot {
        site: ExecutionSiteV29,
        ordinal: u32,
    },
    ReturnComponent {
        site: ExecutionSiteV29,
        local: SemanticLocalIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedObjectEndpointV29 {
    object: ScopedObjectIdentityV29,
    source: ScopedObjectSourceV29,
    root_type: SemanticTypeIdV1,
    projected_type: SemanticTypeIdV1,
    root_schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    projected_schema: fe2o3_kernel_ir::StorageLayoutIdV1,
    source_path: ScopedObjectPathV29,
    // Relative to the Local, snapshot, inline argument or reference target
    // view, never a chosen representative allocation of an alias set.
    path: ScopedObjectPathV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectReadOriginV29 {
    Original(ScopedMemoryReadV29),
    ProjectionIndex(ScopedMemoryIndexReadV29),
    EntryComponent,
    ValueTransfer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectValueOriginV29 {
    Original(ScopedMemoryStoreSourceV29),
    Component {
        original: ScopedMemoryStoreSourceV29,
        path: ScopedObjectPathV29,
    },
    ObjectRead { anchor: usize },
    EntryComponent {
        local: SemanticLocalIdV1,
        argument: u32,
        abi_piece: u32,
        byte_offset: u64,
        bit_width: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectTagOriginV29 {
    Statement(ExecutionSiteV29),
    Aggregate(ExecutionSiteV29),
    EntryComponent,
    // The actual tag needed by a source value transfer, not a source
    // Discriminant statement or a trusted active-variant assertion.
    Transfer,
    CopiedTag { anchor: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedObjectRoleV29 {
    Project {
        source: ScopedObjectEndpointV29,
        projected: ScopedObjectEndpointV29,
    },
    ReadValue {
        source: ScopedObjectEndpointV29,
        read: ScopedObjectReadOriginV29,
    },
    WriteValue {
        destination: ScopedObjectEndpointV29,
        value: ScopedObjectValueOriginV29,
    },
    CopyObject {
        source: ScopedObjectEndpointV29,
        destination: ScopedObjectEndpointV29,
    },
    ReadDiscriminant {
        source: ScopedObjectEndpointV29,
        origin: ScopedObjectTagOriginV29,
    },
    SetDiscriminant {
        destination: ScopedObjectEndpointV29,
        origin: ScopedObjectTagOriginV29,
        variant: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedObjectPayloadV29 {
    operation: ScopedObjectOperationV29,
    result: Option<ValueId>,
    role: ScopedObjectRoleV29,
}

#[derive(Clone, Copy, Debug)]
struct ScopedObjectCheckpointV29 {
    rows: usize,
    objects: usize,
    components: usize,
}

fn scoped_object_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("typed object source payload differs from its actual operation")
}

fn scoped_object_pending_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("typed object effects require actual same-candidate source and physical currentness")
}

fn scoped_object_requires_completion_v29(operation: &Operation) -> bool {
    matches!(operation.kind, OperationKind::Storage(_)
        | OperationKind::Alloca { element: Type::StorageObject(_), .. })
}

fn check_pending_object_completion_v29(
    pending: &PendingScopedRootEmissionV29,
    instances: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    physical: bool,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.source_reference_owner_v29(plan)?;
    if !std::ptr::eq(plan.instances, instances) { return Err(ArgumentResourceV1::Accounting.into()); }
    for sidecar in &pending.sidecars.rows {
        budget.charge_work(1)?;
        if let Some(anchors) = &sidecar.scoped_memory_anchors {
            anchors.check_object_ledger(budget)?;
            if (!anchors.objects.is_empty() || !anchors.object_components.is_empty()) && !physical {
                return Err(scoped_object_pending_v29());
            }
            for payload in &anchors.objects {
                budget.charge_work(1)?;
                if !matches!(payload.operation, ScopedObjectOperationV29::ReadValue { .. } | ScopedObjectOperationV29::WriteValue { .. }
                    | ScopedObjectOperationV29::Project { step: ScopedObjectProjectionV29::Field(_) | ScopedObjectProjectionV29::ArrayIndex(_), .. }) {
                    return Err(scoped_object_pending_v29());
                }
            }
        }
    }
    let body = pending.function.body.as_ref().ok_or_else(scoped_object_error_v29)?;
    for block in &body.blocks {
        budget.charge_work(argument_sum_v1(&[1, block.operations.len()])?)?;
        if !physical && block.operations.iter().any(scoped_object_requires_completion_v29) {
            return Err(scoped_object_pending_v29());
        }
    }
    for slot in &slots.slots {
        budget.charge_work(2)?;
        let ScopedSlotRepresentationV29::Object { schema, bytes, alignment } = slot.representation else { continue; };
        if !physical { return Err(scoped_object_pending_v29()); }
        let (ScopedAllocationIdentityV29::OriginalObject { local, generation },
            ScopedAllocationSourceV29::OriginalObject { cell, schema: original }) = (slot.origin.identity, slot.origin.source)
            else { return Err(scoped_object_pending_v29()); };
        if original != schema { return Err(scoped_object_error_v29()); }
        let checked = budget.source_object_storage_v29(plan, cell, slot.instance, SemanticLocalIdV1::from_index(local), generation, schema)?;
        if !matches!(checked, SemanticRetainedStorageV29::Object { cell: c, schema: s, bytes: b, alignment: a }
            if (c, s, b, a) == (cell, schema, bytes, alignment)) { return Err(scoped_object_error_v29()); }
    }
    // This is only a supported-shape gate. The same pending candidate must
    // still pass the source effect census and actual origin/history/lifetime
    // equations before complete_candidate can return it.
    Ok(())
}

impl ScopedObjectRoleV29 {
    fn visit_paths(
        self,
        mut visit: impl FnMut(ScopedObjectPathV29) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.visit_endpoints(|endpoint| { visit(endpoint.source_path)?; visit(endpoint.path) })?;
        if let Self::WriteValue { value: ScopedObjectValueOriginV29::Component { path, .. }, .. } = self {
            visit(path)?;
        }
        Ok(())
    }

    fn visit_endpoints(
        self,
        mut visit: impl FnMut(ScopedObjectEndpointV29) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match self {
            Self::Project { source, projected } => { visit(source)?; visit(projected)?; }
            Self::CopyObject { source, destination } => { visit(source)?; visit(destination)?; }
            Self::ReadValue { source, .. } | Self::ReadDiscriminant { source, .. } => visit(source)?,
            Self::WriteValue { destination, .. } | Self::SetDiscriminant { destination, .. } => visit(destination)?,
        }
        Ok(())
    }
}

impl ScopedObjectPayloadV29 {
    fn check_operation(
        &self,
        operation: &Operation,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_parts(&operation.kind, &operation.results, budget)
    }

    fn check_parts(
        &self,
        kind: &OperationKind,
        results: &[ValueDef],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        let OperationKind::Storage(actual) = *kind else {
            return Err(scoped_object_error_v29());
        };
        if actual != self.operation {
            return Err(scoped_object_error_v29());
        }
        let has_result = match self.operation {
            ScopedObjectOperationV29::Project { .. }
            | ScopedObjectOperationV29::ReadValue { .. }
            | ScopedObjectOperationV29::ReadDiscriminant { .. } => true,
            ScopedObjectOperationV29::WriteValue { .. }
            | ScopedObjectOperationV29::CopyObject { .. }
            | ScopedObjectOperationV29::SetDiscriminant { .. } => false,
        };
        if results.len() != usize::from(has_result)
            || self.result != results.first().map(|result| result.id)
        {
            return Err(scoped_object_error_v29());
        }
        match (self.operation, self.role) {
            (ScopedObjectOperationV29::Project { .. }, ScopedObjectRoleV29::Project { source, projected }) => {
                if source.object != projected.object || source.root_type != projected.root_type
                    || source.root_schema != projected.root_schema
                    || projected.path.count != argument_sum_v1(&[source.path.count, 1])?
                    || !matches!(&results[0].ty, Type::Pointer(pointer)
                        if pointer.pointee.as_ref() == &Type::StorageObject(projected.projected_schema))
                { return Err(scoped_object_error_v29()); }
            }
            (ScopedObjectOperationV29::ReadValue { .. }, ScopedObjectRoleV29::ReadValue { .. })
            | (ScopedObjectOperationV29::WriteValue { .. }, ScopedObjectRoleV29::WriteValue { .. }) => {}
            (ScopedObjectOperationV29::CopyObject { .. }, ScopedObjectRoleV29::CopyObject { source, destination }) => {
                if source.projected_schema != destination.projected_schema { return Err(scoped_object_error_v29()); }
            }
            (ScopedObjectOperationV29::ReadDiscriminant { .. }, ScopedObjectRoleV29::ReadDiscriminant { .. }) => {
                if results[0].ty != Type::Scalar(ScalarType::U128) {
                    return Err(scoped_object_error_v29());
                }
            }
            (ScopedObjectOperationV29::SetDiscriminant { variant: actual, .. },
                ScopedObjectRoleV29::SetDiscriminant { variant, .. }) if actual == variant => {}
            _ => return Err(scoped_object_error_v29()),
        }
        Ok(())
    }

    fn operands(self) -> [Option<ValueId>; 2] {
        match self.operation {
            ScopedObjectOperationV29::Project { base, step } => [Some(base), match step {
                ScopedObjectProjectionV29::ArrayIndex(index) => Some(index),
                ScopedObjectProjectionV29::Field(_)
                | ScopedObjectProjectionV29::Variant { .. }
                | ScopedObjectProjectionV29::VariantForWrite { .. } => None,
            }],
            ScopedObjectOperationV29::ReadValue { address, .. }
            | ScopedObjectOperationV29::ReadDiscriminant { address, .. }
            | ScopedObjectOperationV29::SetDiscriminant { address, .. } => [Some(address), None],
            ScopedObjectOperationV29::WriteValue { address, value, .. } => [Some(address), Some(value)],
            ScopedObjectOperationV29::CopyObject { source, destination, .. } => [Some(source), Some(destination)],
        }
    }

    fn try_map_values(
        &mut self,
        mut map: impl FnMut(ValueId) -> Result<ValueId, ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // Preserve every access and policy while relocating actual operands in
        // the same stable order as StorageOperationV1::try_visit_operands.
        let operation = match self.operation {
            ScopedObjectOperationV29::Project { base, step } => ScopedObjectOperationV29::Project {
                base: map(base)?,
                step: match step {
                    ScopedObjectProjectionV29::ArrayIndex(index) => ScopedObjectProjectionV29::ArrayIndex(map(index)?),
                    ScopedObjectProjectionV29::Field(index) => ScopedObjectProjectionV29::Field(index),
                    ScopedObjectProjectionV29::Variant { index, access } => ScopedObjectProjectionV29::Variant { index, access },
                    ScopedObjectProjectionV29::VariantForWrite { index } => ScopedObjectProjectionV29::VariantForWrite { index },
                },
            },
            ScopedObjectOperationV29::ReadValue { address, access } => ScopedObjectOperationV29::ReadValue {
                address: map(address)?, access,
            },
            ScopedObjectOperationV29::ReadDiscriminant { address, access } => ScopedObjectOperationV29::ReadDiscriminant {
                address: map(address)?, access,
            },
            ScopedObjectOperationV29::WriteValue { address, value, access } => ScopedObjectOperationV29::WriteValue {
                address: map(address)?, value: map(value)?, access,
            },
            ScopedObjectOperationV29::CopyObject { source, destination, source_access, destination_access, overlap } => {
                ScopedObjectOperationV29::CopyObject {
                    source: map(source)?, destination: map(destination)?, source_access, destination_access, overlap,
                }
            }
            ScopedObjectOperationV29::SetDiscriminant { address, variant, access } => ScopedObjectOperationV29::SetDiscriminant {
                address: map(address)?, variant, access,
            },
        };
        let result = self.result.map(&mut map).transpose()?;
        self.operation = operation;
        self.result = result;
        Ok(())
    }
}

fn scoped_object_reserve_append_v29<T>(
    values: &mut Vec<T>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    scoped_object_reserve_additional_v29(values, 1, budget)
}

fn scoped_object_reserve_additional_v29<T>(
    values: &mut Vec<T>,
    additional: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let needed = argument_sum_v1(&[values.len(), additional])?;
    if needed > values.capacity() {
        let count = argument_product_v1(values.capacity().max(2), 2)?.max(needed);
        let mut replacement = emission_vec_v1(count, budget)?;
        budget.charge_work(values.len())?;
        let old_bytes = argument_product_v1(values.capacity(), std::mem::size_of::<T>())?;
        replacement.append(values);
        *values = replacement;
        budget.release_storage(old_bytes)?;
    }
    Ok(())
}

impl ScopedMemoryAnchorsV29 {
    fn object_checkpoint(&self) -> ScopedObjectCheckpointV29 {
        ScopedObjectCheckpointV29 { rows: self.rows.len(), objects: self.objects.len(), components: self.object_components.len() }
    }

    fn rollback_objects(&mut self, checkpoint: ScopedObjectCheckpointV29) {
        self.rows.truncate(checkpoint.rows);
        self.objects.truncate(checkpoint.objects);
        self.object_components.truncate(checkpoint.components);
    }

    fn append_object_path(
        &mut self,
        components: &[ScopedObjectComponentV29],
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<ScopedObjectPathV29, ProductionSemanticKirErrorV1> {
        self.check_object_ledger(budget)?;
        budget.charge_work(argument_sum_v1(&[2, components.len()])?)?;
        if components.len() > MAX_SSA_VALUE_COMPONENTS_V1 { return Err(scoped_object_error_v29()); }
        let path = ScopedObjectPathV29 { first: self.object_components.len(), count: components.len() };
        scoped_object_reserve_additional_v29(&mut self.object_components, components.len(), budget)?;
        self.object_components.extend_from_slice(components);
        Ok(path)
    }

    fn check_object_source(
        &self,
        function: &SemanticFunctionDeclV1,
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        ordinal: usize,
        anchor: &ScopedMemoryAnchorV29,
        payload: &ScopedObjectPayloadV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_object_ledger(budget)?;
        payload.role.visit_endpoints(|endpoint| {
            budget.charge_work(12)?;
            let original_path = self.object_path(endpoint.source_path, budget)?;
            let view_path = self.object_path(endpoint.path, budget)?;
            budget.charge_work(view_path.len())?;
            let mut view_type = endpoint.root_type;
            for component in view_path {
                let ScopedObjectComponentV29::View { ty, .. } = *component else {
                    return Err(scoped_object_error_v29());
                };
                view_type = ty;
            }
            if view_type != endpoint.projected_type { return Err(scoped_object_error_v29()); }
            let source_place = match endpoint.source {
                ScopedObjectSourceV29::EntryArgument { local } => {
                    budget.charge_work(8)?;
                    if anchor.source.is_some()
                        || !function.locals().get(local.index() as usize).is_some_and(|row|
                            row.role().is_entry_argument() && row.ty() == endpoint.root_type)
                        || endpoint.root_type != endpoint.projected_type
                        || endpoint.root_schema != endpoint.projected_schema
                        || endpoint.source_path.count != 0 || endpoint.path.count != 0
                        || !matches!(endpoint.object, ScopedObjectIdentityV29::Local {
                            instance, local: actual, generation: 0
                        } if instance == self.subject.instance && actual == local)
                    { return Err(scoped_object_error_v29()); }
                    None
                }
                ScopedObjectSourceV29::ProjectionIndex(read) => {
                    check_scoped_index_read_v29(function, occurrences, read, budget)?;
                    if anchor.source != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role)))
                        || !original_path.is_empty() || !view_path.is_empty()
                        || endpoint.root_type != read.ty || endpoint.projected_type != read.ty
                        || endpoint.root_schema != endpoint.projected_schema
                    { return Err(scoped_object_error_v29()); }
                    None
                }
                ScopedObjectSourceV29::Place { site, role, local, prefix } => {
                    if anchor.source.map(|frame| frame.site) != Some(site) { return Err(scoped_object_error_v29()); }
                    let place = scoped_object_original_place_v29(function, site, role).ok_or_else(scoped_object_error_v29)?;
                    if place.local() != local || prefix as usize != original_path.len()
                        || place.projections().get(..original_path.len()).is_none()
                    { return Err(scoped_object_error_v29()); }
                    Some((site, role, &place.projections()[..original_path.len()]))
                }
                ScopedObjectSourceV29::OperandSnapshot { site, role, ordinal } => {
                    if anchor.source.map(|frame| frame.site) != Some(site)
                        || !matches!(endpoint.object, ScopedObjectIdentityV29::Snapshot {
                            site: actual_site, role: ScopedObjectSnapshotRoleV29::Operand(operand), ordinal: actual, ..
                        } if actual_site == site && operand == role && actual == ordinal)
                    { return Err(scoped_object_error_v29()); }
                    match scoped_source_operand_v29(function, site, role).ok_or_else(scoped_object_error_v29)? {
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some((site, role, place.projections())),
                        SemanticOperandV1::Constant(_) => None,
                    }
                }
                ScopedObjectSourceV29::CallResultSnapshot { site, ordinal } => {
                    if anchor.source != Some(ScopedMemoryFrameV29 { site, role: Some(ScopedMemoryRoleV29::CallResult) })
                        || !matches!(endpoint.object, ScopedObjectIdentityV29::Snapshot {
                            site: actual_site, role: ScopedObjectSnapshotRoleV29::CallResult, ordinal: actual, ..
                        } if actual_site == site && actual == ordinal)
                        || scoped_source_call_destination_v29(function, site).is_none()
                    { return Err(scoped_object_error_v29()); }
                    None
                }
                ScopedObjectSourceV29::AggregateComponent { site, operand, destination, variant } => {
                    if anchor.source.map(|frame| frame.site) != Some(site) { return Err(scoped_object_error_v29()); }
                    let Some(SemanticStatementKindV1::Assign(assignment)) = scoped_source_statement_v29(function, site) else {
                        return Err(scoped_object_error_v29());
                    };
                    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                        return Err(scoped_object_error_v29());
                    };
                    let actual_variant = match aggregate.kind() {
                        SemanticAggregateKindV1::EnumVariant(index) => Some(*index),
                        SemanticAggregateKindV1::Array | SemanticAggregateKindV1::Tuple
                        | SemanticAggregateKindV1::Aggregate => None,
                    };
                    if assignment.destination().local() != destination || variant != actual_variant
                        || aggregate.operands().get(operand as usize).is_none()
                    { return Err(scoped_object_error_v29()); }
                    if variant.is_none() {
                        if !assignment.destination().projections().is_empty()
                            || !matches!(aggregate.kind(), SemanticAggregateKindV1::Tuple | SemanticAggregateKindV1::Aggregate)
                            || anchor.source != Some(ScopedMemoryFrameV29::operand(site, Some(ExecutionOperandV29::Destination)))
                            || aggregate.operands()[operand as usize].ty() != endpoint.projected_type
                            || !matches!(view_path, [ScopedObjectComponentV29::View {
                                projection: ScopedObjectViewProjectionV29::Field(_), ..
                            }])
                        { return Err(scoped_object_error_v29()); }
                    }
                    Some((site, ExecutionOperandV29::Destination, assignment.destination().projections()))
                }
                ScopedObjectSourceV29::EntryComponent { local, argument, .. } => {
                    let declaration = function.locals().get(local.index() as usize).ok_or_else(scoped_object_error_v29)?;
                    if anchor.source.is_some() || !matches!(declaration.role(),
                        SemanticLocalRoleV1::Argument(actual) | SemanticLocalRoleV1::RustCallTupleField { argument: actual, .. }
                        if actual == argument)
                    { return Err(scoped_object_error_v29()); }
                    if let ScopedObjectIdentityV29::InlineArgument { local: actual, argument: actual_argument, .. }
                        | ScopedObjectIdentityV29::EntryValue { local: actual, argument: actual_argument, .. } = endpoint.object
                    {
                        if actual != local || actual_argument != argument { return Err(scoped_object_error_v29()); }
                    }
                    None
                }
                ScopedObjectSourceV29::ReturnComponent { site, local } => {
                    let ExecutionSiteV29::Terminator { block } = site else { return Err(scoped_object_error_v29()); };
                    if anchor.source.map(|frame| frame.site) != Some(site)
                        || !matches!(function.blocks().get(block.get() as usize).map(|row| row.terminator().kind()),
                            Some(SemanticTerminatorKindV1::Return))
                        || function.locals().get(local.index() as usize).map(|row| row.role()) != Some(SemanticLocalRoleV1::Return)
                    { return Err(scoped_object_error_v29()); }
                    None
                }
            };
            if source_place.map_or(0, |(_, _, original)| original.len()) != original_path.len() {
                return Err(scoped_object_error_v29());
            }
            budget.charge_work(argument_product_v1(original_path.len(), 12)?)?;
            for (index, component) in original_path.iter().enumerate() {
                let ScopedObjectComponentV29::Original { projection, selector } = *component else {
                    return Err(scoped_object_error_v29());
                };
                let (site, role, original) = source_place.ok_or_else(scoped_object_error_v29)?;
                if original.get(index) != Some(&projection) { return Err(scoped_object_error_v29()); }
                match (projection.kind(), selector) {
                    (SemanticProjectionKindV1::Index(local), Some(capture)) => {
                        let event = match capture { ScopedMemoryOccurrenceV29::Promoted { event, .. }
                            | ScopedMemoryOccurrenceV29::Retained { event } => event };
                        let row = occurrences.events().get(event).ok_or_else(scoped_object_error_v29)?;
                        if !row.is_reachable() || row.site() != site || row.operand() != role
                            || row.role() != ExecutionEventV29::ProjectionIndexUse(u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?)
                            || row.event().variable().get() != local.index()
                        { return Err(scoped_object_error_v29()); }
                        let valid = match capture {
                            ScopedMemoryOccurrenceV29::Promoted { definition, .. } => row.is_promoted()
                                && row.resolved() == Some(SsaResolvedEventV1::Use { variable: fe2o3_mir_model::SsaVariableIdV1::new(local.index()), value: definition }),
                            ScopedMemoryOccurrenceV29::Retained { .. } => !row.is_promoted() && row.resolved().is_none(),
                        };
                        if !valid { return Err(scoped_object_error_v29()); }
                    }
                    (SemanticProjectionKindV1::Index(_), None) => return Err(scoped_object_error_v29()),
                    (_, Some(_)) => return Err(scoped_object_error_v29()),
                    (_, None) => {}
                }
            }
            self.check_object_identity(function, endpoint, budget)?;
            Ok(())
        })?;
        self.check_object_value_source(function, occurrences, ordinal, anchor, payload, budget)
    }

    fn check_object_identity(
        &self,
        function: &SemanticFunctionDeclV1,
        endpoint: ScopedObjectEndpointV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        let (instance, expected) = match endpoint.object {
            ScopedObjectIdentityV29::Local { instance, local, .. } => {
                let source_local = match endpoint.source {
                    ScopedObjectSourceV29::ProjectionIndex(read) => read.local,
                    ScopedObjectSourceV29::EntryArgument { local }
                    | ScopedObjectSourceV29::Place { local, .. }
                    | ScopedObjectSourceV29::EntryComponent { local, .. }
                    | ScopedObjectSourceV29::ReturnComponent { local, .. } => local,
                    ScopedObjectSourceV29::AggregateComponent { destination, .. } => destination,
                    ScopedObjectSourceV29::OperandSnapshot { .. }
                    | ScopedObjectSourceV29::CallResultSnapshot { .. } => return Err(scoped_object_error_v29()),
                };
                if local != source_local { return Err(scoped_object_error_v29()); }
                let path = self.object_path(endpoint.source_path, budget)?;
                budget.charge_work(path.len())?;
                if path.iter().any(|component| matches!(component,
                    ScopedObjectComponentV29::Original { projection, .. }
                        if projection.kind() == SemanticProjectionKindV1::Dereference))
                { return Err(scoped_object_error_v29()); }
                (instance, function.locals().get(local.index() as usize).map(|local| local.ty()))
            }
            ScopedObjectIdentityV29::Reference { instance, site, role, dereference_prefix } => {
                let ScopedObjectSourceV29::Place { site: original, role: original_role, prefix, .. } = endpoint.source
                    else { return Err(scoped_object_error_v29()); };
                if original != site || original_role != role || dereference_prefix == 0
                    || dereference_prefix > prefix
                { return Err(scoped_object_error_v29()); }
                let place = scoped_object_original_place_v29(function, site, role).ok_or_else(scoped_object_error_v29)?;
                let projection = place.projections().get(dereference_prefix as usize - 1)
                    .ok_or_else(scoped_object_error_v29)?;
                if projection.kind() != SemanticProjectionKindV1::Dereference { return Err(scoped_object_error_v29()); }
                // This identifies the original reference evaluation. Its complete
                // may-origin set remains owned by the source plan and final graph solver.
                (instance, Some(projection.result_type()))
            }
            ScopedObjectIdentityV29::Snapshot { instance, site, role, ordinal } => {
                let ty = match (role, endpoint.source) {
                    (ScopedObjectSnapshotRoleV29::Operand(operand),
                        ScopedObjectSourceV29::OperandSnapshot { site: original, role, ordinal: actual })
                        if site == original && operand == role && ordinal == actual => {
                        scoped_source_operand_v29(function, site, role).map(|operand| operand.ty())
                    }
                    (ScopedObjectSnapshotRoleV29::CallResult,
                        ScopedObjectSourceV29::CallResultSnapshot { site: original, ordinal: actual })
                        if site == original && ordinal == actual => {
                        scoped_source_call_destination_v29(function, site).map(|place| place.ty())
                    }
                    (ScopedObjectSnapshotRoleV29::Return, ScopedObjectSourceV29::ReturnComponent { site: original, local })
                        if site == original && ordinal == 0 => {
                        function.locals().get(local.index() as usize).map(|local| local.ty())
                    }
                    _ => return Err(scoped_object_error_v29()),
                };
                (instance, ty)
            }
            ScopedObjectIdentityV29::InlineArgument { instance, local, argument }
            | ScopedObjectIdentityV29::EntryValue { instance, local, argument } => {
                if !matches!(endpoint.source, ScopedObjectSourceV29::EntryComponent {
                    local: actual, argument: actual_argument, ..
                } if local == actual && argument == actual_argument) { return Err(scoped_object_error_v29()); }
                (instance, function.locals().get(local.index() as usize).map(|local| local.ty()))
            }
        };
        if instance != self.subject.instance || expected != Some(endpoint.root_type) {
            return Err(scoped_object_error_v29());
        }
        Ok(())
    }

    fn check_object_value_source(
        &self,
        function: &SemanticFunctionDeclV1,
        occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
        ordinal: usize,
        anchor: &ScopedMemoryAnchorV29,
        payload: &ScopedObjectPayloadV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        match payload.role {
            ScopedObjectRoleV29::Project { source, projected } => {
                let prefix = self.object_path(source.path, budget)?;
                let path = self.object_path(projected.path, budget)?;
                budget.charge_work(prefix.len())?;
                if path.get(..prefix.len()) != Some(prefix) { return Err(scoped_object_error_v29()); }
                let ScopedObjectOperationV29::Project { step, .. } = payload.operation
                    else { return Err(scoped_object_error_v29()); };
                let expected = match step {
                    ScopedObjectProjectionV29::Field(index) => ScopedObjectViewProjectionV29::Field(index),
                    ScopedObjectProjectionV29::Variant { index, .. }
                    | ScopedObjectProjectionV29::VariantForWrite { index } => ScopedObjectViewProjectionV29::Variant(index),
                    ScopedObjectProjectionV29::ArrayIndex(_) => ScopedObjectViewProjectionV29::ArrayElement,
                };
                if path.len() != argument_sum_v1(&[prefix.len(), 1])?
                    || path.last() != Some(&ScopedObjectComponentV29::View { projection: expected, ty: projected.projected_type })
                { return Err(scoped_object_error_v29()); }
            }
            ScopedObjectRoleV29::CopyObject { source, destination } => {
                if source.projected_type != destination.projected_type { return Err(scoped_object_error_v29()); }
            }
            ScopedObjectRoleV29::ReadValue { source, read } => match read {
                ScopedObjectReadOriginV29::ProjectionIndex(read) => {
                    check_scoped_index_read_v29(function, occurrences, read, budget)?;
                    if source.source != ScopedObjectSourceV29::ProjectionIndex(read)
                        || source.projected_type != read.ty
                        || !matches!(payload.operation, ScopedObjectOperationV29::ReadValue { access, .. }
                            if !access.volatile && access.address_space == AddressSpace::Private)
                        || anchor.source != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role)))
                    { return Err(scoped_object_error_v29()); }
                }
                ScopedObjectReadOriginV29::Original(read) => {
                    if source.source != (ScopedObjectSourceV29::Place { site: read.site, role: read.role,
                        local: scoped_object_original_place_v29(function, read.site, read.role)
                            .ok_or_else(scoped_object_error_v29)?.local(), prefix: read.prefix })
                        || read.ty != source.projected_type
                    { return Err(scoped_object_error_v29()); }
                    let place = scoped_object_original_place_v29(function, read.site, read.role).ok_or_else(scoped_object_error_v29)?;
                    let ScopedObjectOperationV29::ReadValue { access, .. } = payload.operation else {
                        return Err(scoped_object_error_v29());
                    };
                    check_scoped_read_volatility_v29(function, read, access.volatile, budget)?;
                    check_scoped_payload_occurrence_v29(occurrences, read.site, read.role, place, read.occurrence, budget)?;
                }
                ScopedObjectReadOriginV29::EntryComponent => {
                    if anchor.source.is_some() || !matches!(source.source, ScopedObjectSourceV29::EntryComponent { .. }) {
                        return Err(scoped_object_error_v29());
                    }
                }
                ScopedObjectReadOriginV29::ValueTransfer => {
                    if !matches!(source.source, ScopedObjectSourceV29::Place { .. }
                        | ScopedObjectSourceV29::OperandSnapshot { .. }
                        | ScopedObjectSourceV29::CallResultSnapshot { .. }
                        | ScopedObjectSourceV29::ReturnComponent { .. })
                    { return Err(scoped_object_error_v29()); }
                }
            },
            ScopedObjectRoleV29::WriteValue { destination, value } => match value {
                ScopedObjectValueOriginV29::Original(original)
                | ScopedObjectValueOriginV29::Component { original, .. } => {
                    if let ScopedObjectSourceV29::AggregateComponent { site, operand, variant: None, .. } = destination.source {
                        if !matches!(value, ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::Operand {
                            site: original_site, role: ExecutionOperandV29::RvalueOperand(original_operand), ..
                        }) if original_site == site && original_operand == operand) {
                            return Err(scoped_object_error_v29());
                        }
                    }
                    let mut ty = scoped_payload_source_type_v29(original);
                    if let ScopedObjectValueOriginV29::Component { path, .. } = value {
                        let path = self.object_path(path, budget)?;
                        budget.charge_work(path.len())?;
                        for component in path {
                            let ScopedObjectComponentV29::View { ty: actual, .. } = *component
                                else { return Err(scoped_object_error_v29()); };
                            ty = actual;
                        }
                        if matches!(original, ScopedMemoryStoreSourceV29::Operand {
                            source: ScopedMemoryOperandSourceV29::Memory { .. }, ..
                        }) { return Err(scoped_object_error_v29()); }
                    }
                    if ty != destination.projected_type { return Err(scoped_object_error_v29()); }
                    match original {
                        ScopedMemoryStoreSourceV29::Operand { site, role, ty, source } => {
                            if anchor.source.map(|frame| frame.site) != Some(site) { return Err(scoped_object_error_v29()); }
                            let operand = scoped_source_operand_v29(function, site, role).ok_or_else(scoped_object_error_v29)?;
                            match (operand, source) {
                                (SemanticOperandV1::Constant(value), ScopedMemoryOperandSourceV29::Constant) if value.ty() == ty => {}
                                (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                                    ScopedMemoryOperandSourceV29::Place(occurrence)
                                    | ScopedMemoryOperandSourceV29::Memory { occurrence, .. }) if place.ty() == ty => {
                                    check_scoped_payload_occurrence_v29(occurrences, site, role, place, occurrence, budget)?;
                                    if let ScopedMemoryOperandSourceV29::Memory { access, .. } = source {
                                        let ScopedObjectOperationV29::WriteValue { value, .. } = payload.operation else {
                                            return Err(scoped_object_error_v29());
                                        };
                                        check_scoped_payload_memory_v29(function, &self.rows, ordinal, anchor,
                                            value, site, role, ty, occurrence, access, budget)?;
                                    }
                                }
                                _ => return Err(scoped_object_error_v29()),
                            }
                        }
                        ScopedMemoryStoreSourceV29::Assignment { site, ty } => {
                            if anchor.source.map(|frame| frame.site) != Some(site)
                                || !matches!(scoped_source_statement_v29(function, site), Some(SemanticStatementKindV1::Assign(row)) if row.value().result_type() == ty)
                            { return Err(scoped_object_error_v29()); }
                        }
                        ScopedMemoryStoreSourceV29::CallResult { site, ty } => {
                            if anchor.source != Some(ScopedMemoryFrameV29 { site, role: Some(ScopedMemoryRoleV29::CallResult) })
                                || scoped_source_call_destination_v29(function, site).map(|place| place.ty()) != Some(ty)
                            { return Err(scoped_object_error_v29()); }
                        }
                        ScopedMemoryStoreSourceV29::EntryArgument { local, ty } => {
                            if anchor.source.is_some() || !function.locals().get(local.index() as usize)
                                .is_some_and(|row| row.ty() == ty && row.role().is_entry_argument())
                            { return Err(scoped_object_error_v29()); }
                        }
                    }
                }
                ScopedObjectValueOriginV29::ObjectRead { anchor: source_ordinal } => {
                    let prior = self.rows.get(source_ordinal).ok_or_else(scoped_object_error_v29)?;
                    let loaded = self.object_payload(prior, budget)?;
                    let (ScopedObjectOperationV29::WriteValue { value, .. }, ScopedObjectRoleV29::ReadValue { source, .. }) = (payload.operation, loaded.role) else {
                        return Err(scoped_object_error_v29());
                    };
                    if source_ordinal >= ordinal
                        || loaded.result != Some(value) || source.projected_type != destination.projected_type
                    { return Err(scoped_object_error_v29()); }
                }
                ScopedObjectValueOriginV29::EntryComponent { local, argument, abi_piece, byte_offset, bit_width } => {
                    if anchor.source.is_some() || destination.source != (ScopedObjectSourceV29::EntryComponent {
                        local, argument, abi_piece, byte_offset, bit_width,
                    }) { return Err(scoped_object_error_v29()); }
                }
            },
            ScopedObjectRoleV29::ReadDiscriminant { source, origin }
            | ScopedObjectRoleV29::SetDiscriminant { destination: source, origin, .. } => {
                match (payload.operation, origin) {
                    (ScopedObjectOperationV29::ReadDiscriminant { .. }, ScopedObjectTagOriginV29::Statement(site)) => {
                        if !matches!(scoped_source_statement_v29(function, site), Some(SemanticStatementKindV1::Assign(row))
                            if matches!(row.value().kind(), SemanticRvalueKindV1::Discriminant(_)))
                            || !matches!(source.source, ScopedObjectSourceV29::Place { site: original, role: ExecutionOperandV29::RvaluePlace, .. } if original == site)
                        { return Err(scoped_object_error_v29()); }
                    }
                    (ScopedObjectOperationV29::SetDiscriminant { variant, .. }, ScopedObjectTagOriginV29::Statement(site)) => {
                        if !matches!(scoped_source_statement_v29(function, site), Some(SemanticStatementKindV1::SetDiscriminant { variant_index: original, .. }) if *original == variant)
                            || !matches!(source.source, ScopedObjectSourceV29::Place { site: original, role: ExecutionOperandV29::StatementPlace, .. } if original == site)
                        { return Err(scoped_object_error_v29()); }
                    }
                    (ScopedObjectOperationV29::SetDiscriminant { variant, .. }, ScopedObjectTagOriginV29::Aggregate(site)) => {
                        if !matches!(scoped_source_statement_v29(function, site), Some(SemanticStatementKindV1::Assign(row))
                            if matches!(row.value().kind(), SemanticRvalueKindV1::Aggregate(aggregate)
                                if aggregate.kind() == &SemanticAggregateKindV1::EnumVariant(variant)))
                            || anchor.source.map(|frame| frame.site) != Some(site)
                        { return Err(scoped_object_error_v29()); }
                    }
                    (_, ScopedObjectTagOriginV29::EntryComponent) => {
                        if anchor.source.is_some() || !matches!(source.source, ScopedObjectSourceV29::EntryComponent { .. }) {
                            return Err(scoped_object_error_v29());
                        }
                    }
                    (ScopedObjectOperationV29::ReadDiscriminant { .. }, ScopedObjectTagOriginV29::Transfer) => {
                        if !matches!(source.source, ScopedObjectSourceV29::OperandSnapshot { .. }
                            | ScopedObjectSourceV29::CallResultSnapshot { .. }
                            | ScopedObjectSourceV29::ReturnComponent { .. } | ScopedObjectSourceV29::Place { .. })
                        { return Err(scoped_object_error_v29()); }
                    }
                    (ScopedObjectOperationV29::SetDiscriminant { .. }, ScopedObjectTagOriginV29::CopiedTag { anchor: read }) => {
                        let prior = self.rows.get(read).ok_or_else(scoped_object_error_v29)?;
                        let original = self.object_payload(prior, budget)?;
                        let ScopedObjectRoleV29::ReadDiscriminant { source: from, .. } = original.role
                            else { return Err(scoped_object_error_v29()); };
                        if read >= ordinal || original.result.is_none() || from.projected_type != source.projected_type {
                            return Err(scoped_object_error_v29());
                        }
                        // The tag's actual branch/variant relation is a final
                        // same-graph obligation, never granted by this ordinal.
                    }
                    _ => return Err(scoped_object_error_v29()),
                }
            }
        }
        Ok(())
    }

    fn check_object_ledger(
        &self,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.subject.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn object_payload(
        &self,
        row: &ScopedMemoryAnchorV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<&ScopedObjectPayloadV29, ProductionSemanticKirErrorV1> {
        self.check_object_ledger(budget)?;
        budget.charge_work(3)?;
        let ScopedMemoryAnchorKindV29::Object(index) = row.kind else {
            return Err(scoped_object_error_v29());
        };
        self.objects.get(index).ok_or_else(scoped_object_error_v29)
    }

    fn object_path(
        &self,
        path: ScopedObjectPathV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<&[ScopedObjectComponentV29], ProductionSemanticKirErrorV1> {
        self.check_object_ledger(budget)?;
        budget.charge_work(3)?;
        if path.count > MAX_SSA_VALUE_COMPONENTS_V1 { return Err(scoped_object_error_v29()); }
        let end = argument_sum_v1(&[path.first, path.count])?;
        self.object_components.get(path.first..end).ok_or_else(scoped_object_error_v29)
    }

    fn append_object(
        &mut self,
        block: BlockId,
        position: usize,
        source: Option<ScopedMemoryFrameV29>,
        payload: ScopedObjectPayloadV29,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_object_ledger(budget)?;
        payload.role.visit_paths(|path| { self.object_path(path, budget)?; Ok(()) })?;
        scoped_object_reserve_append_v29(&mut self.objects, budget)?;
        scoped_object_reserve_append_v29(&mut self.rows, budget)?;
        budget.charge_work(3)?;
        let index = self.objects.len();
        // Both capacities and the entire publication are paid. Nothing fallible
        // follows the first append, so no half payload/anchor pair can escape.
        self.objects.push(payload);
        self.rows.push(ScopedMemoryAnchorV29 {
            block, position, source, kind: ScopedMemoryAnchorKindV29::Object(index),
        });
        Ok(())
    }
}

fn scoped_object_original_place_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
) -> Option<&SemanticPlaceV1> {
    if let Some(place) = scoped_payload_place_v29(function, site, role) { return Some(place); }
    match (scoped_source_statement_v29(function, site)?, role) {
        (SemanticStatementKindV1::SetDiscriminant { place, .. }, ExecutionOperandV29::StatementPlace) => Some(place),
        (SemanticStatementKindV1::Assign(assignment), ExecutionOperandV29::RvaluePlace) => {
            match assignment.value().kind() { SemanticRvalueKindV1::Discriminant(place) => Some(place), _ => None }
        }
        _ => None,
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn with_scoped_object_role_v29<T>(
        &mut self,
        role: ScopedObjectRoleV29,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        self.with_scoped_payload_header_v29(argument_product_v1(
            2, std::mem::size_of::<Option<ScopedObjectRoleV29>>(),
        )?, |this| {
            this.with_emission_budget_v1(|this, budget| {
                budget.charge_work(4)?;
                let recorder = this.scoped_memory.as_ref().ok_or_else(scoped_object_error_v29)?;
                recorder.anchors.check_object_ledger(budget)?;
                if recorder.object_role.is_some() { return Err(scoped_object_error_v29()); }
                role.visit_paths(|path| { recorder.anchors.object_path(path, budget)?; Ok(()) })
            })?;
            this.scoped_memory.as_mut().ok_or_else(scoped_object_error_v29)?.object_role = Some(role);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(this)));
            let consumed = match this.scoped_memory.as_mut() {
                Some(recorder) => recorder.object_role.take().is_none(),
                None => { this.deny_scoped_payload_restoration_v29(); false }
            };
            match result {
                Ok(Ok(_)) if !consumed => Err(scoped_object_error_v29()),
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
    }

    fn record_scoped_object_v29(
        &mut self,
        position: usize,
        operation: ScopedObjectOperationV29,
        results: &[ValueDef],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_scoped_payload_header_v29(argument_sum_v1(&[
            std::mem::size_of::<ScopedMemoryAnchorV29>(),
            std::mem::size_of::<ScopedObjectPayloadV29>(),
            argument_product_v1(2, std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>())?,
        ])?, |this| {
            this.with_emission_budget_v1(|this, budget| {
                let recorder = this.scoped_memory.as_mut().ok_or_else(scoped_object_error_v29)?;
                recorder.anchors.check_object_ledger(budget)?;
                let role = recorder.object_role.ok_or_else(scoped_object_error_v29)?;
                let payload = ScopedObjectPayloadV29 { operation, result: results.first().map(|row| row.id), role };
                payload.check_parts(&OperationKind::Storage(operation), results, budget)?;
                recorder.anchors.append_object(
                    recorder.block.ok_or_else(scoped_object_error_v29)?, position, recorder.frame, payload, budget,
                )?;
                recorder.object_role = None;
                recorder.last_load = None;
                Ok(())
            })
        })
    }
}
