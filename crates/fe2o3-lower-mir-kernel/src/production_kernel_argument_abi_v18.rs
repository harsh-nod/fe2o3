// A kernel entry ABI contract, not a pointer, allocation or execution permit.
mod kernel_argument_abi_v18 {
    use super::*;
    use fe2o3_kernel_descriptor::{
        AccessMode as DescriptorAccess, AliasSemantics, DeviceLayoutDescriptorV1,
        DeviceLayoutRecordV1, LogicalArgumentV1, OwnershipSemantics, PhysicalAbiComponentKind,
        ScalarTypeV1 as DescriptorScalar, SourceTypeDescriptorV1, SourceTypeDescriptorV3,
        SourceTypeRecordV1, SourceTypeRecordV3, ValidName,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticLayoutIdentityV1, SemanticRustTypeKindV1, SemanticTargetDataLayoutV1,
        SemanticTypeIdentityV1,
    };

    /// Original compiler descriptor classification for one source argument.
    ///
    /// Construction does not authenticate the argument or authorize a memory
    /// access. The complete original root/source roster is checked on capture.
    pub enum ProductionKernelArgumentAbiKindV18 {
        /// A checked descriptor argument and its original source kind.
        Descriptor {
            /// Source kind; pointer-sized nominal integers retain their kind.
            source: SourceTypeDescriptorV3,
            /// Existing checked descriptor component representation.
            argument: LogicalArgumentV1,
        },
        /// Source-owned by-value packing proposal; final native mapping is separate.
        ///
        /// This variant contributes no Global pointer component.
        CompilerLaidOutByValue {
            /// Original compiler-laid-out kernarg offset, not a pointer offset.
            offset: u32,
        },
    }

    /// One original source argument in a complete ordered kernel ABI input.
    pub struct ProductionKernelArgumentAbiArgumentV18 {
        /// Identity captured from the original rustc source type.
        pub semantic_type_identity: SemanticTypeIdentityV1,
        /// Closed physical classification; not a value provenance certificate.
        pub kind: ProductionKernelArgumentAbiKindV18,
    }

    /// Borrowed descriptor contract for one original kernel root.
    pub struct ProductionKernelArgumentAbiRootV18<'a> {
        /// Original kernel binding, not a caller-selected function namespace.
        pub kernel_binding: &'a [u8; 32],
        /// Original exported symbol.
        pub export: &'a str,
        /// Every original source argument, including nonphysical Ignore arguments.
        pub arguments: &'a [ProductionKernelArgumentAbiArgumentV18],
        /// Descriptor's captured explicit kernarg extent.
        pub explicit_argument_bytes: u32,
        /// Descriptor's captured kernarg alignment.
        pub kernarg_alignment_bytes: u32,
    }

    /// Complete original descriptor/root input for source-owned V18 capture.
    ///
    /// This borrowed input has no independent authority. Capture checks its
    /// original owner, root order, ABI, types and descriptor component shapes.
    pub struct ProductionKernelArgumentAbiInputV18<'a> {
        /// Roots in the original admitted semantic root order.
        pub roots: &'a [ProductionKernelArgumentAbiRootV18<'a>],
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Kind {
        Descriptor(SourceTypeDescriptorV3),
        CompilerLaidOutByValue { offset: u32 },
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Component {
        kind: PhysicalAbiComponentKind,
        offset: u32,
        size: u16,
        alignment: u16,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Argument {
        ty: SemanticTypeIdV1,
        identity: SemanticTypeIdentityV1,
        kind: Kind,
        ownership: SemanticSourceArgumentOwnershipV1,
        access: DescriptorAccess,
        alias: AliasSemantics,
        components: [Option<Component>; 2],
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Root {
        function: SemanticFunctionIdV1,
        binding: [u8; 32],
        arguments: std::ops::Range<usize>,
        explicit_argument_bytes: u32,
        kernarg_alignment_bytes: u32,
    }

    // The enclosing OwnedExecutionInput pays this inline header. This structure
    // owns only the two vectors' additional retained storage.
    #[derive(Debug)]
    pub(super) struct CapturedKernelArgumentAbiV18 {
        source: [u8; 32],
        ssa: ProductionSemanticSsaIdentityV1,
        target: SemanticTargetDataLayoutV1,
        layout: SemanticLayoutIdentityV1,
        roots: Vec<Root>,
        arguments: Vec<Argument>,
        retained: usize,
    }

    // This lens borrows the complete captured contract. It is neither a new
    // descriptor owner nor an allocation/currentness certificate.
    #[derive(Clone, Copy)]
    pub(super) struct SourceDescriptorRootAbiV29<'a> {
        profile: &'a CapturedKernelArgumentAbiV18,
        original_root: usize,
    }

    include!("production_kernel_inline_argument_abi_v29.rs");

    impl SourceDescriptorRootAbiV29<'_> {
        pub(super) fn check_instances(
            self,
            instances: &ExecutionInstancesV29<'_>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            self.profile.check(instances.owner(), budget)?;
            budget.charge_work(3)?;
            let root = self.profile.roots.get(self.original_root).ok_or_else(mismatch)?;
            if instances.instance(instances.root()).map(|row| row.function())
                != Some(root.function)
                || instances.owner().source_semantic().roots().get(self.original_root)
                    != Some(&root.function)
            {
                return Err(mismatch());
            }
            Ok(())
        }

        pub(super) fn shared_slice(
            self,
            argument: u32,
            ty: SemanticTypeIdV1,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<bool, ProductionSemanticKirErrorV1> {
            budget.charge_work(5)?;
            let root = self.profile.roots.get(self.original_root).ok_or_else(mismatch)?;
            let ordinal = usize::try_from(argument).map_err(|_| mismatch())?;
            let index = root.arguments.start.checked_add(ordinal).ok_or_else(mismatch)?;
            if index >= root.arguments.end {
                return Err(mismatch());
            }
            let row = self.profile.arguments.get(index).ok_or_else(mismatch)?;
            if row.ty != ty {
                return Err(mismatch());
            }
            if !matches!(row.kind, Kind::Descriptor(SourceTypeDescriptorV3::SharedSlice(_))) {
                return Ok(false);
            }
            if !matches!(row.components, [Some(Component { kind: PhysicalAbiComponentKind::GlobalPointer, .. }), Some(Component { kind: PhysicalAbiComponentKind::SliceLengthU64, .. })])
            {
                return Err(mismatch());
            }
            Ok(true)
        }
    }

    fn mismatch() -> ProductionSemanticKirErrorV1 {
        source_reference_error_v29(
            "kernel argument ABI profile differs from the complete original descriptor/source contract",
        )
    }

    // Capture failures are original source errors. A borrowed profile query
    // instead checks attachment binding; preserve any exact resource refusal.
    pub(super) fn query_error(
        error: ProductionSemanticKirErrorV1,
    ) -> ProductionSourceOwnedViewErrorV18 {
        match error {
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
            _ => ProductionSourceOwnedViewErrorV18::Binding(
                "kernel argument ABI profile differs from its original descriptor/source contract",
            ),
        }
    }

    fn descriptor_scalar(scalar: DescriptorScalar) -> ScalarType {
        match scalar {
            DescriptorScalar::I8 => ScalarType::I8,
            DescriptorScalar::U8 => ScalarType::U8,
            DescriptorScalar::I16 => ScalarType::I16,
            DescriptorScalar::U16 => ScalarType::U16,
            DescriptorScalar::I32 => ScalarType::I32,
            DescriptorScalar::U32 => ScalarType::U32,
            DescriptorScalar::I64 => ScalarType::I64,
            DescriptorScalar::U64 => ScalarType::U64,
            DescriptorScalar::F16 => ScalarType::F16,
            DescriptorScalar::F32 => ScalarType::F32,
            DescriptorScalar::F64 => ScalarType::F64,
        }
    }

    fn descriptor_source_scalar(scalar: DescriptorScalar) -> SemanticScalarTypeV1 {
        match scalar {
            DescriptorScalar::I8 => SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 8,
            },
            DescriptorScalar::I16 => SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 16,
            },
            DescriptorScalar::I32 => SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 32,
            },
            DescriptorScalar::I64 => SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 64,
            },
            DescriptorScalar::U8 => SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 8,
            },
            DescriptorScalar::U16 => SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 16,
            },
            DescriptorScalar::U32 => SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
            DescriptorScalar::U64 => SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            },
            DescriptorScalar::F16 => SemanticScalarTypeV1::Float { bits: 16 },
            DescriptorScalar::F32 => SemanticScalarTypeV1::Float { bits: 32 },
            DescriptorScalar::F64 => SemanticScalarTypeV1::Float { bits: 64 },
        }
    }

    fn records(
        kind: SourceTypeDescriptorV3,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        (
            fe2o3_kernel_descriptor::RustTypeIdentity,
            fe2o3_kernel_descriptor::DeviceLayoutIdentity,
        ),
        ProductionSemanticKirErrorV1,
    > {
        let (source, layout) = match kind {
            SourceTypeDescriptorV3::Scalar(s) => (
                SourceTypeDescriptorV3::Scalar(s),
                DeviceLayoutDescriptorV1::scalar(s),
            ),
            SourceTypeDescriptorV3::SharedSlice(s) => (
                SourceTypeDescriptorV3::SharedSlice(s),
                DeviceLayoutDescriptorV1::shared_slice(s),
            ),
            SourceTypeDescriptorV3::DisjointSlice(s) => (
                SourceTypeDescriptorV3::DisjointSlice(s),
                DeviceLayoutDescriptorV1::disjoint_slice(s),
            ),
            SourceTypeDescriptorV3::GlobalMutPointer(s) => (
                SourceTypeDescriptorV3::GlobalMutPointer(s),
                DeviceLayoutDescriptorV1::global_mut_pointer(s),
            ),
            SourceTypeDescriptorV3::Usize => (
                SourceTypeDescriptorV3::Scalar(DescriptorScalar::U64),
                DeviceLayoutDescriptorV1::scalar(DescriptorScalar::U64),
            ),
            SourceTypeDescriptorV3::Isize => (
                SourceTypeDescriptorV3::Scalar(DescriptorScalar::I64),
                DeviceLayoutDescriptorV1::scalar(DescriptorScalar::I64),
            ),
        };
        let scratch = fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3;
        budget.reserve_storage(scratch)?;
        let result = (|| {
            let source = SourceTypeRecordV3::new(source, &mut |work| budget.charge_work(work));
            let source = source.map_err(descriptor_error)?;
            let layout = fe2o3_kernel_descriptor::device_layout_record_v3(layout, &mut |work| {
                budget.charge_work(work)
            })
            .map_err(descriptor_error)?;
            Ok((source.identity(), layout.identity()))
        })();
        let release = budget.release_storage(scratch);
        match result {
            Err(error) => Err(error),
            Ok(value) => {
                release?;
                Ok(value)
            }
        }
    }

    fn descriptor_error(
        error: fe2o3_kernel_descriptor::DescriptorWireErrorV3<ArgumentResourceV1>,
    ) -> ProductionSemanticKirErrorV1 {
        match error {
            fe2o3_kernel_descriptor::DescriptorWireErrorV3::Work(error) => error.into(),
            _ => mismatch(),
        }
    }

    fn descriptor_shape(
        ordinal: usize,
        row: &Argument,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let Kind::Descriptor(kind) = row.kind else {
            return if row.components == [None, None]
                && row.access == DescriptorAccess::ByValue
                && row.alias == AliasSemantics::Value
            {
                Ok(())
            } else {
                Err(mismatch())
            };
        };
        let first = row.components[0].ok_or_else(mismatch)?;
        let index = u16::try_from(ordinal).map_err(|_| mismatch())?;
        // The public checked constructor remains the component validator. V1
        // contains at most two components with exactly PhysicalComponentV3's
        // fields; QUERY covers its fixed descriptor hash/validation scratch.
        let scratch = argument_sum_v1(&[
            fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3,
            size_of::<LogicalArgumentV1>(),
            argument_product_v1(2, size_of::<fe2o3_kernel_descriptor::PhysicalComponentV3>())?,
            3,
        ])?;
        budget.reserve_storage(scratch)?;
        let result = (|| {
            budget.charge_work(128)?;
            let (source, layout) = match kind {
                SourceTypeDescriptorV3::Scalar(s) => (
                    SourceTypeDescriptorV1::scalar(s),
                    DeviceLayoutDescriptorV1::scalar(s),
                ),
                SourceTypeDescriptorV3::SharedSlice(s) => (
                    SourceTypeDescriptorV1::shared_slice(s),
                    DeviceLayoutDescriptorV1::shared_slice(s),
                ),
                SourceTypeDescriptorV3::DisjointSlice(s) => (
                    SourceTypeDescriptorV1::disjoint_slice(s),
                    DeviceLayoutDescriptorV1::disjoint_slice(s),
                ),
                SourceTypeDescriptorV3::GlobalMutPointer(s) => (
                    SourceTypeDescriptorV1::global_mut_pointer(s),
                    DeviceLayoutDescriptorV1::global_mut_pointer(s),
                ),
                SourceTypeDescriptorV3::Usize => (
                    SourceTypeDescriptorV1::scalar(DescriptorScalar::U64),
                    DeviceLayoutDescriptorV1::scalar(DescriptorScalar::U64),
                ),
                SourceTypeDescriptorV3::Isize => (
                    SourceTypeDescriptorV1::scalar(DescriptorScalar::I64),
                    DeviceLayoutDescriptorV1::scalar(DescriptorScalar::I64),
                ),
            };
            let source = SourceTypeRecordV1::new(source);
            let layout = DeviceLayoutRecordV1::new(layout);
            let name = ValidName::new("abi".to_owned()).map_err(|_| mismatch())?;
            let expected = match kind {
                SourceTypeDescriptorV3::Scalar(_)
                | SourceTypeDescriptorV3::Usize
                | SourceTypeDescriptorV3::Isize => {
                    LogicalArgumentV1::scalar(index, name, &source, &layout, first.offset)
                }
                SourceTypeDescriptorV3::SharedSlice(_) => {
                    LogicalArgumentV1::shared_slice(index, name, &source, &layout, first.offset)
                }
                SourceTypeDescriptorV3::DisjointSlice(_) => LogicalArgumentV1::disjoint_slice(
                    index,
                    name,
                    &source,
                    &layout,
                    row.access,
                    first.offset,
                ),
                SourceTypeDescriptorV3::GlobalMutPointer(_) => {
                    LogicalArgumentV1::global_mut_pointer(
                        index,
                        name,
                        &source,
                        &layout,
                        first.offset,
                    )
                }
            }
            .map_err(|_| mismatch())?;
            let mut components = [None, None];
            for (index, (kind, offset, size, alignment)) in
                expected.physical_components().enumerate()
            {
                budget.charge_work(5)?;
                components[index] = Some(Component {
                    kind,
                    offset,
                    size,
                    alignment,
                });
            }
            if components != row.components
                || expected.access() != row.access
                || expected.alias() != row.alias
            {
                return Err(mismatch());
            }
            Ok(())
        })();
        let release = budget.release_storage(scratch);
        match result {
            Err(error) => Err(error),
            Ok(()) => {
                release?;
                Ok(())
            }
        }
    }

    // Pointer-sized source kinds remain explicit even though their fixed
    // physical scalar descriptor is u64/i64.
    fn check_source_kind(
        semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
        function: &SemanticFunctionDeclV1,
        ordinal: usize,
        ty: SemanticTypeIdV1,
        kind: Kind,
        access: DescriptorAccess,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(16)?;
        let declaration = semantic
            .types()
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?;
        let ownership = function
            .abi()
            .source_argument_ownership()
            .get(ordinal)
            .copied()
            .ok_or_else(mismatch)?;
        if function.abi().source_input_types().get(ordinal) != Some(&ty)
            || declaration.layout().is_uninhabited()
        {
            return Err(mismatch());
        }
        let Kind::Descriptor(kind) = kind else {
            if ownership != SemanticSourceArgumentOwnershipV1::ByValue {
                return Err(mismatch());
            }
            by_value_source_abi_v29(semantic, function, ordinal, ty)?;
            return Ok(());
        };
        match kind {
            SourceTypeDescriptorV3::SharedSlice(element) => {
                // A root has the admitted GPU kernel ABI, not the Rust helper
                // ABI checked by shared_slice_helper_parameter_v1. Reuse its
                // exact source leaf shape without manufacturing Global typing.
                let value = function
                    .abi()
                    .adjusted_arguments()
                    .get(ordinal)
                    .ok_or_else(mismatch)?;
                if function.role() != SemanticFunctionRoleV1::KernelRoot
                    || function.kernel_entry().is_none()
                    || function.abi().canon_abi() != SemanticCanonAbiV1::GpuKernel
                    || function.abi().extern_abi()
                        != fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::GpuKernel
                    || ownership != SemanticSourceArgumentOwnershipV1::SharedBorrow
                    || value.role() != SemanticAbiArgumentRoleV1::Source
                    || value.ty() != ty
                    || value.value().adjusted().is_some()
                    || value.value().pointee_override().is_some()
                    || !matches!(value.mode(), SemanticAbiPassModeV1::Pair { .. })
                    || !shared_slice_leaf_v1(semantic.types(), ty)
                {
                    return Err(mismatch());
                }
                let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
                    return Err(mismatch());
                };
                let Some(SemanticTypeShapeV1::Slice { element: actual }) = semantic
                    .types()
                    .get(pointer.pointee().index() as usize)
                    .map(SemanticTypeDeclV1::shape)
                else {
                    return Err(mismatch());
                };
                if semantic
                    .types()
                    .get(actual.index() as usize)
                    .map(SemanticTypeDeclV1::shape)
                    != Some(&SemanticTypeShapeV1::Scalar(descriptor_source_scalar(
                        element,
                    )))
                {
                    return Err(mismatch());
                }
            }
            SourceTypeDescriptorV3::Scalar(element) => {
                if ownership != SemanticSourceArgumentOwnershipV1::ByValue
                    || matches!(
                        declaration.rust_type_kind(),
                        SemanticRustTypeKindV1::Usize | SemanticRustTypeKindV1::Isize
                    )
                    || declaration.shape()
                        != &SemanticTypeShapeV1::Scalar(descriptor_source_scalar(element))
                {
                    return Err(mismatch());
                }
            }
            SourceTypeDescriptorV3::Usize | SourceTypeDescriptorV3::Isize => {
                let expected = if kind == SourceTypeDescriptorV3::Usize {
                    SemanticRustTypeKindV1::Usize
                } else {
                    SemanticRustTypeKindV1::Isize
                };
                if ownership != SemanticSourceArgumentOwnershipV1::ByValue
                    || declaration.rust_type_kind() != expected
                    || declaration.layout().size_bytes() != Some(8)
                    || declaration.layout().alignment_bytes() != 8
                {
                    return Err(mismatch());
                }
            }
            SourceTypeDescriptorV3::DisjointSlice(_)
            | SourceTypeDescriptorV3::GlobalMutPointer(_) => {
                // The existing nominal parameter checks remain the source
                // authority. This profile does not widen their accepted types.
                if ownership != SemanticSourceArgumentOwnershipV1::ExclusiveOwner {
                    return Err(mismatch());
                }
                let fields = match declaration.shape() {
                    SemanticTypeShapeV1::Aggregate(fields) => fields.fields().len(),
                    _ => 0,
                };
                budget.charge_work(argument_sum_v1(&[
                    argument_product_v1(semantic.callables().len(), 8)?,
                    argument_product_v1(fields, 16)?,
                ])?)?;
                let scratch = argument_product_v1(4, size_of::<Type>())?;
                budget.reserve_storage(scratch)?;
                let actual = match kind {
                    SourceTypeDescriptorV3::DisjointSlice(_) => {
                        authenticated_disjoint_slice_parameter(
                            semantic.types(),
                            semantic.callables(),
                            function,
                            ordinal as u32,
                            ty,
                        )
                    }
                    SourceTypeDescriptorV3::GlobalMutPointer(_) => {
                        authenticated_global_mut_pointer_parameter(
                            semantic.types(),
                            function,
                            ordinal as u32,
                            ty,
                        )
                    }
                    _ => unreachable!(),
                };
                let matches = match (&kind, actual.as_ref()) {
                    (SourceTypeDescriptorV3::DisjointSlice(element), Some(Type::Slice(slice))) => {
                        slice.address_space == AddressSpace::Global
                            && slice.element.as_ref() == &Type::Scalar(descriptor_scalar(*element))
                            && access_matches(slice.access, access)
                    }
                    (
                        SourceTypeDescriptorV3::GlobalMutPointer(element),
                        Some(Type::Pointer(pointer)),
                    ) => {
                        pointer.address_space == AddressSpace::Global
                            && pointer.pointee.as_ref()
                                == &Type::Scalar(descriptor_scalar(*element))
                            && access_matches(pointer.access, access)
                    }
                    _ => false,
                };
                drop(actual);
                budget.release_storage(scratch)?;
                if !matches {
                    return Err(mismatch());
                }
            }
        }
        Ok(())
    }

    fn access_matches(actual: AccessMode, expected: DescriptorAccess) -> bool {
        matches!(
            (actual, expected),
            (AccessMode::ReadOnly, DescriptorAccess::ReadOnly)
                | (AccessMode::WriteOnly, DescriptorAccess::WriteOnly)
                | (AccessMode::ReadWrite, DescriptorAccess::ReadWrite)
        )
    }

    fn capture_argument(
        semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
        function: &SemanticFunctionDeclV1,
        ordinal: usize,
        input: &ProductionKernelArgumentAbiArgumentV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Argument, ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        let ty = *function
            .abi()
            .source_input_types()
            .get(ordinal)
            .ok_or_else(mismatch)?;
        let declaration = semantic
            .types()
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?;
        if input.semantic_type_identity != declaration.identity() {
            return Err(mismatch());
        }
        let ownership = *function
            .abi()
            .source_argument_ownership()
            .get(ordinal)
            .ok_or_else(mismatch)?;
        let mut row = Argument {
            ty,
            identity: declaration.identity(),
            ownership,
            kind: match input.kind {
                ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset } => {
                    Kind::CompilerLaidOutByValue { offset }
                }
                ProductionKernelArgumentAbiKindV18::Descriptor { .. } => {
                    Kind::CompilerLaidOutByValue { offset: 0 }
                }
            },
            access: DescriptorAccess::ByValue,
            alias: AliasSemantics::Value,
            components: [None, None],
        };
        if let ProductionKernelArgumentAbiKindV18::Descriptor { source, argument } = &input.kind {
            budget.charge_work(64)?;
            let (expected_source, expected_layout) = records(*source, budget)?;
            if usize::from(argument.source_index()) != ordinal
                || argument.source_type() != expected_source
                || argument.device_layout() != expected_layout
                || argument.physical_components().len() > 2
            {
                return Err(mismatch());
            }
            let expected_ownership = match ownership {
                SemanticSourceArgumentOwnershipV1::ByValue => OwnershipSemantics::ByValue,
                SemanticSourceArgumentOwnershipV1::SharedBorrow => OwnershipSemantics::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner => {
                    OwnershipSemantics::UniqueBorrow
                }
                _ => return Err(mismatch()),
            };
            if argument.ownership() != expected_ownership {
                return Err(mismatch());
            }
            row.kind = Kind::Descriptor(*source);
            row.access = argument.access();
            row.alias = argument.alias();
            for (index, (kind, offset, size, alignment)) in
                argument.physical_components().enumerate()
            {
                budget.charge_work(5)?;
                row.components[index] = Some(Component {
                    kind,
                    offset,
                    size,
                    alignment,
                });
            }
        }
        descriptor_shape(ordinal, &row, budget)?;
        check_source_kind(
            semantic, function, ordinal, ty, row.kind, row.access, budget,
        )?;
        Ok(row)
    }

    fn packed_argument_end(
        semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
        function: &SemanticFunctionDeclV1,
        ordinal: usize,
        row: &Argument,
        previous: u64,
        extent: u32,
        alignment: u32,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<u64, ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        let layout = semantic
            .types()
            .get(row.ty.index() as usize)
            .ok_or_else(mismatch)?
            .layout();
        let size = layout.size_bytes().ok_or_else(mismatch)?;
        let source_alignment = layout.alignment_bytes();
        let offset = u64::from(match row.kind {
            Kind::CompilerLaidOutByValue { offset } => offset,
            Kind::Descriptor(_) => row.components[0].ok_or_else(mismatch)?.offset,
        });
        if matches!(row.kind, Kind::CompilerLaidOutByValue { .. }) {
            let (_, abi) = by_value_source_abi_v29(semantic, function, ordinal, row.ty)?;
            if matches!(abi.mode(), SemanticAbiPassModeV1::Ignore) {
                if offset != previous || offset > u64::from(extent) { return Err(mismatch()); }
                return Ok(previous);
            }
        }
        let end = offset.checked_add(size).ok_or_else(mismatch)?;
        if source_alignment == 0
            || offset % source_alignment != 0
            || source_alignment > u64::from(alignment)
            || offset < previous
            || end > u64::from(extent)
        {
            return Err(mismatch());
        }
        for component in row.components.into_iter().flatten() {
            budget.charge_work(5)?;
            let component_end = u64::from(component.offset)
                .checked_add(u64::from(component.size))
                .ok_or_else(mismatch)?;
            if u64::from(component.offset) < offset || component_end > end {
                return Err(mismatch());
            }
        }
        Ok(end)
    }

    impl CapturedKernelArgumentAbiV18 {
        pub(super) fn capture(
            owner: &ProductionSemanticSsaOwnerV1,
            input: ProductionKernelArgumentAbiInputV18<'_>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            let semantic = owner.source_semantic();
            budget.charge_work(argument_sum_v1(&[input.roots.len(), 4])?)?;
            if input.roots.len() != semantic.roots().len() {
                return Err(mismatch());
            }
            let count = input.roots.iter().try_fold(0usize, |count, row| {
                argument_sum_v1(&[count, row.arguments.len()])
            })?;
            let mut roots = emission_vec_v1(input.roots.len(), budget)?;
            let mut arguments = emission_vec_v1(count, budget)?;
            for (&function_id, root) in semantic.roots().iter().zip(input.roots) {
                budget.charge_work(argument_sum_v1(&[root.export.len(), 40])?)?;
                let function = semantic
                    .functions()
                    .get(function_id.index() as usize)
                    .ok_or_else(mismatch)?;
                let entry = function.kernel_entry().ok_or_else(mismatch)?;
                if entry.kernel_binding_identity().as_bytes() != root.kernel_binding
                    || entry.export_symbol().as_bytes() != root.export.as_bytes()
                    || function.abi().source_input_types().len() != root.arguments.len()
                    || root.kernarg_alignment_bytes == 0
                    || !root.kernarg_alignment_bytes.is_power_of_two()
                {
                    return Err(mismatch());
                }
                let first = arguments.len();
                let mut previous = 0;
                for (ordinal, argument) in root.arguments.iter().enumerate() {
                    let row = capture_argument(semantic, function, ordinal, argument, budget)?;
                    previous = packed_argument_end(
                        semantic,
                        function,
                        ordinal,
                        &row,
                        previous,
                        root.explicit_argument_bytes,
                        root.kernarg_alignment_bytes,
                        budget,
                    )?;
                    arguments.push(row);
                }
                roots.push(Root {
                    function: function_id,
                    binding: *root.kernel_binding,
                    arguments: first..arguments.len(),
                    explicit_argument_bytes: root.explicit_argument_bytes,
                    kernarg_alignment_bytes: root.kernarg_alignment_bytes,
                });
            }
            let retained = argument_sum_v1(&[
                argument_product_v1(roots.capacity(), size_of::<Root>())?,
                argument_product_v1(arguments.capacity(), size_of::<Argument>())?,
            ])?;
            Ok(Self {
                source: *owner.source_semantic_sha256(),
                ssa: owner.identity(),
                target: semantic.target(),
                layout: semantic.target_layout_identity(),
                roots,
                arguments,
                retained,
            })
        }

        pub(super) fn retained_storage(&self) -> usize {
            self.retained
        }

        pub(super) fn argument_count(
            &self,
            root: usize,
        ) -> Result<usize, ProductionSemanticKirErrorV1> {
            let root = self.roots.get(root).ok_or_else(mismatch)?;
            root.arguments
                .end
                .checked_sub(root.arguments.start)
                .ok_or_else(mismatch)
        }

        pub(super) fn argument_kind(
            &self,
            root: usize,
            argument: usize,
        ) -> Result<Option<SourceTypeDescriptorV3>, ProductionSemanticKirErrorV1> {
            let root = self.roots.get(root).ok_or_else(mismatch)?;
            let index = root
                .arguments
                .start
                .checked_add(argument)
                .ok_or_else(mismatch)?;
            if index >= root.arguments.end {
                return Err(mismatch());
            }
            Ok(match self.arguments.get(index).ok_or_else(mismatch)?.kind {
                Kind::Descriptor(kind) => Some(kind),
                Kind::CompilerLaidOutByValue { .. } => None,
            })
        }

        pub(super) fn matches_original_input(
            &self,
            owner: &ProductionSemanticSsaOwnerV1,
            input: ProductionKernelArgumentAbiInputV18<'_>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            self.check(owner, budget)?;
            budget.charge_work(4)?;
            if input.roots.len() != self.roots.len() {
                return Err(mismatch());
            }
            let semantic = owner.source_semantic();
            for (original, captured) in input.roots.iter().zip(&self.roots) {
                budget.charge_work(argument_sum_v1(&[original.export.len(), 40])?)?;
                let function = semantic
                    .functions()
                    .get(captured.function.index() as usize)
                    .ok_or_else(mismatch)?;
                let entry = function.kernel_entry().ok_or_else(mismatch)?;
                if original.kernel_binding != &captured.binding
                    || original.export.as_bytes() != entry.export_symbol().as_bytes()
                    || original.arguments.len() != captured.arguments.len()
                    || original.explicit_argument_bytes != captured.explicit_argument_bytes
                    || original.kernarg_alignment_bytes != captured.kernarg_alignment_bytes
                {
                    return Err(mismatch());
                }
                for (ordinal, argument) in original.arguments.iter().enumerate() {
                    let row = capture_argument(semantic, function, ordinal, argument, budget)?;
                    if self.arguments.get(captured.arguments.start + ordinal) != Some(&row) {
                        return Err(mismatch());
                    }
                }
            }
            Ok(())
        }

        pub(super) fn descriptor_root(
            &self,
            owner: &ProductionSemanticSsaOwnerV1,
            original_root: usize,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<SourceDescriptorRootAbiV29<'_>, ProductionSemanticKirErrorV1> {
            self.check(owner, budget)?;
            budget.charge_work(1)?;
            if self.roots.get(original_root).is_none() {
                return Err(mismatch());
            }
            Ok(SourceDescriptorRootAbiV29 { profile: self, original_root })
        }

        pub(super) fn check(
            &self,
            owner: &ProductionSemanticSsaOwnerV1,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            budget.charge_work(72)?;
            let semantic = owner.source_semantic();
            if self.source != *owner.source_semantic_sha256()
                || self.ssa != owner.identity()
                || self.target != semantic.target()
                || self.layout != semantic.target_layout_identity()
                || self.roots.len() != semantic.roots().len()
            {
                return Err(mismatch());
            }
            let mut first = 0;
            for (&function_id, root) in semantic.roots().iter().zip(&self.roots) {
                budget.charge_work(40)?;
                let function = semantic
                    .functions()
                    .get(function_id.index() as usize)
                    .ok_or_else(mismatch)?;
                let entry = function.kernel_entry().ok_or_else(mismatch)?;
                let end = argument_sum_v1(&[first, function.abi().source_input_types().len()])?;
                if root.function != function_id
                    || root.binding != *entry.kernel_binding_identity().as_bytes()
                    || root.arguments != (first..end)
                    || end > self.arguments.len()
                    || root.kernarg_alignment_bytes == 0
                    || !root.kernarg_alignment_bytes.is_power_of_two()
                {
                    return Err(mismatch());
                }
                let mut previous = 0;
                for (ordinal, row) in self.arguments[first..end].iter().enumerate() {
                    budget.charge_work(8)?;
                    if function.abi().source_input_types().get(ordinal) != Some(&row.ty)
                        || function.abi().source_argument_ownership().get(ordinal)
                            != Some(&row.ownership)
                        || semantic
                            .types()
                            .get(row.ty.index() as usize)
                            .map(SemanticTypeDeclV1::identity)
                            != Some(row.identity)
                    {
                        return Err(mismatch());
                    }
                    descriptor_shape(ordinal, row, budget)?;
                    check_source_kind(
                        semantic, function, ordinal, row.ty, row.kind, row.access, budget,
                    )?;
                    previous = packed_argument_end(
                        semantic,
                        function,
                        ordinal,
                        row,
                        previous,
                        root.explicit_argument_bytes,
                        root.kernarg_alignment_bytes,
                        budget,
                    )?;
                }
                first = end;
            }
            if first != self.arguments.len() {
                return Err(mismatch());
            }
            Ok(())
        }
    }

    #[cfg(test)]
    pub(super) mod tests {
        include!("production_kernel_argument_abi_v18_tests.rs");
    }
}

pub use kernel_argument_abi_v18::{
    ProductionKernelArgumentAbiArgumentV18, ProductionKernelArgumentAbiInputV18,
    ProductionKernelArgumentAbiKindV18, ProductionKernelArgumentAbiRootV18,
    ProductionKernelByValueAbiV29, ProductionKernelIgnoredArgumentAbiV29,
    ProductionKernelInlineArgumentAbiV29,
};
