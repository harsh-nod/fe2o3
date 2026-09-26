// Payloads bind the existing access row to its actual value producer. They do
// not replace pointer read-from, initialization, lifetime or expression proofs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedMemoryPayloadV29 {
    Load {
        result: ValueId,
        read: ScopedMemoryReadV29,
    },
    IndexLoad {
        result: ValueId,
        read: ScopedMemoryIndexReadV29,
    },
    Store {
        value: ValueId,
        source: ScopedMemoryStoreSourceV29,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedMemoryIndexReadV29 {
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    projection: u32,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
    event: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ScopedMemoryReadV29 {
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    prefix: u32,
    ty: SemanticTypeIdV1,
    occurrence: ScopedMemoryOccurrenceV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedMemoryOccurrenceV29 {
    Promoted {
        event: usize,
        definition: SsaValueV1,
    },
    Retained {
        event: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedMemoryStoreSourceV29 {
    Operand {
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        ty: SemanticTypeIdV1,
        source: ScopedMemoryOperandSourceV29,
    },
    Assignment {
        site: ExecutionSiteV29,
        ty: SemanticTypeIdV1,
    },
    CallResult {
        site: ExecutionSiteV29,
        ty: SemanticTypeIdV1,
    },
    EntryArgument {
        local: SemanticLocalIdV1,
        ty: SemanticTypeIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedMemoryOperandSourceV29 {
    Place(ScopedMemoryOccurrenceV29),
    Memory {
        occurrence: ScopedMemoryOccurrenceV29,
        access: usize,
    },
    Constant,
}

fn scoped_payload_archived_value_v29(
    archive: &SemanticSsaBindingsV1,
    original: &SemanticPlaceV1,
    occurrence: ScopedMemoryOccurrenceV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<ValueId>, ProductionSemanticKirErrorV1> {
    let ScopedMemoryOccurrenceV29::Promoted { definition, .. } = occurrence else {
        return Ok(None);
    };
    charge_execution_cfg_lookup_v29(archive.len(), budget)?;
    let mut value = archive
        .get(&definition)
        .ok_or_else(scoped_memory_error_v29)?;
    for projection in original.projections() {
        budget.charge_work(3)?;
        match (projection.kind(), value) {
            (SemanticProjectionKindV1::Field(field), SemanticValueBindingV1::Aggregate(fields)) => {
                value = fields
                    .get(field as usize)
                    .ok_or_else(scoped_memory_error_v29)?;
            }
            // Memory projections need an actual Load receipt, not a value
            // guessed from the holder's SSA definition or physical type.
            _ => return Ok(None),
        }
    }
    budget.charge_work(2)?;
    Ok(match value {
        SemanticValueBindingV1::Value { id, ty } if scoped_payload_scalar_type_v29(ty) => Some(*id),
        _ => None,
    })
}

fn check_scoped_payload_memory_v29(
    function: &SemanticFunctionDeclV1,
    anchors: &[ScopedMemoryAnchorV29],
    ordinal: usize,
    row: &ScopedMemoryAnchorV29,
    value: ValueId,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    ty: SemanticTypeIdV1,
    occurrence: ScopedMemoryOccurrenceV29,
    access: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let original =
        scoped_payload_place_v29(function, site, role).ok_or_else(scoped_memory_error_v29)?;
    let prior = anchors.get(access).ok_or_else(scoped_memory_error_v29)?;
    let ScopedMemoryAnchorKindV29::Access {
        payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
        ..
    } = prior.kind
    else {
        return Err(scoped_memory_error_v29());
    };
    if access >= ordinal
        || prior.block != row.block
        || prior.position >= row.position
        || prior.source != Some(ScopedMemoryFrameV29::operand(site, Some(role)))
        || result != value
        || read.site != site
        || read.role != role
        || read.ty != ty
        || read.occurrence != occurrence
        || read.prefix as usize != original.projections().len()
        || original.ty() != ty
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}

fn check_scoped_payload_archive_v29(
    archive: &SemanticSsaBindingsV1,
    original: &SemanticPlaceV1,
    occurrence: ScopedMemoryOccurrenceV29,
    value: ValueId,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if scoped_payload_archived_value_v29(archive, original, occurrence, budget)? != Some(value) {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}

fn scoped_payload_place_v29(
    function: &SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
) -> Option<&SemanticPlaceV1> {
    match scoped_source_operand_v29(function, site, role) {
        Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) => Some(place),
        Some(SemanticOperandV1::Constant(_)) => None,
        None => scoped_source_place_v29(function, site, role),
    }
}

fn scoped_payload_occurrence_v29(
    cursor: &ExecutionAvailabilityV29<'_>,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    original: &SemanticPlaceV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<ScopedMemoryOccurrenceV29>, ProductionSemanticKirErrorV1> {
    cursor.check_ledger(budget)?;
    budget.charge_work(7)?;
    if !scoped_payload_place_v29(cursor.function, site, role)
        .is_some_and(|place| std::ptr::eq(place, original))
    {
        return Err(scoped_memory_error_v29());
    }
    let block = match site {
        ExecutionSiteV29::Statement { block, .. } | ExecutionSiteV29::Terminator { block } => block,
    };
    if cursor.block != Some(block) {
        return Err(scoped_memory_error_v29());
    }
    let key = unit_local_source_key_v1(site, role, Some(ExecutionEventV29::BaseUse));
    budget.charge_work(argument_product_v1(
        8,
        scoped_initialization_search_work_v29(cursor.index.len()),
    )?)?;
    let Ok(index) = cursor.index.binary_search_by_key(&key, |row| row.key) else {
        return Ok(None);
    };
    let event = cursor.index[index].index;
    let row = cursor
        .occurrences
        .events()
        .get(event)
        .ok_or_else(scoped_memory_error_v29)?;
    if !row.is_reachable()
        || row.site() != site
        || row.operand() != role
        || row.role() != ExecutionEventV29::BaseUse
        || row.event().variable().get() != original.local().index()
    {
        return Err(scoped_memory_error_v29());
    }
    match (row.is_promoted(), row.resolved()) {
        (true, Some(SsaResolvedEventV1::Use { variable, value }))
            if variable.get() == original.local().index()
                && cursor.current.get(variable.get() as usize) == Some(&Some(value)) =>
        {
            Ok(Some(ScopedMemoryOccurrenceV29::Promoted {
                event,
                definition: value,
            }))
        }
        (false, None) => Ok(Some(ScopedMemoryOccurrenceV29::Retained { event })),
        _ => Err(scoped_memory_error_v29()),
    }
}

fn scoped_payload_prefix_type_v29(
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    prefix: usize,
) -> Result<SemanticTypeIdV1, ProductionSemanticKirErrorV1> {
    if prefix > place.projections().len() {
        return Err(scoped_memory_error_v29());
    }
    if prefix == 0 {
        function
            .locals()
            .get(place.local().index() as usize)
            .map(|local| local.ty())
            .ok_or_else(scoped_memory_error_v29)
    } else {
        Ok(place.projections()[prefix - 1].result_type())
    }
}

fn scoped_payload_scalar_type_v29(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(_) | Type::Pointer(_))
}

fn scoped_payload_source_type_v29(source: ScopedMemoryStoreSourceV29) -> SemanticTypeIdV1 {
    match source {
        ScopedMemoryStoreSourceV29::Operand { ty, .. }
        | ScopedMemoryStoreSourceV29::Assignment { ty, .. }
        | ScopedMemoryStoreSourceV29::CallResult { ty, .. }
        | ScopedMemoryStoreSourceV29::EntryArgument { ty, .. } => ty,
    }
}

fn scoped_payload_type_matches_v29(
    types: &[SemanticTypeDeclV1],
    mut source: SemanticTypeIdV1,
    mut physical: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    // Borrow the physical type tree; no Type clone or second layout table.
    for _ in 0..256 {
        budget.charge_work(7)?;
        let declaration = types
            .get(source.index() as usize)
            .ok_or_else(scoped_memory_error_v29)?;
        if declaration.layout().is_uninhabited() {
            return Ok(false);
        }
        match (declaration.shape(), physical) {
            (
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_),
                Type::Scalar(_),
            ) => {
                let header = std::mem::size_of::<Result<Type, ProductionSemanticKirErrorV1>>();
                budget.reserve_storage(header)?;
                // This closed scalar conversion has no callback or retained
                // allocation; its temporary Type is dropped before exact refund.
                let result = lower_scalar_type(types, source).map(|expected| expected == *physical);
                let release = budget.release_storage(header);
                return result.and_then(|same| {
                    release?;
                    Ok(same)
                });
            }
            (SemanticTypeShapeV1::Pointer(pointer), Type::Pointer(actual))
                if pointer.metadata() == SemanticPointerMetadataV1::None
                    && pointer.pointer_width_bits() == 64
                    && actual.address_space == lower_address_space(pointer.address_space())?
                    && actual.access
                        == match pointer.mutability() {
                            SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
                            SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
                        } =>
            {
                source = pointer.pointee();
                physical = &actual.pointee;
            }
            _ => return Ok(false),
        }
    }
    Err(scoped_memory_error_v29())
}

impl SemanticFunctionLoweringV1<'_, '_> {
    fn deny_scoped_payload_restoration_v29(&self) {
        if let Some(root) = self
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .and_then(|references| references.plan.storage_root.as_ref())
        {
            root.deny_active_root_refund();
        }
    }
    fn complete_scoped_operand_payload_v29(
        &mut self,
        source: ScopedMemoryStoreSourceV29,
        value: ValueId,
    ) -> Result<Option<ScopedMemoryStoreSourceV29>, ProductionSemanticKirErrorV1> {
        let last = self
            .scoped_memory
            .as_mut()
            .and_then(|recorder| recorder.last_load.take());
        let ScopedMemoryStoreSourceV29::Operand {
            site,
            role,
            ty,
            source: ScopedMemoryOperandSourceV29::Place(occurrence),
        } = source
        else {
            return Ok(Some(source));
        };
        self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(5)?;
            let recorder = this
                .scoped_memory
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?;
            let cursor = this
                .execution
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?;
            cursor.check_ledger(budget)?;
            if recorder.anchors.subject != ScopedInitializationSubjectV29::from_cursor(cursor) {
                return Err(scoped_memory_error_v29());
            }
            let original = scoped_payload_place_v29(this.function, site, role)
                .ok_or_else(scoped_memory_error_v29)?;
            check_scoped_payload_occurrence_v29(
                &cursor.occurrences,
                site,
                role,
                original,
                occurrence,
                budget,
            )?;
            if let Some(expected) = scoped_payload_archived_value_v29(
                &this.semantic_ssa_bindings,
                original,
                occurrence,
                budget,
            )? {
                if value != expected {
                    return Err(scoped_memory_error_v29());
                }
                return Ok(Some(source));
            }
            let Some(access) = last else {
                return Ok(None);
            };
            let prior = recorder
                .anchors
                .rows
                .get(access)
                .ok_or_else(scoped_memory_error_v29)?;
            let ScopedMemoryAnchorKindV29::Access {
                payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
                ..
            } = prior.kind
            else {
                return Err(scoped_memory_error_v29());
            };
            // A holder read is not the final operand value. It remains an
            // unresolved payload rather than receiving a memory transport claim.
            if result != value {
                return Ok(None);
            }
            if prior.block != recorder.block.ok_or_else(scoped_memory_error_v29)?
                || prior.source != Some(ScopedMemoryFrameV29::operand(site, Some(role)))
                || read.site != site
                || read.role != role
                || read.ty != ty
                || read.prefix as usize != original.projections().len()
                || read.occurrence != occurrence
                || original.ty() != ty
            {
                return Err(scoped_memory_error_v29());
            }
            Ok(Some(ScopedMemoryStoreSourceV29::Operand {
                site,
                role,
                ty,
                source: ScopedMemoryOperandSourceV29::Memory { occurrence, access },
            }))
        })
    }

    fn finish_scoped_payloads_v29(
        &mut self,
        blocks: &[BasicBlock],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return Ok(());
        }
        self.with_scoped_payload_header_v29(0, |this| {
            this.with_emission_budget_v1(|this, budget| {
                order_scoped_array_payload_recipes_v29(&mut this.private_arrays.effects.rows, budget)?;
                let recorder = this
                    .scoped_memory
                    .as_ref()
                    .ok_or_else(scoped_memory_error_v29)?;
                let cursor = this
                    .execution
                    .as_ref()
                    .ok_or_else(scoped_memory_error_v29)?;
                cursor.check_ledger(budget)?;
                if recorder.frame.is_some()
                    || recorder.read_payload.is_some()
                    || recorder.index_payload.is_some()
                    || recorder.store_payload.is_some()
                    || recorder.anchors.subject
                        != ScopedInitializationSubjectV29::from_cursor(cursor)
                {
                    return Err(scoped_memory_error_v29());
                }
                let mut ordinal = 0;
                for block in blocks {
                    budget.charge_work(1)?;
                    while let Some(row) = recorder.anchors.rows.get(ordinal) {
                        budget.charge_work(3)?;
                        if row.block != block.id {
                            break;
                        }
                        if let ScopedMemoryAnchorKindV29::Access {
                            payload: Some(payload),
                            ..
                        } = row.kind
                        {
                            let operation = block
                                .operations
                                .get(row.position)
                                .ok_or_else(scoped_memory_error_v29)?;
                            check_scoped_payload_v29(
                                this.function,
                                &cursor.occurrences,
                                row,
                                operation,
                                budget,
                            )?;
                            check_scoped_array_initializer_recipe_v29(
                                this.function,
                                row,
                                &this.private_arrays.effects.rows,
                                budget,
                            )?;
                            if let ScopedMemoryPayloadV29::Store {
                                value,
                                source:
                                    ScopedMemoryStoreSourceV29::Operand {
                                        site,
                                        role,
                                        ty,
                                        source,
                                    },
                            } = payload
                            {
                                match source {
                                    ScopedMemoryOperandSourceV29::Place(occurrence) => {
                                        let original =
                                            scoped_payload_place_v29(this.function, site, role)
                                                .ok_or_else(scoped_memory_error_v29)?;
                                        check_scoped_payload_archive_v29(
                                            &this.semantic_ssa_bindings,
                                            original,
                                            occurrence,
                                            value,
                                            budget,
                                        )?;
                                    }
                                    ScopedMemoryOperandSourceV29::Memory { occurrence, access } => {
                                        check_scoped_payload_memory_v29(
                                            this.function,
                                            &recorder.anchors.rows,
                                            ordinal,
                                            row,
                                            value,
                                            site,
                                            role,
                                            ty,
                                            occurrence,
                                            access,
                                            budget,
                                        )?;
                                    }
                                    ScopedMemoryOperandSourceV29::Constant => {}
                                }
                            }
                        }
                        ordinal = argument_sum_v1(&[ordinal, 1])?;
                    }
                }
                if ordinal != recorder.anchors.rows.len() {
                    return Err(scoped_memory_error_v29());
                }
                Ok(())
            })
        })
    }

    fn with_scoped_payload_header_v29<T>(
        &mut self,
        extra: usize,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return body(self);
        }
        let cursor = self
            .execution
            .as_ref()
            .ok_or_else(scoped_memory_error_v29)?;
        let ledger = cursor.ledger;
        let source = cursor.references.map(|row| row.plan);
        let root = source.and_then(|plan| plan.storage_root.as_ref());
        let subject = self
            .scoped_memory
            .as_ref()
            .ok_or_else(scoped_memory_error_v29)?
            .anchors
            .subject;
        let header = argument_sum_v1(&[
            extra,
            std::mem::size_of::<ScopedInitializationSubjectV29>(),
            std::mem::size_of::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>(
            ),
            std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<std::thread::Result<Result<T, ProductionSemanticKirErrorV1>>>(),
        ])?;
        let (slot, entry, required) = self.with_emission_budget_v1(|_, budget| {
            if budget.work_ledger_identity_v1() != ledger {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let slot = budget
                .prepared_input_slot_v1()
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.charge_work(argument_sum_v1(&[
                4,
                8,
                if root.is_some() {
                    source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29
                } else {
                    0
                },
            ])?)?;
            let entry = budget.storage();
            let required = argument_sum_v1(&[entry, header])?;
            budget.reserve_storage(header)?;
            Ok((slot, entry, required))
        })?;
        let mut growth = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            if let Some(root) = root {
                growth = Some(
                    root.capture_retained_growth()
                        .ok_or(ArgumentResourceV1::Accounting)?,
                );
            }
            body(self)
        }));
        let same_recorder = self
            .scoped_memory
            .as_ref()
            .is_some_and(|recorder| recorder.anchors.subject == subject);
        if !same_recorder && let Some(root) = root {
            root.deny_active_root_refund();
        }
        // The callback has restored/dropped its temporary capture slots. Only
        // this fixed header is ours; emitted rows and live root growth stay paid.
        let refunded = self.with_emission_budget_v1(|_, budget| {
            Ok(same_recorder
                && scoped_emission_refund_v29(
                    source,
                    ledger,
                    slot,
                    entry,
                    required,
                    growth,
                    Some(header),
                    budget,
                ))
        });
        match result {
            Ok(Ok(value)) => {
                if matches!(refunded, Ok(true)) {
                    Ok(value)
                } else {
                    Err(ArgumentResourceV1::Accounting.into())
                }
            }
            Ok(Err(error)) => Err(error),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn with_scoped_local_read_payload_v29<T>(
        &mut self,
        local: SemanticLocalIdV1,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        if let Some(read) = self.scoped_memory.as_ref().and_then(|row| row.index_payload) {
            if read.local != local {
                return Err(scoped_memory_error_v29());
            }
            return body(self);
        }
        let source = self
            .scoped_memory
            .as_ref()
            .and_then(|recorder| recorder.read_payload);
        let Some((read, _)) = source else {
            return body(self);
        };
        let original = scoped_payload_place_v29(self.function, read.site, read.role)
            .ok_or_else(scoped_memory_error_v29)?;
        if original.local() != local {
            return Err(scoped_memory_error_v29());
        }
        self.with_scoped_read_payload_v29(original, 0, body)
    }

    fn prepare_scoped_read_source_v29(
        &mut self,
        frame: ScopedMemoryFrameV29,
    ) -> Result<Option<ScopedMemoryReadV29>, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return Ok(None);
        }
        let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
            return Ok(None);
        };
        let Some(original) = scoped_payload_place_v29(self.function, frame.site, role) else {
            return Ok(None);
        };
        self.with_scoped_payload_header_v29(0, |this| {
            this.with_emission_budget_v1(|this, budget| {
                let cursor = this
                    .execution
                    .as_ref()
                    .ok_or_else(scoped_memory_error_v29)?;
                let Some(occurrence) =
                    scoped_payload_occurrence_v29(cursor, frame.site, role, original, budget)?
                else {
                    return Ok(None);
                };
                Ok(Some(ScopedMemoryReadV29 {
                    site: frame.site,
                    role,
                    prefix: u32::try_from(original.projections().len())
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    ty: original.ty(),
                    occurrence,
                }))
            })
        })
    }

    fn prepare_scoped_operand_payload_v29(
        &mut self,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        original: &SemanticOperandV1,
    ) -> Result<Option<ScopedMemoryStoreSourceV29>, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return Ok(None);
        }
        self.scoped_memory
            .as_mut()
            .ok_or_else(scoped_memory_error_v29)?
            .last_load = None;
        self.with_scoped_payload_header_v29(0, |this| {
            this.with_emission_budget_v1(|this, budget| {
                budget.charge_work(5)?;
                if !scoped_source_operand_v29(this.function, site, role)
                    .is_some_and(|source| std::ptr::eq(source, original))
                {
                    return Err(scoped_memory_error_v29());
                }
                let (ty, source) = match original {
                    SemanticOperandV1::Constant(constant) => {
                        (constant.ty(), ScopedMemoryOperandSourceV29::Constant)
                    }
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                        let cursor = this
                            .execution
                            .as_ref()
                            .ok_or_else(scoped_memory_error_v29)?;
                        let occurrence =
                            scoped_payload_occurrence_v29(cursor, site, role, place, budget)?
                                .ok_or_else(scoped_memory_error_v29)?;
                        (place.ty(), ScopedMemoryOperandSourceV29::Place(occurrence))
                    }
                };
                Ok(Some(ScopedMemoryStoreSourceV29::Operand {
                    site,
                    role,
                    ty,
                    source,
                }))
            })
        })
    }

    fn prepare_scoped_array_initializer_payload_v29(
        &mut self,
        site: ExecutionSiteV29,
        destination: &SemanticPlaceV1,
        component: usize,
    ) -> Result<Option<ScopedMemoryStoreSourceV29>, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return Ok(None);
        }
        let component = u32::try_from(component).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        self.with_scoped_payload_header_v29(
            std::mem::size_of::<Result<&SemanticOperandV1, ProductionSemanticKirErrorV1>>(),
            |this| {
                let function = this.function;
                let types = this.types;
                let original = this.with_emission_budget_v1(|_, budget| {
                    budget.charge_work(2)?;
                    if !matches!(scoped_source_statement_v29(function, site),
                        Some(SemanticStatementKindV1::Assign(assignment))
                            if std::ptr::eq(assignment.destination(), destination))
                    {
                        return Err(scoped_memory_error_v29());
                    }
                    scoped_array_initializer_operand_v29(types, function, site, component, budget)
                })?;
                this.prepare_scoped_operand_payload_v29(
                    site,
                    ExecutionOperandV29::RvalueOperand(component),
                    original,
                )
            },
        )
    }

    fn with_scoped_store_payload_v29<T>(
        &mut self,
        source: Option<ScopedMemoryStoreSourceV29>,
        value: SemanticValueBindingV1,
        body: impl FnOnce(&mut Self, SemanticValueBindingV1) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return body(self, value);
        }
        let header = argument_sum_v1(&[
            argument_product_v1(
                2,
                std::mem::size_of::<Option<(ValueId, ScopedMemoryStoreSourceV29)>>(),
            )?,
            std::mem::size_of::<SemanticValueBindingV1>(),
            std::mem::size_of::<Option<usize>>(),
        ])?;
        self.with_scoped_payload_header_v29(header, |this| {
            this.with_scoped_store_payload_inner_v29(source, value, body)
        })
    }

    fn with_scoped_store_payload_inner_v29<T>(
        &mut self,
        source: Option<ScopedMemoryStoreSourceV29>,
        value: SemanticValueBindingV1,
        body: impl FnOnce(&mut Self, SemanticValueBindingV1) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let token = match (source, &value) {
            (Some(source), SemanticValueBindingV1::Value { id, ty })
                if scoped_payload_scalar_type_v29(ty) =>
            {
                self.with_emission_budget_v1(|this, budget| {
                    scoped_payload_type_matches_v29(
                        this.types,
                        scoped_payload_source_type_v29(source),
                        ty,
                        budget,
                    )
                })?
                .then_some((*id, source))
            }
            _ => None,
        };
        let token = match token {
            Some((value, source)) => self
                .complete_scoped_operand_payload_v29(source, value)?
                .map(|source| (value, source)),
            None => {
                self.scoped_memory
                    .as_mut()
                    .ok_or_else(scoped_memory_error_v29)?
                    .last_load = None;
                None
            }
        };
        let recorder = self
            .scoped_memory
            .as_mut()
            .ok_or_else(scoped_memory_error_v29)?;
        let previous = std::mem::replace(&mut recorder.store_payload, token);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self, value)));
        let restored = if let Some(recorder) = self.scoped_memory.as_mut() {
            recorder.store_payload = previous;
            true
        } else {
            self.deny_scoped_payload_restoration_v29();
            false
        };
        match result {
            Ok(Ok(_)) if !restored => Err(scoped_memory_error_v29()),
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn with_scoped_read_payload_v29<T>(
        &mut self,
        original: &SemanticPlaceV1,
        prefix: usize,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        if self.scoped_memory.is_none() {
            return body(self);
        }
        let header = argument_sum_v1(&[
            argument_product_v1(
                2,
                std::mem::size_of::<Option<(ScopedMemoryReadV29, bool)>>(),
            )?,
            std::mem::size_of::<
                Result<Option<(ScopedMemoryReadV29, bool)>, ProductionSemanticKirErrorV1>,
            >(),
        ])?;
        self.with_scoped_payload_header_v29(header, |this| {
            this.with_scoped_read_payload_inner_v29(original, prefix, body)
        })
    }

    fn with_scoped_read_payload_inner_v29<T>(
        &mut self,
        original: &SemanticPlaceV1,
        prefix: usize,
        body: impl FnOnce(&mut Self) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let token = self.with_emission_budget_v1(|this, budget| {
            budget.charge_work(6)?;
            let Some((mut source, _)) = this
                .scoped_memory
                .as_ref()
                .ok_or_else(scoped_memory_error_v29)?
                .read_payload
            else {
                return Ok(None);
            };
            if !scoped_payload_place_v29(this.function, source.site, source.role)
                .is_some_and(|place| std::ptr::eq(place, original))
            {
                return Err(scoped_memory_error_v29());
            }
            source.ty = scoped_payload_prefix_type_v29(this.function, original, prefix)?;
            source.prefix = u32::try_from(prefix).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            Ok(Some((source, true)))
        })?;
        let recorder = self
            .scoped_memory
            .as_mut()
            .ok_or_else(scoped_memory_error_v29)?;
        let previous = std::mem::replace(&mut recorder.read_payload, token);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self)));
        let restored = if let Some(recorder) = self.scoped_memory.as_mut() {
            recorder.read_payload = previous;
            true
        } else {
            self.deny_scoped_payload_restoration_v29();
            false
        };
        match result {
            Ok(Ok(_)) if !restored => Err(scoped_memory_error_v29()),
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

fn scoped_recorded_payload_v29(
    recorder: &ScopedMemoryRecorderV29,
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    kind: &OperationKind,
    results: &[ValueDef],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Option<ScopedMemoryPayloadV29>, ProductionSemanticKirErrorV1> {
    budget.charge_work(5)?;
    match kind {
        OperationKind::Load { .. } | OperationKind::GuardedLoad { .. } => {
            if let Some(read) = recorder.index_payload {
                let [result] = results else {
                    return Err(scoped_memory_error_v29());
                };
                if recorder.read_payload.is_some()
                    || recorder.frame != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role)))
                {
                    return Err(scoped_memory_error_v29());
                }
                return Ok(Some(ScopedMemoryPayloadV29::IndexLoad { result: result.id, read }));
            }
            let Some((read, true)) = recorder.read_payload else {
                return Ok(None);
            };
            let [result] = results else {
                return Err(scoped_memory_error_v29());
            };
            if !scoped_payload_scalar_type_v29(&result.ty) {
                return Ok(None);
            }
            if !scoped_payload_type_matches_v29(types, read.ty, &result.ty, budget)? {
                return Ok(None);
            }
            if recorder.frame != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role))) {
                return Err(scoped_memory_error_v29());
            }
            Ok(Some(ScopedMemoryPayloadV29::Load {
                result: result.id,
                read,
            }))
        }
        OperationKind::Store { value, .. } | OperationKind::GuardedStore { value, .. } => {
            let Some((expected, source)) = recorder.store_payload else {
                return Ok(None);
            };
            if !results.is_empty() || expected != *value {
                return Ok(None);
            }
            // A matching generated RHS is still not a source Store: its frame
            // must be the precise destination/result selected by the producer.
            let compatible = match source {
                ScopedMemoryStoreSourceV29::Operand {
                    site,
                    role: ExecutionOperandV29::StoreValue,
                    ..
                } => {
                    recorder.frame
                        == Some(ScopedMemoryFrameV29::operand(
                            site,
                            Some(ExecutionOperandV29::StoreDestination),
                        ))
                }
                ScopedMemoryStoreSourceV29::Operand {
                    site,
                    role: ExecutionOperandV29::RvalueOperand(component),
                    ..
                } => {
                    if matches!(scoped_source_statement_v29(function, site),
                        Some(SemanticStatementKindV1::Assign(assignment))
                            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(aggregate)
                                if aggregate.kind() == &SemanticAggregateKindV1::Array))
                    {
                        scoped_array_initializer_operand_v29(types, function, site, component, budget)?;
                    } else if component != 0 {
                        return Ok(None);
                    }
                    recorder.frame
                        == Some(ScopedMemoryFrameV29::operand(
                            site,
                            Some(ExecutionOperandV29::Destination),
                        ))
                }
                ScopedMemoryStoreSourceV29::Assignment { site, .. } => {
                    recorder.frame
                        == Some(ScopedMemoryFrameV29::operand(
                            site,
                            Some(ExecutionOperandV29::Destination),
                        ))
                }
                ScopedMemoryStoreSourceV29::CallResult { site, .. } => {
                    recorder.frame
                        == Some(ScopedMemoryFrameV29 {
                            site,
                            role: Some(ScopedMemoryRoleV29::CallResult),
                        })
                }
                ScopedMemoryStoreSourceV29::EntryArgument { .. } => recorder.frame.is_none(),
                _ => false,
            };
            Ok(compatible.then_some(ScopedMemoryPayloadV29::Store {
                value: *value,
                source,
            }))
        }
        _ => Ok(None),
    }
}

fn check_scoped_payload_occurrence_v29(
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    place: &SemanticPlaceV1,
    capture: ScopedMemoryOccurrenceV29,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(9)?;
    let event = match capture {
        ScopedMemoryOccurrenceV29::Promoted { event, .. }
        | ScopedMemoryOccurrenceV29::Retained { event } => event,
    };
    let row = occurrences
        .events()
        .get(event)
        .ok_or_else(scoped_memory_error_v29)?;
    if !row.is_reachable()
        || row.site() != site
        || row.operand() != role
        || row.role() != ExecutionEventV29::BaseUse
        || row.event().variable().get() != place.local().index()
    {
        return Err(scoped_memory_error_v29());
    }
    let valid = match capture {
        ScopedMemoryOccurrenceV29::Promoted { definition, .. } => {
            row.is_promoted()
                && row.resolved()
                    == Some(SsaResolvedEventV1::Use {
                        variable: fe2o3_mir_model::SsaVariableIdV1::new(place.local().index()),
                        value: definition,
                    })
        }
        ScopedMemoryOccurrenceV29::Retained { .. } => {
            !row.is_promoted() && row.resolved().is_none()
        }
    };
    if !valid {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}

// The existing private-array initializer relation separately checks the exact
// literal definition, component GEP and Store. This supplies its source role,
// not a new aggregate value or initialization certificate.
fn scoped_array_initializer_operand_v29<'a>(
    types: &[SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    site: ExecutionSiteV29,
    component: u32,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<&'a SemanticOperandV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(16)?;
    let Some(SemanticStatementKindV1::Assign(assignment)) =
        scoped_source_statement_v29(function, site)
    else {
        return Err(scoped_memory_error_v29());
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return Err(scoped_memory_error_v29());
    };
    let destination = assignment.destination();
    let Some(SemanticTypeShapeV1::Array { element, length }) = types
        .get(destination.ty().index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(scoped_memory_error_v29());
    };
    let original = aggregate
        .operands()
        .get(component as usize)
        .ok_or_else(scoped_memory_error_v29)?;
    if aggregate.kind() != &SemanticAggregateKindV1::Array
        || !destination.projections().is_empty()
        || assignment.value().result_type() != destination.ty()
        || function.locals().get(destination.local().index() as usize).map(|row| row.ty())
            != Some(destination.ty())
        || u64::try_from(aggregate.operands().len()).ok() != Some(*length)
        || u64::from(component) >= *length
        || !matches!(original, SemanticOperandV1::Constant(constant)
            if constant.ty() == *element
                && matches!(constant.value(), SemanticConstantValueV1::Scalar(_)))
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(original)
}

impl PrivateArrayChargeV1 for &mut dyn SemanticEmissionBudgetV1 {
    type Error = ProductionSemanticKirErrorV1;

    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.charge_work(amount)
    }
}

fn order_scoped_array_payload_recipes_v29(
    effects: &mut [PrivateArrayEffectV1],
    mut budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // Frame emission follows source RPO. Sort the existing rows once before
    // source-key lookup; into_rows only moves them, it does not canonicalize.
    private_array_heapsort_v1(effects, private_array_effect_key_v1, &mut budget, scoped_memory_error_v29)?;
    for pair in effects.windows(2) {
        if private_array_compare_keys_v1(
            private_array_effect_key_v1(&pair[0]),
            private_array_effect_key_v1(&pair[1]),
            &mut budget,
        )? != std::cmp::Ordering::Less {
            return Err(scoped_memory_error_v29());
        }
    }
    Ok(())
}

fn check_scoped_array_initializer_recipe_v29(
    function: &SemanticFunctionDeclV1,
    row: &ScopedMemoryAnchorV29,
    effects: &[PrivateArrayEffectV1],
    mut budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let ScopedMemoryAnchorKindV29::Access {
        pointer,
        payload: Some(ScopedMemoryPayloadV29::Store {
            value,
            source: ScopedMemoryStoreSourceV29::Operand {
                site,
                role: ExecutionOperandV29::RvalueOperand(component),
                source: ScopedMemoryOperandSourceV29::Constant,
                ..
            },
        }),
    } = row.kind else {
        return Ok(());
    };
    let Some(SemanticStatementKindV1::Assign(assignment)) =
        scoped_source_statement_v29(function, site)
    else {
        return Err(scoped_memory_error_v29());
    };
    if !matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(aggregate)
        if aggregate.kind() == &SemanticAggregateKindV1::Array)
    {
        return Ok(());
    }
    let (block, statement) = scoped_memory_site_key_v29(site);
    let statement = statement.ok_or_else(scoped_memory_error_v29)?;
    let role = private_array_role_key_v1(ExecutionOperandV29::Destination)
        .ok_or_else(scoped_memory_error_v29)?;
    let index = private_array_binary_search_v1(
        effects,
        |effect| {
            let role = private_array_role_key_v1(effect.role).unwrap_or((u8::MAX, u32::MAX));
            [effect.semantic_block as usize, effect.semantic_statement as usize,
                role.0 as usize, role.1 as usize, effect.original_index.component() as usize]
        },
        [block as usize, statement as usize, role.0 as usize, role.1 as usize, component as usize],
        &mut budget,
    )?.map_err(|_| scoped_memory_error_v29())?;
    budget.charge_work(12)?;
    let effect = &effects[index];
    if effect.role != ExecutionOperandV29::Destination
        || effect.access != PrivateArrayAccessV1::Write
        || effect.local != assignment.destination().local().index()
        || effect.semantic_type != assignment.destination().ty()
        || effect.memory_location.block != row.block
        || effect.memory_location.operation != row.position
        || effect.gep != pointer
        || !matches!(effect.original_index,
            PrivateArrayIndexV1::InitializerElement {
                component: expected,
                value: PrivateArrayInitializerValueV1::LiteralScalar { value: actual, .. },
            } if expected == component && actual == value)
    {
        return Err(scoped_memory_error_v29());
    }
    Ok(())
}

fn check_scoped_payload_v29(
    function: &SemanticFunctionDeclV1,
    occurrences: &ProductionSemanticSsaFunctionOccurrencesV1<'_>,
    row: &ScopedMemoryAnchorV29,
    operation: &Operation,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let ScopedMemoryAnchorKindV29::Access {
        payload: Some(payload),
        ..
    } = row.kind
    else {
        return Ok(());
    };
    budget.charge_work(8)?;
    if !occurrences
        .owner()
        .source_semantic()
        .functions()
        .get(occurrences.function().index() as usize)
        .is_some_and(|source| std::ptr::eq(source, function))
    {
        return Err(scoped_memory_error_v29());
    }
    match payload {
        ScopedMemoryPayloadV29::IndexLoad { result, read } => {
            check_scoped_index_payload_v29(function, occurrences, row, operation, result, read, budget)
        }
        ScopedMemoryPayloadV29::Load { result, read } => {
            if !matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
            ) || row.source != Some(ScopedMemoryFrameV29::operand(read.site, Some(read.role)))
            {
                return Err(scoped_memory_error_v29());
            }
            let [actual] = operation.results.as_slice() else {
                return Err(scoped_memory_error_v29());
            };
            let original = scoped_payload_place_v29(function, read.site, read.role)
                .ok_or_else(scoped_memory_error_v29)?;
            if actual.id != result
                || scoped_payload_prefix_type_v29(function, original, read.prefix as usize)?
                    != read.ty
                || !scoped_payload_type_matches_v29(
                    occurrences.owner().source_semantic().types(),
                    read.ty,
                    &actual.ty,
                    budget,
                )?
            {
                return Err(scoped_memory_error_v29());
            }
            let volatile = match operation.kind {
                OperationKind::Load { access, .. } | OperationKind::GuardedLoad { access, .. } => access.volatile,
                _ => return Err(scoped_memory_error_v29()),
            };
            check_scoped_read_volatility_v29(function, read, volatile, budget)?;
            check_scoped_payload_occurrence_v29(
                occurrences,
                read.site,
                read.role,
                original,
                read.occurrence,
                budget,
            )
        }
        ScopedMemoryPayloadV29::Store { value, source } => {
            if !operation.results.is_empty()
                || !matches!(operation.kind,
                OperationKind::Store { value: actual, .. } | OperationKind::GuardedStore { value: actual, .. } if actual == value)
            {
                return Err(scoped_memory_error_v29());
            }
            match source {
                ScopedMemoryStoreSourceV29::Operand {
                    site,
                    role,
                    ty,
                    source,
                } => {
                    let destination_role = match role {
                        ExecutionOperandV29::StoreValue => ExecutionOperandV29::StoreDestination,
                        ExecutionOperandV29::RvalueOperand(component) => {
                            if matches!(scoped_source_statement_v29(function, site),
                                Some(SemanticStatementKindV1::Assign(assignment))
                                    if matches!(assignment.value().kind(),
                                        SemanticRvalueKindV1::Aggregate(aggregate)
                                            if aggregate.kind() == &SemanticAggregateKindV1::Array))
                            {
                                scoped_array_initializer_operand_v29(
                                    occurrences.owner().source_semantic().types(),
                                    function,
                                    site,
                                    component,
                                    budget,
                                )?;
                            } else if component != 0 {
                                return Err(scoped_memory_error_v29());
                            }
                            ExecutionOperandV29::Destination
                        }
                        _ => return Err(scoped_memory_error_v29()),
                    };
                    if row.source
                        != Some(ScopedMemoryFrameV29::operand(site, Some(destination_role)))
                    {
                        return Err(scoped_memory_error_v29());
                    }
                    match (source, scoped_source_operand_v29(function, site, role)) {
                        (
                            ScopedMemoryOperandSourceV29::Constant,
                            Some(SemanticOperandV1::Constant(original)),
                        ) if original.ty() == ty => Ok(()),
                        (
                            ScopedMemoryOperandSourceV29::Place(
                                capture @ ScopedMemoryOccurrenceV29::Promoted { .. },
                            )
                            | ScopedMemoryOperandSourceV29::Memory {
                                occurrence: capture,
                                ..
                            },
                            Some(
                                SemanticOperandV1::Copy(original)
                                | SemanticOperandV1::Move(original),
                            ),
                        ) if original.ty() == ty => check_scoped_payload_occurrence_v29(
                            occurrences,
                            site,
                            role,
                            original,
                            capture,
                            budget,
                        ),
                        _ => Err(scoped_memory_error_v29()),
                    }
                }
                ScopedMemoryStoreSourceV29::Assignment { site, ty } => {
                    if row.source
                        != Some(ScopedMemoryFrameV29::operand(
                            site,
                            Some(ExecutionOperandV29::Destination),
                        ))
                        || !matches!(scoped_source_statement_v29(function, site),
                            Some(SemanticStatementKindV1::Assign(assignment)) if assignment.value().result_type() == ty)
                    {
                        return Err(scoped_memory_error_v29());
                    }
                    Ok(())
                }
                ScopedMemoryStoreSourceV29::CallResult { site, ty } => {
                    if row.source
                        != Some(ScopedMemoryFrameV29 {
                            site,
                            role: Some(ScopedMemoryRoleV29::CallResult),
                        })
                        || scoped_source_call_destination_v29(function, site)
                            .map(|place| place.ty())
                            != Some(ty)
                    {
                        return Err(scoped_memory_error_v29());
                    }
                    Ok(())
                }
                ScopedMemoryStoreSourceV29::EntryArgument { local, ty } => {
                    if row.source.is_some()
                        || !function.locals().get(local.index() as usize).is_some_and(
                            |declaration| {
                                declaration.ty() == ty && declaration.role().is_entry_argument()
                            },
                        )
                    {
                        return Err(scoped_memory_error_v29());
                    }
                    // The existing slot/ABI census independently checks this
                    // exact Store against the actual original input parameter.
                    Ok(())
                }
            }
        }
    }
}
