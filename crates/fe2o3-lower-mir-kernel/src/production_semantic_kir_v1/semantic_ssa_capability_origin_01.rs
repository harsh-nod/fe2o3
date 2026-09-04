fn option_component_witness_contract_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
    witness_type: SemanticTypeIdV1,
) -> Option<SemanticDisjointIndexSpaceV1> {
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
            output_block,
            output_space,
            ..
        } if *output_block == witness_type => Some(*output_space),
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
            output_tile,
            output_space,
            ..
        } if *output_tile == witness_type => Some(*output_space),
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
            output_stripe,
            output_space,
            ..
        } if *output_stripe == witness_type => Some(*output_space),
        _ => None,
    }
}
fn direct_index_witness_contract_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
    witness_type: SemanticTypeIdV1,
) -> Option<SemanticPromotedBindingV1> {
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { index_witness, .. }
            if *index_witness == witness_type =>
        {
            Some(SemanticPromotedBindingV1::IndexWitness {
                index_space: SemanticDisjointIndexSpaceV1::Index1d,
                disjoint: false,
                availability: None,
            })
        }
        SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
            output_witness,
            index_space,
            ..
        } if *output_witness == witness_type => Some(SemanticPromotedBindingV1::IndexWitness {
            index_space: *index_space,
            disjoint: true,
            availability: None,
        }),
        _ => None,
    }
}

fn option_capability_contract_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
    payload_type: SemanticTypeIdV1,
    availability: SemanticOptionAvailabilityV1,
) -> Option<SemanticPromotedBindingV1> {
    let availability = SemanticCapabilityAvailabilityV1::Option(availability);
    match operation {
        SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
            output_witness,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space,
            offset,
            ..
        } if *output_witness == payload_type
            && *output_space
                == SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: *offset } =>
        {
            Some(SemanticPromotedBindingV1::IndexWitness {
                index_space: *output_space,
                disjoint: true,
                availability: Some(availability),
            })
        }
        SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
            output_witness,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space,
            offset,
            ..
        } if *output_witness == payload_type
            && *output_space
                == SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: *offset } =>
        {
            Some(SemanticPromotedBindingV1::IndexWitness {
                index_space: *output_space,
                disjoint: true,
                availability: Some(availability),
            })
        }
        SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader }
            if *grid_leader == payload_type =>
        {
            Some(SemanticPromotedBindingV1::GridLeader { availability })
        }
        _ => None,
    }
}

fn option_payload_type_v1(
    types: &[SemanticTypeDeclV1],
    option_type: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    let SemanticTypeShapeV1::Enum { variants, .. } =
        types.get(option_type.index() as usize)?.shape()
    else {
        return None;
    };
    let [none, some] = &**variants else {
        return None;
    };
    if none.discriminant() != 0
        || some.discriminant() != 1
        || none.is_uninhabited()
        || some.is_uninhabited()
        || !none.fields().fields().is_empty()
    {
        return None;
    }
    let [payload] = some.fields().fields() else {
        return None;
    };
    Some(*payload)
}

fn exact_option_payload_projection_v1(
    types: &[SemanticTypeDeclV1],
    option_type: SemanticTypeIdV1,
    projections: &[SemanticProjectionV1],
    result_type: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    let [downcast, field] = projections else {
        return None;
    };
    if downcast.kind() != SemanticProjectionKindV1::Downcast(1)
        || field.kind() != SemanticProjectionKindV1::Field(0)
    {
        return None;
    }
    let payload_type = option_payload_type_v1(types, option_type)?;
    // rustc's `PlaceTy` retains the enum type across a downcast and records the
    // active variant separately. Requiring a synthetic aggregate type here
    // rejects the canonical `((_option as Some).0)` projection produced by MIR.
    if downcast.result_type() != option_type
        || field.result_type() != payload_type
        || result_type != payload_type
    {
        return None;
    }
    Some(payload_type)
}

fn optionalized_capability_binding_v1(
    binding: SemanticPromotedBindingV1,
    availability: SemanticOptionAvailabilityV1,
) -> Option<SemanticPromotedBindingV1> {
    match binding {
        SemanticPromotedBindingV1::IndexWitness {
            index_space,
            disjoint,
            availability: Some(SemanticCapabilityAvailabilityV1::Option(actual_availability)),
            ..
        } if actual_availability == availability => {
            Some(SemanticPromotedBindingV1::OptionIndexWitness {
                index_space,
                disjoint,
                availability,
            })
        }
        SemanticPromotedBindingV1::GridLeader {
            availability: SemanticCapabilityAvailabilityV1::Option(actual_availability),
        } if actual_availability == availability => {
            Some(SemanticPromotedBindingV1::OptionGridLeader { availability })
        }
        _ => None,
    }
}

fn optional_pointer_binding_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    operation: &SemanticCompilerIntrinsicOperationV1,
    payload_type: SemanticTypeIdV1,
    availability: SemanticOptionAvailabilityV1,
) -> Option<SemanticPromotedBindingV1> {
    let (disjoint_slice, element) = match operation {
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
            disjoint_slice,
            element,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
            disjoint_slice,
            element,
            ..
        } => (*disjoint_slice, *element),
        _ => return None,
    };
    let (authenticated_element, _, access) = disjoint_slice_descriptor(callables, disjoint_slice)?;
    if authenticated_element != element {
        return None;
    }
    let SemanticTypeShapeV1::Pointer(pointer) = types.get(payload_type.index() as usize)?.shape()
    else {
        return None;
    };
    if pointer.pointee() != element
        || pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
    {
        return None;
    }
    let Type::Scalar(element) = lower_scalar_type(types, element).ok()? else {
        return None;
    };
    Some(SemanticPromotedBindingV1::OptionPointer {
        element,
        address_space: AddressSpace::Global,
        access,
        availability,
    })
}

#[derive(Clone, Copy)]
enum SemanticCapabilityDefinitionV1<'a> {
    Assignment(&'a SemanticRvalueKindV1),
    Call(&'a SemanticDirectCallV1),
}

#[derive(Clone, Copy)]
enum SemanticTransparentDefinitionV1 {
    Borrow {
        source: u32,
        referent_type: SemanticTypeIdV1,
    },
    ValueAlias {
        source: u32,
        source_type: SemanticTypeIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SemanticCapabilityOriginNodeV1 {
    Local(u32, u32),
    EnumPayload(u32, u32, u32, u32),
}

fn merge_capability_origin_v1(
    binding: &mut Option<SemanticPromotedBindingV1>,
    candidate: SemanticPromotedBindingV1,
) -> bool {
    if binding.is_some_and(|existing| existing != candidate) {
        return false;
    }
    *binding = Some(candidate);
    true
}

struct SemanticCapabilityOriginResolverV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    function: &'a SemanticFunctionDeclV1,
    option_dominance: &'a SemanticOptionDominanceV1,
    certified_locals: &'a BTreeSet<u32>,
    definitions: BTreeMap<u32, Vec<SemanticCapabilityDefinitionV1<'a>>>,
    invalidated_locals: BTreeSet<u32>,
    memo: BTreeMap<SemanticCapabilityOriginNodeV1, Option<SemanticPromotedBindingV1>>,
    visiting: BTreeSet<SemanticCapabilityOriginNodeV1>,
    work_limit: usize,
    storage_limit: usize,
    work: usize,
    storage: usize,
    peak_storage: usize,
}

impl<'a> SemanticCapabilityOriginResolverV1<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        option_dominance: &'a SemanticOptionDominanceV1,
        certified_locals: &'a BTreeSet<u32>,
        max_analysis_work: usize,
        max_analysis_storage: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut resolver = Self {
            types,
            callables,
            function,
            option_dominance,
            certified_locals,
            definitions: BTreeMap::new(),
            invalidated_locals: BTreeSet::new(),
            memo: BTreeMap::new(),
            visiting: BTreeSet::new(),
            work_limit: max_analysis_work,
            storage_limit: max_analysis_storage,
            work: 0,
            storage: 0,
            peak_storage: 0,
        };
        for block in function.blocks() {
            resolver.charge_work(1)?;
            for statement in block.statements() {
                resolver.charge_work(1)?;
                resolver.index_statement(statement.kind())?;
            }
            resolver.charge_work(1)?;
            resolver.index_terminator(block.terminator().kind())?;
        }
        Ok(resolver)
    }

    fn push_definition(
        &mut self,
        local: u32,
        definition: SemanticCapabilityDefinitionV1<'a>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let amount = 1usize
            .checked_add(usize::from(!self.definitions.contains_key(&local)))
            .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual: usize::MAX,
                limit: self.storage_limit,
            })?;
        self.charge_storage(amount)?;
        self.definitions.entry(local).or_default().push(definition);
        Ok(())
    }

    fn invalidate_local(&mut self, local: u32) -> Result<(), ProductionSemanticKirErrorV1> {
        if !self.invalidated_locals.contains(&local) {
            self.charge_storage(1)?;
            self.invalidated_locals.insert(local);
        }
        Ok(())
    }

    fn index_statement(
        &mut self,
        statement: &'a SemanticStatementKindV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match statement {
            SemanticStatementKindV1::Assign(assignment) => {
                let local = assignment.destination().local().index();
                if assignment.destination().projections().is_empty() {
                    self.push_definition(
                        local,
                        SemanticCapabilityDefinitionV1::Assignment(assignment.value().kind()),
                    )?;
                } else {
                    self.invalidate_local(local)?;
                }
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. } => {
                        self.invalidate_local(place.local().index())?;
                    }
                    _ => {}
                }
            }
            SemanticStatementKindV1::Store(store) => {
                self.invalidate_local(store.destination().local().index())?;
            }
            SemanticStatementKindV1::AtomicRmw(operation) => {
                self.invalidate_local(operation.destination().local().index())?;
                self.invalidate_local(operation.address().local().index())?;
            }
            SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                self.invalidate_local(operation.destination().local().index())?;
                self.invalidate_local(operation.address().local().index())?;
            }
            SemanticStatementKindV1::SetDiscriminant { place, .. }
            | SemanticStatementKindV1::Deinitialize(place) => {
                self.invalidate_local(place.local().index())?;
            }
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Assume(_)
            | SemanticStatementKindV1::Nop => {}
        }
        Ok(())
    }

    fn index_terminator(
        &mut self,
        terminator: &'a SemanticTerminatorKindV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        match terminator {
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(destination) = call.destination() {
                    let local = destination.place().local().index();
                    if destination.place().projections().is_empty() {
                        self.push_definition(local, SemanticCapabilityDefinitionV1::Call(call))?;
                    } else {
                        self.invalidate_local(local)?;
                    }
                }
            }
            SemanticTerminatorKindV1::Drop { place, .. } => {
                self.invalidate_local(place.local().index())?;
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::SwitchInt { .. }
            | SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::Assert { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
        Ok(())
    }

    fn resolve(
        &mut self,
        local: SemanticLocalIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let ty = self
            .function
            .locals()
            .get(local.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .ty();
        self.resolve_local(local, ty)
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.work =
            self.work
                .checked_add(amount)
                .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: usize::MAX,
                    limit: self.work_limit,
                })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisWork,
            self.work,
            self.work_limit,
        )
    }

    fn charge_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        let next = self.storage.checked_add(amount).ok_or(
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual: usize::MAX,
                limit: self.storage_limit,
            },
        )?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            next,
            self.storage_limit,
        )?;
        self.storage = next;
        self.peak_storage = self.peak_storage.max(next);
        Ok(())
    }

    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.storage = self.storage.checked_sub(amount).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "capability SSA provenance storage accounting underflow",
            )
        })?;
        Ok(())
    }

    fn definition_count(&self, local: SemanticLocalIdV1) -> usize {
        self.definitions.get(&local.index()).map_or(0, Vec::len)
    }

    fn definition(
        &self,
        local: SemanticLocalIdV1,
        index: usize,
    ) -> Option<SemanticCapabilityDefinitionV1<'a>> {
        self.definitions
            .get(&local.index())
            .and_then(|definitions| definitions.get(index))
            .copied()
    }

    fn resolve_local(
        &mut self,
        local: SemanticLocalIdV1,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let node = SemanticCapabilityOriginNodeV1::Local(local.index(), expected_type.index());
        self.resolve_node(node, |resolver| {
            if !resolver.certified_locals.contains(&local.index())
                || resolver.invalidated_locals.contains(&local.index())
            {
                return Ok(None);
            }
            let Some(declaration) = resolver.function.locals().get(local.index() as usize) else {
                return Ok(None);
            };
            if declaration.ty() != expected_type {
                return Ok(None);
            }
            let definition_count = resolver.definition_count(local);
            if definition_count == 0 {
                return Ok(None);
            }
            let mut binding = None;
            for index in 0..definition_count {
                resolver.charge_work(1)?;
                let Some(definition) = resolver.definition(local, index) else {
                    return Ok(None);
                };
                let candidate = match definition {
                    SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Use(
                        operand,
                    )) => resolver.resolve_operand(operand, expected_type)?,
                    SemanticCapabilityDefinitionV1::Assignment(_) => None,
                    SemanticCapabilityDefinitionV1::Call(call) => {
                        resolver.resolve_direct_call(local, expected_type, call)
                    }
                };
                let Some(candidate) = candidate else {
                    return Ok(None);
                };
                if !merge_capability_origin_v1(&mut binding, candidate) {
                    return Ok(None);
                }
            }
            Ok(binding)
        })
    }

    fn resolve_operand(
        &mut self,
        operand: &SemanticOperandV1,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let place = match operand {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
            SemanticOperandV1::Constant(_) => return Ok(None),
        };
        self.resolve_place(place, expected_type)
    }

    fn resolve_place(
        &mut self,
        place: &SemanticPlaceV1,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        if place.ty() != expected_type {
            return Ok(None);
        }
        match place.projections() {
            [] => self.resolve_local(place.local(), expected_type),
            [downcast, field] => {
                let SemanticProjectionKindV1::Downcast(variant) = downcast.kind() else {
                    return Ok(None);
                };
                let SemanticProjectionKindV1::Field(field) = field.kind() else {
                    return Ok(None);
                };
                self.resolve_enum_payload(place.local(), variant, field, expected_type)
            }
            _ => Ok(None),
        }
    }

    fn resolve_enum_payload(
        &mut self,
        local: SemanticLocalIdV1,
        variant: u32,
        field: u32,
        expected_type: SemanticTypeIdV1,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        let node = SemanticCapabilityOriginNodeV1::EnumPayload(
            local.index(),
            variant,
            field,
            expected_type.index(),
        );
        self.resolve_node(node, |resolver| {
            if !resolver.certified_locals.contains(&local.index())
                || resolver.invalidated_locals.contains(&local.index())
            {
                return Ok(None);
            }
            let Some(enum_type) = resolver
                .function
                .locals()
                .get(local.index() as usize)
                .map(|declaration| declaration.ty())
            else {
                return Ok(None);
            };
            let Some(SemanticTypeShapeV1::Enum { variants, .. }) = resolver
                .types
                .get(enum_type.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Ok(None);
            };
            if variants
                .get(variant as usize)
                .and_then(|variant| variant.fields().fields().get(field as usize))
                != Some(&expected_type)
            {
                return Ok(None);
            }
            let Some(expected_arity) = variants
                .get(variant as usize)
                .map(|variant| variant.fields().fields().len())
            else {
                return Ok(None);
            };
            let definition_count = resolver.definition_count(local);
            if definition_count == 0 {
                return Ok(None);
            }
            let mut binding = None;
            for index in 0..definition_count {
                resolver.charge_work(1)?;
                let Some(definition) = resolver.definition(local, index) else {
                    return Ok(None);
                };
                let candidate = match definition {
                    SemanticCapabilityDefinitionV1::Assignment(
                        SemanticRvalueKindV1::Aggregate(aggregate),
                    ) => match aggregate.kind() {
                        SemanticAggregateKindV1::EnumVariant(actual)
                            if *actual == variant
                                && aggregate.operands().len() == expected_arity =>
                        {
                            let Some(operand) = aggregate.operands().get(field as usize) else {
                                return Ok(None);
                            };
                            resolver.resolve_operand(operand, expected_type)?
                        }
                        SemanticAggregateKindV1::EnumVariant(_) => continue,
                        SemanticAggregateKindV1::Array
                        | SemanticAggregateKindV1::Tuple
                        | SemanticAggregateKindV1::Aggregate => None,
                    },
                    SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Use(
                        operand,
                    )) => {
                        let place = match operand {
                            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                                if place.ty() == enum_type && place.projections().is_empty() =>
                            {
                                place
                            }
                            SemanticOperandV1::Copy(_)
                            | SemanticOperandV1::Move(_)
                            | SemanticOperandV1::Constant(_) => return Ok(None),
                        };
                        resolver.resolve_enum_payload(
                            place.local(),
                            variant,
                            field,
                            expected_type,
                        )?
                    }
                    SemanticCapabilityDefinitionV1::Assignment(_) => None,
                    SemanticCapabilityDefinitionV1::Call(call) => resolver
                        .resolve_option_payload_call(
                            local,
                            enum_type,
                            variant,
                            field,
                            expected_type,
                            call,
                        ),
                };
                let Some(candidate) = candidate else {
                    return Ok(None);
                };
                if !merge_capability_origin_v1(&mut binding, candidate) {
                    return Ok(None);
                }
            }
            Ok(binding)
        })
    }

    fn resolve_option_payload_call(
        &self,
        local: SemanticLocalIdV1,
        enum_type: SemanticTypeIdV1,
        variant: u32,
        field: u32,
        payload_type: SemanticTypeIdV1,
        call: &SemanticDirectCallV1,
    ) -> Option<SemanticPromotedBindingV1> {
        if variant != 1
            || field != 0
            || option_payload_type_v1(self.types, enum_type) != Some(payload_type)
        {
            return None;
        }
        let availability = self.option_dominance.availability(local)?;
        let operation = self.compiler_intrinsic(call)?;
        option_capability_contract_v1(operation, payload_type, availability).or_else(|| {
            option_component_witness_contract_v1(operation, payload_type).map(|index_space| {
                SemanticPromotedBindingV1::ComponentWitness {
                    index_space,
                    availability: SemanticCapabilityAvailabilityV1::Option(availability),
                }
            })
        })
    }

    fn resolve_direct_call(
        &self,
        local: SemanticLocalIdV1,
        result_type: SemanticTypeIdV1,
        call: &SemanticDirectCallV1,
    ) -> Option<SemanticPromotedBindingV1> {
        let operation = self.compiler_intrinsic(call)?;
        if let (Some(availability), Some(payload_type)) = (
            self.option_dominance.availability(local),
            option_payload_type_v1(self.types, result_type),
        ) {
            return option_capability_contract_v1(operation, payload_type, availability)
                .and_then(|binding| optionalized_capability_binding_v1(binding, availability))
                .or_else(|| {
                    option_component_witness_contract_v1(operation, payload_type).map(
                        |index_space| SemanticPromotedBindingV1::OptionComponentWitness {
                            index_space,
                            availability,
                        },
                    )
                })
                .or_else(|| {
                    optional_pointer_binding_v1(
                        self.types,
                        self.callables,
                        operation,
                        payload_type,
                        availability,
                    )
                });
        }
        direct_index_witness_contract_v1(operation, result_type)
    }

    fn transparent_definition(
        &self,
        local: SemanticLocalIdV1,
    ) -> Option<SemanticTransparentDefinitionV1> {
        if self.invalidated_locals.contains(&local.index()) || self.definition_count(local) != 1 {
            return None;
        }
        match self.definition(local, 0)? {
            SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Borrow {
                place,
                ..
            }) if place.projections().is_empty() => Some(SemanticTransparentDefinitionV1::Borrow {
                source: place.local().index(),
                referent_type: place.ty(),
            }),
            SemanticCapabilityDefinitionV1::Assignment(SemanticRvalueKindV1::Use(
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
            )) if place.projections().is_empty() => {
                Some(SemanticTransparentDefinitionV1::ValueAlias {
                    source: place.local().index(),
                    source_type: place.ty(),
                })
            }
            SemanticCapabilityDefinitionV1::Assignment(_)
            | SemanticCapabilityDefinitionV1::Call(_) => None,
        }
    }

    fn compiler_intrinsic(
        &self,
        call: &SemanticDirectCallV1,
    ) -> Option<&SemanticCompilerIntrinsicOperationV1> {
        match self.callables.get(call.callee().index() as usize)? {
            SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } => Some(operation),
            SemanticCallableDeclV1::Defined { .. }
            | SemanticCallableDeclV1::DeviceFfiImport { .. } => None,
        }
    }

    fn resolve_node(
        &mut self,
        node: SemanticCapabilityOriginNodeV1,
        resolve: impl FnOnce(
            &mut Self,
        )
            -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1>,
    ) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
        self.charge_work(1)?;
        if let Some(cached) = self.memo.get(&node) {
            return Ok(*cached);
        }
        if self.visiting.contains(&node) {
            return Ok(None);
        }
        self.charge_storage(1)?;
        self.visiting.insert(node);
        let result = resolve(self);
        self.visiting.remove(&node);
        self.release_storage(1)?;
        let result = result?;
        self.charge_storage(1)?;
        self.memo.insert(node, result);
        Ok(result)
    }
}

#[cfg(test)]
fn promoted_capability_binding_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    option_dominance: &SemanticOptionDominanceV1,
    certified_locals: &BTreeSet<u32>,
    local: u32,
) -> Result<Option<SemanticPromotedBindingV1>, ProductionSemanticKirErrorV1> {
    if function.locals().get(local as usize).is_none() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    SemanticCapabilityOriginResolverV1::new(
        types,
        callables,
        function,
        option_dominance,
        certified_locals,
        usize::MAX,
        usize::MAX,
    )?
    .resolve(SemanticLocalIdV1::from_index(local))
}
