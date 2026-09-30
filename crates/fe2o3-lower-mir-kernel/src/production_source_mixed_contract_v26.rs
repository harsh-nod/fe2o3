// Producers are retained nominal source-checked handoffs, including final LICM.
// Encoded bytes are descriptive and never replace that owner or its proof.
mod mixed_source_contract_v26 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionCoordinateV1 as SliceDefinition,
        CanonicalKirOperationCoordinateV1 as SliceOperation,
    };
    mod wire {
        pub(super) use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
        pub(super) use fe2o3_kernel_descriptor::*;
    }
    use wire::{MixedArgumentV26, MixedOccurrenceV26, MixedScalarV26};

    trait ContractOccurrence {
        fn premise_index(&self) -> usize;
        fn original_instance(&self) -> usize;
        fn original_operation(&self) -> SliceOperation;
        fn output_operation(&self) -> SliceOperation;
        fn original_address_formation(&self) -> SliceOperation;
        fn output_address_formation(&self) -> SliceOperation;
        fn output_address_index(&self) -> SliceDefinition;
        fn output_guard_edge(&self) -> fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1;
        fn output_guard_condition(&self) -> SliceDefinition;
        fn domain(&self) -> fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26;
        fn invocation_projection(&self) -> Option<(Axis, ValueId)>;
        fn memory_access(&self) -> MemoryAccess;
    }
    macro_rules! contract_occurrence {
        ($ty:ty) => {
            impl ContractOccurrence for $ty {
                fn premise_index(&self) -> usize {
                    <$ty>::premise_index(self)
                }
                fn original_instance(&self) -> usize {
                    <$ty>::original_instance(self)
                }
                fn original_operation(&self) -> SliceOperation {
                    <$ty>::original_operation(self)
                }
                fn output_operation(&self) -> SliceOperation {
                    <$ty>::output_operation(self)
                }
                fn original_address_formation(&self) -> SliceOperation {
                    <$ty>::original_address_formation(self)
                }
                fn output_address_formation(&self) -> SliceOperation {
                    <$ty>::output_address_formation(self)
                }
                fn output_address_index(&self) -> SliceDefinition {
                    <$ty>::output_address_index(self)
                }
                fn output_guard_edge(&self) -> fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1 {
                    <$ty>::output_guard_edge(self)
                }
                fn output_guard_condition(&self) -> SliceDefinition {
                    <$ty>::output_guard_condition(self)
                }
                fn domain(&self) -> fe2o3_kernel_ir::CanonicalConditionalSliceDomainV26 {
                    <$ty>::domain(self)
                }
                fn invocation_projection(&self) -> Option<(Axis, ValueId)> {
                    <$ty>::invocation_projection(self)
                }
                fn memory_access(&self) -> MemoryAccess {
                    <$ty>::memory_access(self)
                }
            }
        };
    }
    contract_occurrence!(ProductionMixedRuntimeOccurrenceV26);
    contract_occurrence!(ProductionMixedLicmRuntimeOccurrenceV28);

    struct ContractParts<'view, 'source, R> {
        source: &'view ProductionSourceOwnedViewV18<'source>,
        graph: &'view fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
        premises: &'view [ProductionMixedSliceRuntimePremiseV26],
        occurrences: &'view [R],
        launches: &'view [fe2o3_kernel_ir::ExplicitLaunchExtent],
        width: fe2o3_kernel_ir::FormalIndexWidth,
    }
    // Private to this emitter: serialization cannot confer a new owner authority.
    trait ContractOwner {
        type Occurrence: ContractOccurrence;
        fn parts(
            &self,
            budget: &ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<ContractParts<'_, '_, Self::Occurrence>>;
        fn observe(
            &self,
            required: usize,
            budget: &ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()>;
    }
    macro_rules! contract_prefix_owner {
        ($ty:ty) => {
            impl ContractOwner for $ty {
                type Occurrence = ProductionMixedRuntimeOccurrenceV26;
                fn parts(
                    &self,
                    budget: &ArgumentBudgetV1<'_>,
                ) -> SourceOwnedResultV18<ContractParts<'_, '_, Self::Occurrence>> {
                    self.owned.check(budget)?;
                    Ok(ContractParts {
                        source: self.owned.source,
                        graph: self.output(budget)?.owner(),
                        premises: &self.premises,
                        occurrences: &self.occurrences,
                        launches: self.launches,
                        width: self.width,
                    })
                }
                fn observe(
                    &self,
                    required: usize,
                    budget: &ArgumentBudgetV1<'_>,
                ) -> SourceOwnedResultV18<()> {
                    self.observe_retained_storage_v18(required, budget)
                }
            }
        };
    }
    contract_prefix_owner!(ProductionConditionalMixedOutputHandoffV26<'_, '_>);
    contract_prefix_owner!(ProductionConditionalMixedPureCseOutputHandoffV26<'_, '_>);
    impl ContractOwner for ProductionConditionalMixedLicmOutputHandoffV28<'_, '_, '_, '_> {
        type Occurrence = ProductionMixedLicmRuntimeOccurrenceV28;
        fn parts(
            &self,
            budget: &ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<ContractParts<'_, '_, Self::Occurrence>> {
            let source = self.relocation(budget)?.prefix(budget)?.owned.source;
            let (launches, width) = self.launch_context(budget)?;
            Ok(ContractParts {
                source,
                graph: self.output(budget)?,
                premises: self.runtime_premises(budget)?,
                occurrences: self.runtime_occurrences(budget)?,
                launches,
                width,
            })
        }
        fn observe(
            &self,
            required: usize,
            budget: &ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()> {
            self.observe_retained_storage_v28(required, budget)
        }
    }

    fn mismatch() -> ProductionSourceOwnedViewErrorV18 {
        ProductionSourceOwnedViewErrorV18::Binding(
            "mixed source contract descriptor or occurrence differs",
        )
    }
    fn descriptor_error(
        error: wire::DescriptorWireErrorV3<ArgumentResourceV1>,
    ) -> ProductionSourceOwnedViewErrorV18 {
        match error {
            wire::DescriptorWireErrorV3::Work(error) => error.into(),
            _ => mismatch(),
        }
    }
    fn codec_error(
        error: wire::MixedContractErrorV26<ArgumentResourceV1>,
    ) -> ProductionSourceOwnedViewErrorV18 {
        match error {
            wire::MixedContractErrorV26::Resource(error) => error.into(),
            wire::MixedContractErrorV26::Invalid(_) => mismatch(),
        }
    }
    fn ordinal(value: usize) -> SourceOwnedResultV18<u32> {
        value
            .try_into()
            .map_err(|_| ArgumentResourceV1::Arithmetic.into())
    }
    fn scalar(value: ScalarType) -> MixedScalarV26 {
        match value {
            ScalarType::Bool => MixedScalarV26::Bool,
            ScalarType::I8 => MixedScalarV26::I8,
            ScalarType::I16 => MixedScalarV26::I16,
            ScalarType::I32 => MixedScalarV26::I32,
            ScalarType::I64 => MixedScalarV26::I64,
            ScalarType::I128 => MixedScalarV26::I128,
            ScalarType::U8 => MixedScalarV26::U8,
            ScalarType::U16 => MixedScalarV26::U16,
            ScalarType::U32 => MixedScalarV26::U32,
            ScalarType::U64 => MixedScalarV26::U64,
            ScalarType::U128 => MixedScalarV26::U128,
            ScalarType::Index => MixedScalarV26::Index,
            ScalarType::F16 => MixedScalarV26::F16,
            ScalarType::Bf16 => MixedScalarV26::Bf16,
            ScalarType::F32 => MixedScalarV26::F32,
            ScalarType::F64 => MixedScalarV26::F64,
        }
    }
    fn operation(value: SliceOperation) -> wire::MixedOperationV26 {
        wire::MixedOperationV26 {
            function: value.block.function.0,
            block: value.block.block,
            operation: value.operation,
        }
    }
    fn definition(value: SliceDefinition) -> wire::MixedDefinitionV26 {
        match value {
            SliceDefinition::FunctionArgument { function, argument } => {
                wire::MixedDefinitionV26::FunctionArgument {
                    function: function.0,
                    argument,
                }
            }
            SliceDefinition::BlockArgument { block, argument } => {
                wire::MixedDefinitionV26::BlockArgument {
                    function: block.function.0,
                    block: block.block,
                    argument,
                }
            }
            SliceDefinition::Result {
                operation: at,
                result,
            } => wire::MixedDefinitionV26::Result {
                operation: operation(at),
                result,
            },
        }
    }
    fn axis(value: fe2o3_kernel_ir::Axis) -> u8 {
        match value {
            fe2o3_kernel_ir::Axis::X => 0,
            fe2o3_kernel_ir::Axis::Y => 1,
            fe2o3_kernel_ir::Axis::Z => 2,
        }
    }

    fn check_argument(
        source_kind: wire::SourceTypeDescriptorV3,
        expected: &wire::LogicalArgumentV1,
        generated: &wire::LogicalArgumentRefV3<'_, '_>,
        table: &wire::DeviceDescriptorTableV3<'_>,
        generated_field: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        budget.charge_work(128)?;
        // The V3 decoder validates every record with wire_v3::validate_argument:
        // layout is the canonical source lowering and component access/alias
        // are fixed by logical semantics and kind. Names are descriptive labels,
        // not part of the captured original physical ABI; the descriptor subject
        // binds them for the generated packer's separate name/field check.
        let source = table
            .source_type(generated.source_type(), &mut |n| budget.charge_work(n))
            .map_err(descriptor_error)?;
        if usize::from(generated.source_index()) != generated_field
            || source.descriptor() != source_kind
            || generated.source_type() != expected.source_type()
            || generated.device_layout() != expected.device_layout()
            || generated.ownership() != expected.ownership()
            || generated.access() != expected.access()
            || generated.alias() != expected.alias()
            || generated.component_count() != expected.physical_components().len()
        {
            return Err(mismatch());
        }
        for (index, (kind, offset, size, alignment)) in expected.physical_components().enumerate() {
            budget.charge_work(8)?;
            let actual = generated
                .component(index, &mut |n| budget.charge_work(n))
                .map_err(descriptor_error)?;
            if (actual.kind, actual.offset, actual.size, actual.alignment)
                != (kind, offset, size, alignment)
            {
                return Err(mismatch());
            }
        }
        Ok(())
    }

    fn occurrence(
        row: &impl ContractOccurrence,
        argument: u16,
        function: &fe2o3_kernel_ir::Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<MixedOccurrenceV26> {
        use fe2o3_kernel_ir::{
            CanonicalConditionalSliceDomainV26 as Domain, FormalGuardedPathV1 as Path,
        };
        budget.charge_work(128)?;
        let domain = row.domain();
        let (guard_index, length, predicate, path) = match domain {
            Domain::Read(d) => (d.guard_index(), d.length(), d.predicate(), d.path()),
            Domain::Store(d) => (d.guard_index(), d.length(), d.predicate(), d.path()),
        };
        let edge = row.output_guard_edge();
        let projection = row.invocation_projection();
        let formation = row.output_address_formation();
        let body = function.body.as_ref().ok_or_else(mismatch)?;
        let formation_block = body
            .blocks
            .get(formation.block.block as usize)
            .ok_or_else(mismatch)?;
        if formation_block
            .operations
            .get(formation.operation as usize)
            .is_none()
        {
            return Err(mismatch());
        }
        let guarded_formation =
            matches!(path, Path::TrueEdge { target, .. } if target == formation_block.id);
        // A dereference guard is not an address-formation guard. A direct global
        // invocation index is bounded independently; otherwise retain its whole
        // unsigned domain unless formation itself is in the proven true block.
        let formation_envelope = if guarded_formation {
            wire::MixedIndexEnvelopeV26::LogicalExtent { argument }
        } else if let Some((projection, _)) = projection {
            wire::MixedIndexEnvelopeV26::InvocationAxis {
                axis: axis(projection),
            }
        } else {
            let ty = match row.output_address_index() {
                SliceDefinition::FunctionArgument { argument, .. } => {
                    function.signature.parameters.get(argument as usize)
                }
                SliceDefinition::BlockArgument { block, argument } => body
                    .blocks
                    .get(block.block as usize)
                    .and_then(|b| b.parameters.get(argument as usize))
                    .map(|v| &v.ty),
                SliceDefinition::Result { operation, result } => body
                    .blocks
                    .get(operation.block.block as usize)
                    .and_then(|b| b.operations.get(operation.operation as usize))
                    .and_then(|op| op.results.get(result as usize))
                    .map(|v| &v.ty),
            }
            .ok_or_else(mismatch)?;
            let bits = match ty {
                Type::Scalar(ScalarType::U8) => 8,
                Type::Scalar(ScalarType::U16) => 16,
                Type::Scalar(ScalarType::U32) => 32,
                Type::Scalar(ScalarType::U64 | ScalarType::Index) => 64,
                _ => return Err(mismatch()),
            };
            wire::MixedIndexEnvelopeV26::UnsignedWidth { bits }
        };
        let memory = row.memory_access();
        // The retained native/source occurrence already joins this physical
        // pointer to its exact Global slice domain. Preserve the access's real
        // representation; Generic by itself never establishes that domain.
        let address_space = match memory.address_space {
            AddressSpace::Global => wire::MixedMemorySpaceV26::Global,
            AddressSpace::Generic => wire::MixedMemorySpaceV26::Generic,
            _ => return Err(mismatch()),
        };
        Ok(MixedOccurrenceV26 {
            argument,
            original_instance: ordinal(row.original_instance())?,
            original_operation: operation(row.original_operation()),
            output_operation: operation(row.output_operation()),
            original_formation: operation(row.original_address_formation()),
            output_formation: operation(formation),
            output_address_index: definition(row.output_address_index()),
            output_guard_edge: wire::MixedEdgeV26 {
                function: edge.source.function.0,
                block: edge.source.block,
                successor: edge.successor,
            },
            output_guard_condition: definition(row.output_guard_condition()),
            slice_value: domain.slice().0,
            pointer_value: domain.pointer().0,
            index_value: domain.index().0,
            guard_index_value: guard_index.0,
            length_value: length.0,
            predicate_value: predicate.0,
            path: match path {
                Path::ExplicitPredicate => wire::MixedGuardPathV26::ExplicitPredicate,
                Path::TrueEdge {
                    source,
                    ordinal,
                    target,
                } => wire::MixedGuardPathV26::TrueEdge {
                    source: source.0,
                    successor: ordinal
                        .try_into()
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    target: target.0,
                },
            },
            element_bytes: domain.element_bytes(),
            alignment: memory.alignment,
            address_space,
            writing: domain.writing(),
            volatile: memory.volatile,
            invocation_axis: projection.map_or(255, |(projection, _)| axis(projection)),
            invocation_value: projection.map_or(0, |(_, value)| value.0),
            access_envelope: wire::MixedIndexEnvelopeV26::LogicalExtent { argument },
            formation_envelope,
        })
    }

    fn emit_contract<H: ContractOwner>(
        owner: &H,
        original_root: usize,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        table: &wire::DeviceDescriptorTableV3<'_>,
        descriptor_kernel: usize,
        output: &mut [u8],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        let parts = owner.parts(budget)?;
        let source = parts.source;
        let floor = budget.storage();
        source.retain_query(scoped_source_attempt_v29(
            source.cleanup,
            budget,
            floor,
            |budget| {
                let scratch_floor = budget.storage();
                let headers = argument_sum_v1(&[
                    wire::DESCRIPTOR_QUERY_STORAGE_V3,
                    wire::MIXED_CONTRACT_CODEC_STORAGE_V26,
                    size_of::<[Vec<usize>; 4]>(),
                    size_of::<[MixedArgumentV26; 4]>(),
                    size_of::<[MixedOccurrenceV26; 4]>(),
                    size_of::<[usize; 64]>(),
                    size_of::<ContractParts<'_, '_, H::Occurrence>>(),
                    output.len(),
                ])?;
                budget.reserve_storage(headers)?;
                budget.charge_work(512)?;
                source.require_kernel_argument_abi_v18(
                    ProductionKernelArgumentAbiInputV18 { roots: abi.roots },
                    budget,
                )?;
                let root = abi.roots.get(original_root).ok_or_else(mismatch)?;
                let (_, original_function) = source.root(original_root, budget)?;
                let original = source.canonical(budget)?;
                let original_function = original
                    .module()
                    .functions
                    .get(original_function)
                    .ok_or_else(mismatch)?;
                let graph = parts.graph;
                let mut selected = None;
                for (index, function) in graph.module().functions.iter().enumerate() {
                    budget.charge_work(argument_sum_v1(&[
                        1,
                        function.id.as_str().len(),
                        original_function.id.as_str().len(),
                    ])?)?;
                    if function.id == original_function.id {
                        if selected.replace((index, function)).is_some() {
                            return Err(mismatch());
                        }
                    }
                }
                let (function_index, function) = selected.ok_or_else(mismatch)?;
                let launch = *parts.launches.get(original_root).ok_or_else(mismatch)?;
                let fe2o3_kernel_ir::ExplicitLaunchExtent::Exact { rank, extents } = launch else {
                    return Err(mismatch());
                };
                let width = match parts.width {
                    fe2o3_kernel_ir::FormalIndexWidth::Bits32 => 32,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64 => 64,
                    fe2o3_kernel_ir::FormalIndexWidth::Unknown => return Err(mismatch()),
                };
                let kernel = table
                    .kernel(descriptor_kernel, &mut |n| budget.charge_work(n))
                    .map_err(descriptor_error)?;
                let source_launch = source
                    .source_launch(budget)?
                    .roots()
                    .get(original_root)
                    .ok_or_else(mismatch)?
                    .source_launch();
                let grid = kernel.launch().max_grid();
                let block_matches = match (
                    source_launch.exact_workgroup(),
                    kernel.launch().block_size(),
                ) {
                    (Some(expected), wire::BlockSizeV1::Exact(actual)) => {
                        expected == [actual.x(), actual.y(), actual.z()]
                    }
                    (None, wire::BlockSizeV1::Any) => true,
                    _ => false,
                };
                budget.charge_work(argument_sum_v1(&[
                    64,
                    root.export.len(),
                    kernel.entry_name().len(),
                ])?)?;
                if kernel.kernel_id().as_bytes() != root.kernel_binding
                    || kernel.entry_name() != root.export
                    || kernel.launch().rank() != rank
                    || source_launch.rank() != rank
                    || !block_matches
                    || source_launch.max_grid() != [grid.x(), grid.y(), grid.z()]
                    || kernel.abi_layout().explicit_argument_size() != root.explicit_argument_bytes
                    || kernel.abi_layout().kernarg_segment_alignment()
                        != root.kernarg_alignment_bytes
                {
                    return Err(mismatch());
                }
                let mut selected =
                    source_reference_emission_vec_v29(wire::MAX_MIXED_ARGUMENTS_V26, budget)
                        .map_err(source_argument_error_v18)?;
                for premise in parts.premises {
                    budget.charge_work(3)?;
                    if premise.root() == original_root {
                        if selected.len() == wire::MAX_MIXED_ARGUMENTS_V26 {
                            return Err(mismatch());
                        }
                        selected.push(premise);
                    }
                }
                budget.charge_work(argument_product_v1(
                    selected.len(),
                    selected
                        .len()
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                )?)?;
                selected.sort_unstable_by_key(|p| p.original_argument());
                let mut arguments = source_reference_emission_vec_v29(selected.len(), budget)
                    .map_err(source_argument_error_v18)?;
                let mut fields = kernel.arguments();
                let mut generated_field = 0usize;
                for (source_argument, input) in root.arguments.iter().enumerate() {
                    budget.charge_work(8)?;
                    let (source_kind, expected) = match &input.kind {
                        ProductionKernelArgumentAbiKindV18::Descriptor { source, argument } => {
                            (*source, argument)
                        }
                        ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { .. } => {
                            if !matches!(
                                source.kernel_argument_by_value_abi_v29(
                                    original_root,
                                    source_argument,
                                    budget
                                )?,
                                Some(ProductionKernelByValueAbiV29::Ignored(_))
                            ) {
                                return Err(mismatch());
                            }
                            continue;
                        }
                    };
                    let generated = fields
                        .next(&mut |n| budget.charge_work(n))
                        .map_err(descriptor_error)?
                        .ok_or_else(mismatch)?;
                    check_argument(
                        source_kind,
                        expected,
                        &generated,
                        table,
                        generated_field,
                        budget,
                    )?;
                    if let Some(premise) = selected
                        .get(arguments.len())
                        .filter(|p| p.original_argument() as usize == source_argument)
                    {
                        let SliceDefinition::FunctionArgument {
                            function: owner,
                            argument: physical_parameter,
                        } = premise.parameter()
                        else {
                            return Err(mismatch());
                        };
                        if owner.0 as usize != function_index
                            || premise.launch() != launch
                            || premise.index_width() != parts.width
                            || premise.source_identity() != input.semantic_type_identity
                            || !matches!(
                                source_kind,
                                wire::SourceTypeDescriptorV3::SharedSlice(_)
                                    | wire::SourceTypeDescriptorV3::DisjointSlice(_)
                            )
                            || generated.component_count() != 2
                        {
                            return Err(mismatch());
                        }
                        let pointer = generated
                            .component(0, &mut |n| budget.charge_work(n))
                            .map_err(descriptor_error)?;
                        let length = generated
                            .component(1, &mut |n| budget.charge_work(n))
                            .map_err(descriptor_error)?;
                        if pointer.kind != wire::PhysicalAbiComponentKind::GlobalPointer
                            || length.kind != wire::PhysicalAbiComponentKind::SliceLengthU64
                        {
                            return Err(mismatch());
                        }
                        let counts = premise.access_counts();
                        budget.charge_work(256)?;
                        arguments.push(MixedArgumentV26 {
                            source_argument: ordinal(source_argument)?,
                            generated_field: generated_field
                                .try_into()
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            physical_parameter,
                            semantic_type: premise.source_type().index(),
                            semantic_type_identity: *premise.source_identity().as_bytes(),
                            descriptor_type_identity: *generated.source_type().as_bytes(),
                            device_layout_identity: *generated.device_layout().as_bytes(),
                            scalar: scalar(premise.scalar()),
                            pointer_offset: pointer.offset,
                            length_offset: length.offset,
                            reads: ordinal(counts[0])?,
                            writes: ordinal(counts[1])?,
                            source_exclusive: premise.source_exclusive_contract(),
                        });
                    } else if matches!(
                        source_kind,
                        wire::SourceTypeDescriptorV3::SharedSlice(_)
                            | wire::SourceTypeDescriptorV3::DisjointSlice(_)
                    ) {
                        // Even an unused slice needs an explicit completed
                        // source/native zero-count premise, not absence.
                        return Err(mismatch());
                    }
                    generated_field = generated_field
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                if arguments.len() != selected.len()
                    || generated_field != kernel.argument_count()
                    || fields
                        .next(&mut |n| budget.charge_work(n))
                        .map_err(descriptor_error)?
                        .is_some()
                {
                    return Err(mismatch());
                }
                let mut occurrences =
                    source_reference_emission_vec_v29(wire::MAX_MIXED_OCCURRENCES_V26, budget)
                        .map_err(source_argument_error_v18)?;
                for row in parts.occurrences {
                    budget.charge_work(3)?;
                    let premise = parts
                        .premises
                        .get(row.premise_index())
                        .ok_or_else(mismatch)?;
                    if premise.root() != original_root {
                        continue;
                    }
                    if occurrences.len() == wire::MAX_MIXED_OCCURRENCES_V26 {
                        return Err(mismatch());
                    }
                    budget.charge_work(wire::MAX_MIXED_ARGUMENTS_V26)?;
                    let argument = arguments
                        .binary_search_by_key(&premise.original_argument(), |a| a.source_argument)
                        .map_err(|_| mismatch())?;
                    occurrences.push(occurrence(
                        row,
                        argument
                            .try_into()
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        function,
                        budget,
                    )?);
                }
                let subjects = wire::MixedContractSubjectsV26 {
                    kernel_id: *root.kernel_binding,
                    source_semantic_identity: *source.source_ssa(budget)?.source_semantic_sha256(),
                    original_graph_identity: *original.identity().digest(),
                    output_graph_identity: *graph.identity().digest(),
                    descriptor_identity: wire::mixed_descriptor_subject_v26(table, &mut |n| {
                        budget.charge_work(n)
                    })
                    .map_err(codec_error)?,
                    original_root: ordinal(original_root)?,
                    output_function: ordinal(function_index)?,
                    source_rank: rank,
                    index_width: width,
                    exact_grid: extents,
                    source_argument_count: ordinal(root.arguments.len())?,
                    generated_field_count: ordinal(generated_field)?,
                    explicit_argument_bytes: root.explicit_argument_bytes,
                    kernarg_alignment: root.kernarg_alignment_bytes,
                };
                let input = wire::MixedContractInputV26 {
                    subjects,
                    arguments: &arguments,
                    occurrences: &occurrences,
                };
                let len =
                    wire::encoded_mixed_contract_v26_len(&input, &mut |n| budget.charge_work(n))
                        .map_err(codec_error)?;
                let output = output.get_mut(..len).ok_or_else(mismatch)?;
                owner.parts(budget)?;
                wire::encode_mixed_contract_v26(&input, output, &mut |n| budget.charge_work(n))
                    .map_err(codec_error)?;
                drop(input);
                drop(occurrences);
                drop(arguments);
                drop(selected);
                drop(fields);
                drop(kernel);
                owner.observe(scratch_floor, budget)?;
                let scratch = budget
                    .storage()
                    .checked_sub(scratch_floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                budget.release_storage(scratch)?;
                Ok(len)
            },
        ))
    }
    macro_rules! mixed_source_contract_emitter_v26 {
        ($handoff:ty) => {
            impl $handoff {
                /// Emits a complete descriptor-bound runtime contract from the
                /// genuine retained final graph. Bytes remain inert and cannot
                /// replace the owner, refinement proof or runtime discharge.
                pub fn emit_mixed_contract_v26(
                    &self,
                    original_root: usize,
                    abi: ProductionKernelArgumentAbiInputV18<'_>,
                    table: &wire::DeviceDescriptorTableV3<'_>,
                    descriptor_kernel: usize,
                    output: &mut [u8],
                    budget: &mut ArgumentBudgetV1<'_>,
                ) -> SourceOwnedResultV18<usize> {
                    emit_contract(
                        self,
                        original_root,
                        abi,
                        table,
                        descriptor_kernel,
                        output,
                        budget,
                    )
                }
            }
        };
    }

    mixed_source_contract_emitter_v26!(ProductionConditionalMixedOutputHandoffV26<'_, '_>);
    mixed_source_contract_emitter_v26!(ProductionConditionalMixedPureCseOutputHandoffV26<'_, '_>);
    mixed_source_contract_emitter_v26!(
        ProductionConditionalMixedLicmOutputHandoffV28<'_, '_, '_, '_>
    );
}
