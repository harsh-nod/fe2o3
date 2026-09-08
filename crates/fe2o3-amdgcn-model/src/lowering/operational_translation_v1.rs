//! Typed fail-closed coverage for production V13 operational translation evidence.

use fe2o3_kernel_ir::{
    ExecutionAtomicKindV1, ExecutionCapabilityOperationV1, ExecutionCollectiveKindV1, Module,
    OperationKind, Terminator,
};

/// Closed execution-capability operation family awaiting operational translation evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionV13ExecutionCapabilityOperationKindV1 {
    /// Derive workgroup authority from kernel context.
    WorkgroupDerive,
    /// Derive subgroup authority from workgroup authority.
    SubgroupDerive,
    /// Allocate logical LDS storage.
    LdsAllocate,
    /// Initialize LDS cooperatively by invocation.
    LdsInitializeByInvocation,
    /// Publish initialized LDS storage.
    LdsPublish,
    /// Read published LDS storage.
    LdsReadPublished,
    /// Workgroup execution and memory barrier.
    WorkgroupBarrier,
    /// Subgroup execution and memory barrier.
    SubgroupBarrier,
    /// Workgroup memory fence.
    WorkgroupFence,
    /// Subgroup memory fence.
    SubgroupFence,
    /// Atomic operation with its exact subkind.
    Atomic(ExecutionAtomicKindV1),
    /// Workgroup collective with its exact subkind.
    WorkgroupCollective(ExecutionCollectiveKindV1),
    /// Subgroup collective with its exact subkind.
    SubgroupCollective(ExecutionCollectiveKindV1),
    /// Matrix authority access.
    MatrixAccess,
    /// Asynchronous copy initiation.
    AsyncCopy,
    /// Asynchronous copy wait and publication.
    AsyncWait,
    /// Unsafe raw-memory capability binding.
    RawMemoryBind,
    /// Private-memory allocation.
    PrivateMemoryAllocate,
    /// Workgroup-memory index-space witness.
    WorkgroupMemoryIndex,
    /// Workgroup-memory allocation.
    WorkgroupMemoryAllocate,
    /// Workgroup-memory publication.
    WorkgroupMemoryPublish,
    /// Capability-governed memory load.
    MemoryLoad,
    /// Capability-governed memory store.
    MemoryStore,
}

/// Closed KIR operation family awaiting complete operational translation evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionV13KirOperationFamilyV1 {
    /// Constant operation.
    Constant,
    /// Target-neutral intrinsic.
    Intrinsic,
    /// Memory intrinsic.
    MemoryIntrinsic,
    /// Unary scalar operation.
    Unary,
    /// Binary scalar operation.
    Binary,
    /// Comparison.
    Compare,
    /// Scalar or pointer cast.
    Cast,
    /// Scalar select.
    Select,
    /// Function call.
    Call,
    /// Stack/private allocation.
    Alloca,
    /// Slice-length projection.
    SliceLength,
    /// Slice-data projection.
    SliceData,
    /// Pointer element addressing.
    GetElementPointer,
    /// Unconditional load.
    Load,
    /// Guarded load.
    GuardedLoad,
    /// Guarded store.
    GuardedStore,
    /// Unconditional store.
    Store,
    /// Legacy barrier operation.
    Barrier,
    /// Legacy atomic operation.
    Atomic,
    /// Memory fence.
    Fence,
    /// Workgroup barrier.
    WorkgroupBarrier,
    /// Workgroup-memory declaration.
    WorkgroupMemory,
    /// Matrix operation.
    Matrix,
    /// gfx950 LDS transpose operation.
    Gfx950LdsTranspose,
    /// Wave operation.
    Wave,
    /// Target inline assembly.
    InlineAssembly,
    /// Kernel-context issuance.
    KernelContextIssue,
    /// Global capability binding.
    GlobalCapabilityBind,
    /// Global capability index projection.
    GlobalCapabilityIndex,
    /// Typed V13 execution-capability operation.
    ExecutionCapability(ProductionV13ExecutionCapabilityOperationKindV1),
    /// Return terminator.
    ReturnTerminator,
    /// Unconditional branch terminator.
    BranchTerminator,
    /// Conditional branch terminator.
    ConditionalBranchTerminator,
    /// Value switch terminator.
    SwitchTerminator,
    /// Integer switch terminator.
    IntegerSwitchTerminator,
    /// Unreachable terminator.
    UnreachableTerminator,
}

/// Exact source location and kind not covered by a complete operational derivation.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionV13OperationalTranslationUnsupportedV1 {
    function_symbol: String,
    block: u32,
    operation: u32,
    family: ProductionV13KirOperationFamilyV1,
}

impl ProductionV13OperationalTranslationUnsupportedV1 {
    /// Returns the exact source function symbol.
    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }

    /// Returns the source block identifier.
    pub const fn block(&self) -> u32 {
        self.block
    }

    /// Returns the source operation ordinal within the block.
    pub const fn operation(&self) -> u32 {
        self.operation
    }

    /// Returns the closed unsupported operation family.
    pub const fn family(&self) -> ProductionV13KirOperationFamilyV1 {
        self.family
    }
}

pub(super) fn collect_unsupported_operational_translation_v1(
    module: &Module,
) -> Box<[ProductionV13OperationalTranslationUnsupportedV1]> {
    let mut unsupported = Vec::new();
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            unsupported.extend(
                block
                    .operations
                    .iter()
                    .enumerate()
                    .map(
                        |(ordinal, operation)| ProductionV13OperationalTranslationUnsupportedV1 {
                            function_symbol: function.id.as_str().to_owned(),
                            block: block.id.0,
                            operation: ordinal as u32,
                            family: operation_family(&operation.kind),
                        },
                    ),
            );
            if let Some(terminator) = &block.terminator {
                unsupported.push(ProductionV13OperationalTranslationUnsupportedV1 {
                    function_symbol: function.id.as_str().to_owned(),
                    block: block.id.0,
                    operation: block.operations.len() as u32,
                    family: terminator_family(terminator),
                });
            }
        }
    }
    unsupported.into_boxed_slice()
}

fn terminator_family(terminator: &Terminator) -> ProductionV13KirOperationFamilyV1 {
    match terminator {
        Terminator::Return { .. } => ProductionV13KirOperationFamilyV1::ReturnTerminator,
        Terminator::Branch { .. } => ProductionV13KirOperationFamilyV1::BranchTerminator,
        Terminator::ConditionalBranch { .. } => {
            ProductionV13KirOperationFamilyV1::ConditionalBranchTerminator
        }
        Terminator::Switch { .. } => ProductionV13KirOperationFamilyV1::SwitchTerminator,
        Terminator::IntegerSwitch { .. } => {
            ProductionV13KirOperationFamilyV1::IntegerSwitchTerminator
        }
        Terminator::Unreachable => ProductionV13KirOperationFamilyV1::UnreachableTerminator,
    }
}

fn operation_family(operation: &OperationKind) -> ProductionV13KirOperationFamilyV1 {
    use ProductionV13KirOperationFamilyV1 as Family;
    match operation {
        OperationKind::Constant(_) => Family::Constant,
        OperationKind::Intrinsic(_) => Family::Intrinsic,
        OperationKind::MemoryIntrinsic(_) => Family::MemoryIntrinsic,
        OperationKind::Unary { .. } => Family::Unary,
        OperationKind::Binary { .. } => Family::Binary,
        OperationKind::Compare { .. } => Family::Compare,
        OperationKind::Cast { .. } => Family::Cast,
        OperationKind::Select { .. } => Family::Select,
        OperationKind::Call { .. } => Family::Call,
        OperationKind::Alloca { .. } => Family::Alloca,
        OperationKind::SliceLength { .. } => Family::SliceLength,
        OperationKind::SliceData { .. } => Family::SliceData,
        OperationKind::GetElementPointer { .. } => Family::GetElementPointer,
        OperationKind::Load { .. } => Family::Load,
        OperationKind::GuardedLoad { .. } => Family::GuardedLoad,
        OperationKind::GuardedStore { .. } => Family::GuardedStore,
        OperationKind::Store { .. } => Family::Store,
        OperationKind::Barrier(_) => Family::Barrier,
        OperationKind::Atomic(_) => Family::Atomic,
        OperationKind::Fence(_) => Family::Fence,
        OperationKind::WorkgroupBarrier(_) => Family::WorkgroupBarrier,
        OperationKind::WorkgroupMemory(_) => Family::WorkgroupMemory,
        OperationKind::Matrix(_) => Family::Matrix,
        OperationKind::Gfx950LdsTranspose(_) => Family::Gfx950LdsTranspose,
        OperationKind::Wave(_) => Family::Wave,
        OperationKind::InlineAssembly(_) => Family::InlineAssembly,
        OperationKind::KernelContextIssue(_) => Family::KernelContextIssue,
        OperationKind::GlobalCapabilityBind(_) => Family::GlobalCapabilityBind,
        OperationKind::GlobalCapabilityIndex(_) => Family::GlobalCapabilityIndex,
        OperationKind::ExecutionCapability(operation) => {
            Family::ExecutionCapability(execution_operation_kind(&operation.operation))
        }
    }
}

fn execution_operation_kind(
    operation: &ExecutionCapabilityOperationV1,
) -> ProductionV13ExecutionCapabilityOperationKindV1 {
    use ExecutionCapabilityOperationV1 as Operation;
    use ProductionV13ExecutionCapabilityOperationKindV1 as Kind;
    match operation {
        Operation::WorkgroupDerive { .. } => Kind::WorkgroupDerive,
        Operation::SubgroupDerive { .. } => Kind::SubgroupDerive,
        Operation::LdsAllocate { .. } => Kind::LdsAllocate,
        Operation::LdsInitializeByInvocation { .. } => Kind::LdsInitializeByInvocation,
        Operation::LdsPublish { .. } => Kind::LdsPublish,
        Operation::LdsReadPublished { .. } => Kind::LdsReadPublished,
        Operation::WorkgroupBarrier { .. } => Kind::WorkgroupBarrier,
        Operation::SubgroupBarrier { .. } => Kind::SubgroupBarrier,
        Operation::WorkgroupFence { .. } => Kind::WorkgroupFence,
        Operation::SubgroupFence { .. } => Kind::SubgroupFence,
        Operation::Atomic { kind, .. } => Kind::Atomic(*kind),
        Operation::WorkgroupCollective { kind, .. } => Kind::WorkgroupCollective(*kind),
        Operation::SubgroupCollective { kind, .. } => Kind::SubgroupCollective(*kind),
        Operation::MatrixAccess { .. } => Kind::MatrixAccess,
        Operation::AsyncCopy { .. } => Kind::AsyncCopy,
        Operation::AsyncWait { .. } => Kind::AsyncWait,
        Operation::RawMemoryBind { .. } => Kind::RawMemoryBind,
        Operation::PrivateMemoryAllocate { .. } => Kind::PrivateMemoryAllocate,
        Operation::WorkgroupMemoryIndex { .. } => Kind::WorkgroupMemoryIndex,
        Operation::WorkgroupMemoryAllocate { .. } => Kind::WorkgroupMemoryAllocate,
        Operation::WorkgroupMemoryPublish { .. } => Kind::WorkgroupMemoryPublish,
        Operation::MemoryLoad { .. } => Kind::MemoryLoad,
        Operation::MemoryStore { .. } => Kind::MemoryStore,
    }
}
