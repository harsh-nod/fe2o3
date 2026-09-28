/// Fixed diagnostic coordinates in the exact graph being censused.
/// This is explanatory data, never an operation-admission certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCheckedOutputCensusContextV1 {
    /// A block/operation ordinal in the current ranked kernel.
    Ranked {
        /// Zero-based block ordinal in the exact ranked kernel.
        block: usize,
        /// Zero-based operation ordinal in that block.
        operation: usize,
        /// Closed ranked operation kind, including an explicit view space.
        kind: ProductionCheckedOutputRankedCensusKindV1,
    },
    /// An existing coordinate in the current B/C/O inventory.
    Native {
        /// Exact inventory function/block/operation coordinate.
        coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        /// Closed operation kind and fixed scalar/intrinsic discriminator.
        kind: ProductionCheckedOutputNativeCensusKindV1,
        /// Total result count; no result list is copied.
        results: usize,
        /// Bounded shallow summary of the first result, when one exists.
        first_result: Option<ProductionCheckedOutputCensusTypeV1>,
    },
}

macro_rules! ranked_census_kinds {
    ($($variant:ident),+ $(,)?) => {
        /// Exhaustive ranked operation categories used only in refusal diagnostics.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ProductionCheckedOutputRankedCensusKindV1 {
            $(#[doc = concat!("The `", stringify!($variant), "` ranked operation.")]
            $variant,)+
            /// A ranked view with its actual explicit memory space.
            ViewInSpace(dialect_kernel::MemorySpaceAttr),
            /// An allocation-level effect with its actual explicit memory space.
            AllocationEffect(dialect_kernel::MemorySpaceAttr),
        }
        impl ProductionCheckedOutputRankedCensusKindV1 {
            pub(super) fn of(operation: &fe2o3_pliron::ProductionRankedOperationV1) -> Self {
                use fe2o3_pliron::ProductionRankedOperationV1 as O;
                match operation {
                    $(O::$variant { .. } => Self::$variant,)+
                    O::ViewInSpace { memory_space, .. } => Self::ViewInSpace(*memory_space),
                    O::AllocationEffect { memory_space, .. } => Self::AllocationEffect(*memory_space),
                }
            }
        }
    };
}
ranked_census_kinds!(
    ExecutionLayout,
    View,
    PipelineCreate,
    PipelineEvent,
    IndexConstant,
    IndexUnsignedCast,
    IndexUnknown,
    InvocationIndex,
    IndexBinary,
    DeterministicJoin,
    CheckedTiledIndex2D,
    CheckedRowStripedIndex2D,
    PredicatedCheckedTiledIndex2D,
    PredicatedCheckedRowStripedIndex2D,
    Dimension,
    Access,
    PredicatedAccess,
    ValueAccess,
    AtomicAccess,
    AtomicValueAccess,
    OwnershipContract,
    Barrier,
    Fence,
    TensorLayout,
    TensorResultComponent,
    SemanticSymbol,
    SemanticConstant,
    SemanticBinary,
    SemanticExpression,
    CollectiveSemantics,
    RequireEquivalent,
    RequireAuthenticatedReferenceEquivalent,
    RequestAuthenticatedReferenceEquivalent,
    RequireEffectRefinement,
    RequestEffectRefinement,
    RequireNumericalRefinement,
    RequestNumericalRefinement,
    RequireTensorRefinement,
    RequestTensorRefinement,
);

macro_rules! native_census_kinds {
    ($($variant:ident),+ $(,)?) => {
        /// Exhaustive physical operation categories used only in refusal diagnostics.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ProductionCheckedOutputNativeCensusKindV1 {
            $(#[doc = concat!("The `", stringify!($variant), "` physical operation.")]
            $variant,)+
            /// Exact scalar unary operation.
            Unary(fe2o3_kernel_ir::UnaryOp),
            /// Exact scalar binary operation, including checked operation kind.
            Binary(fe2o3_kernel_ir::BinaryOp),
            /// Exact cast operation.
            Cast(fe2o3_kernel_ir::CastKind),
            /// Exact launch/invocation intrinsic, including hierarchy and axis.
            Intrinsic(fe2o3_kernel_ir::IntrinsicKind),
            /// Explicit allocation address space.
            Alloca(fe2o3_kernel_ir::AddressSpace),
            /// Explicit load address space.
            Load(fe2o3_kernel_ir::AddressSpace),
            /// Explicit guarded-load address space.
            GuardedLoad(fe2o3_kernel_ir::AddressSpace),
            /// Explicit store address space.
            Store(fe2o3_kernel_ir::AddressSpace),
            /// Explicit guarded-store address space.
            GuardedStore(fe2o3_kernel_ir::AddressSpace),
        }
    };
}
native_census_kinds!(
    Storage,
    Execution,
    VerificationContract,
    VectorLoad,
    VectorStore,
    VectorLayoutConvert,
    Constant,
    MemoryIntrinsic,
    Compare,
    Select,
    Call,
    SliceLength,
    SliceData,
    GetElementPointer,
    Barrier,
    Atomic,
    Fence,
    WorkgroupBarrier,
    WorkgroupMemory,
    Matrix,
    Gfx950LdsTranspose,
    Wave,
    InlineAssembly,
    Gfx942OrderedRegion,
    Gfx942OrderedProgram,
    Gfx942CompleteBodyDeclaration,
    Gfx942CompleteBodyStep,
    Gfx942PhysicalEntryDeclaration,
    Gfx942PhysicalEntryStep,
    Gfx942PhysicalGlobalCopyDeclaration,
    Gfx942PhysicalGlobalCopyStep,
    Gfx942PhysicalLdsExchangeDeclaration,
    Gfx942PhysicalLdsExchangeStep,
);
impl ProductionCheckedOutputNativeCensusKindV1 {
    pub(super) fn of(operation: &fe2o3_kernel_ir::OperationKind) -> Self {
        use fe2o3_kernel_ir::OperationKind as O;
        match operation {
            O::Storage(_) => Self::Storage,
            O::Execution(_) => Self::Execution,
            O::VerificationContract(_) => Self::VerificationContract,
            O::VectorLoad(_) => Self::VectorLoad,
            O::VectorStore(_) => Self::VectorStore,
            O::VectorLayoutConvert(_) => Self::VectorLayoutConvert,
            O::Constant(_) => Self::Constant,
            O::Intrinsic(value) => Self::Intrinsic(value.kind),
            O::MemoryIntrinsic(_) => Self::MemoryIntrinsic,
            O::Unary { op, .. } => Self::Unary(*op),
            O::Binary { op, .. } => Self::Binary(*op),
            O::Compare { .. } => Self::Compare,
            O::Cast { kind, .. } => Self::Cast(*kind),
            O::Select { .. } => Self::Select,
            O::Call { .. } => Self::Call,
            O::Alloca { address_space, .. } => Self::Alloca(*address_space),
            O::SliceLength { .. } => Self::SliceLength,
            O::SliceData { .. } => Self::SliceData,
            O::GetElementPointer { .. } => Self::GetElementPointer,
            O::Load { access, .. } => Self::Load(access.address_space),
            O::GuardedLoad { access, .. } => Self::GuardedLoad(access.address_space),
            O::Store { access, .. } => Self::Store(access.address_space),
            O::GuardedStore { access, .. } => Self::GuardedStore(access.address_space),
            O::Barrier(_) => Self::Barrier,
            O::Atomic(_) => Self::Atomic,
            O::Fence(_) => Self::Fence,
            O::WorkgroupBarrier(_) => Self::WorkgroupBarrier,
            O::WorkgroupMemory(_) => Self::WorkgroupMemory,
            O::Matrix(_) => Self::Matrix,
            O::Gfx950LdsTranspose(_) => Self::Gfx950LdsTranspose,
            O::Wave(_) => Self::Wave,
            O::InlineAssembly(_) => Self::InlineAssembly,
            O::Gfx942OrderedRegion(_) => Self::Gfx942OrderedRegion,
            O::Gfx942OrderedProgram(_) => Self::Gfx942OrderedProgram,
            O::Gfx942CompleteBodyDeclaration(_) => Self::Gfx942CompleteBodyDeclaration,
            O::Gfx942CompleteBodyStep(_) => Self::Gfx942CompleteBodyStep,
            O::Gfx942PhysicalEntryDeclaration(_) => Self::Gfx942PhysicalEntryDeclaration,
            O::Gfx942PhysicalEntryStep(_) => Self::Gfx942PhysicalEntryStep,
            O::Gfx942PhysicalGlobalCopyDeclaration(_) => Self::Gfx942PhysicalGlobalCopyDeclaration,
            O::Gfx942PhysicalGlobalCopyStep(_) => Self::Gfx942PhysicalGlobalCopyStep,
            O::Gfx942PhysicalLdsExchangeDeclaration(_) => {
                Self::Gfx942PhysicalLdsExchangeDeclaration
            }
            O::Gfx942PhysicalLdsExchangeStep(_) => Self::Gfx942PhysicalLdsExchangeStep,
        }
    }
}

/// Shallow, bounded result-type information; recursive pointees are not copied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCheckedOutputCensusTypeV1 {
    /// Unit type.
    Unit,
    /// Exact scalar type.
    Scalar(fe2o3_kernel_ir::ScalarType),
    /// Non-storable execution-role type.
    Execution,
    /// Fixed-vector type; its element/lane payload is not retained here.
    Vector,
    /// Module-local storage object; no schema or layout authority.
    StorageObject,
    /// Pointer address space and access mode, not a recursive pointee claim.
    Pointer(fe2o3_kernel_ir::AddressSpace, fe2o3_kernel_ir::AccessMode),
    /// Slice address space and access mode, not a recursive element claim.
    Slice(fe2o3_kernel_ir::AddressSpace, fe2o3_kernel_ir::AccessMode),
}
impl ProductionCheckedOutputCensusTypeV1 {
    pub(super) fn of(ty: &fe2o3_kernel_ir::Type) -> Self {
        use fe2o3_kernel_ir::Type;
        match ty {
            Type::Unit => Self::Unit,
            Type::Scalar(scalar) => Self::Scalar(*scalar),
            Type::Execution(_) => Self::Execution,
            Type::StorageObject(_) => Self::StorageObject,
            Type::Vector(_) => Self::Vector,
            Type::Pointer(pointer) => Self::Pointer(pointer.address_space, pointer.access),
            Type::Slice(slice) => Self::Slice(slice.address_space, slice.access),
        }
    }
}

impl ProductionCheckedOutputCensusContextV1 {
    pub(super) fn ranked(
        block: usize,
        operation: usize,
        value: &fe2o3_pliron::ProductionRankedOperationV1,
    ) -> Self {
        Self::Ranked {
            block,
            operation,
            kind: ProductionCheckedOutputRankedCensusKindV1::of(value),
        }
    }
    pub(super) fn native(
        coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        value: &fe2o3_kernel_ir::Operation,
    ) -> Self {
        Self::Native {
            coordinate,
            kind: ProductionCheckedOutputNativeCensusKindV1::of(&value.kind),
            results: value.results.len(),
            first_result: value
                .results
                .first()
                .map(|result| ProductionCheckedOutputCensusTypeV1::of(&result.ty)),
        }
    }
}
