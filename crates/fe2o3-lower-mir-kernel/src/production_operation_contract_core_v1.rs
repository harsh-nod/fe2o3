#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NormalizedSynchronizationV1 {
    execution_scope: Option<u8>,
    memory_scope: u8,
    ordering: u8,
    address_space: u8,
}

fn normalize_kir_scope_v1(scope: SynchronizationScope) -> u8 {
    match scope {
        SynchronizationScope::Invocation => 0,
        SynchronizationScope::Subgroup => 1,
        SynchronizationScope::Workgroup => 2,
        SynchronizationScope::Device => 3,
        SynchronizationScope::System => 4,
    }
}

fn normalize_ranked_hierarchy_v1(scope: dialect_gpu::HierarchyAttr) -> u8 {
    match scope {
        dialect_gpu::HierarchyAttr::Lane => 0,
        dialect_gpu::HierarchyAttr::Subgroup => 1,
        dialect_gpu::HierarchyAttr::Workgroup => 2,
        dialect_gpu::HierarchyAttr::Grid => 3,
    }
}

fn normalize_ranked_memory_scope_v1(scope: dialect_gpu::MemoryScopeAttr) -> u8 {
    match scope {
        dialect_gpu::MemoryScopeAttr::Subgroup => 1,
        dialect_gpu::MemoryScopeAttr::Workgroup => 2,
        dialect_gpu::MemoryScopeAttr::Device => 3,
        dialect_gpu::MemoryScopeAttr::System => 4,
    }
}

fn normalize_kir_order_v1(order: MemoryOrdering) -> Option<u8> {
    match order {
        MemoryOrdering::Relaxed => None,
        MemoryOrdering::Acquire => Some(1),
        MemoryOrdering::Release => Some(2),
        MemoryOrdering::AcquireRelease => Some(3),
        MemoryOrdering::SequentiallyConsistent => Some(4),
    }
}

fn normalize_ranked_order_v1(order: dialect_gpu::MemoryOrderAttr) -> u8 {
    match order {
        dialect_gpu::MemoryOrderAttr::Acquire => 1,
        dialect_gpu::MemoryOrderAttr::Release => 2,
        dialect_gpu::MemoryOrderAttr::AcquireRelease => 3,
        dialect_gpu::MemoryOrderAttr::SequentiallyConsistent => 4,
    }
}

fn normalize_kir_address_space_v1(space: AddressSpace) -> u8 {
    match space {
        AddressSpace::Private => 0,
        AddressSpace::Workgroup => 1,
        AddressSpace::Global => 2,
        AddressSpace::Constant => 3,
        AddressSpace::Generic => 4,
    }
}

fn normalize_ranked_address_space_v1(space: dialect_gpu::AddressSpaceAttr) -> u8 {
    match space {
        dialect_gpu::AddressSpaceAttr::Private => 0,
        dialect_gpu::AddressSpaceAttr::Workgroup => 1,
        dialect_gpu::AddressSpaceAttr::Global => 2,
        dialect_gpu::AddressSpaceAttr::Constant => 3,
        dialect_gpu::AddressSpaceAttr::Generic => 4,
    }
}

fn singleton_kir_address_space_v1(
    spaces: &BTreeSet<AddressSpace>,
) -> Result<u8, ProductionMirPlironTranslationErrorV1> {
    if spaces.len() != 1 {
        return Err(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch);
    }
    let space = spaces
        .first()
        .copied()
        .ok_or(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch)?;
    Ok(normalize_kir_address_space_v1(space))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct NormalizedTensorV1 {
    contract: TensorLayoutContractV1,
    active_lanes: u32,
    convergence: u8,
}

fn kir_synchronization_contract_v1(
    operation: &Operation,
) -> Result<Option<NormalizedSynchronizationV1>, ProductionMirPlironTranslationErrorV1> {
    Ok(match &operation.kind {
        OperationKind::Barrier(barrier) => Some(NormalizedSynchronizationV1 {
            execution_scope: Some(normalize_kir_scope_v1(barrier.execution_scope)),
            memory_scope: normalize_kir_scope_v1(barrier.memory_scope),
            ordering: normalize_kir_order_v1(barrier.semantics.ordering)
                .ok_or(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch)?,
            address_space: singleton_kir_address_space_v1(&barrier.semantics.address_spaces)?,
        }),
        OperationKind::WorkgroupBarrier(barrier) => Some(NormalizedSynchronizationV1 {
            execution_scope: Some(normalize_kir_scope_v1(SynchronizationScope::Workgroup)),
            memory_scope: normalize_kir_scope_v1(barrier.memory_scope),
            ordering: normalize_kir_order_v1(barrier.semantics.ordering)
                .ok_or(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch)?,
            address_space: singleton_kir_address_space_v1(&barrier.semantics.address_spaces)?,
        }),
        OperationKind::Fence(fence) => Some(NormalizedSynchronizationV1 {
            execution_scope: None,
            memory_scope: normalize_kir_scope_v1(fence.memory_scope),
            ordering: normalize_kir_order_v1(fence.semantics.ordering)
                .ok_or(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch)?,
            address_space: singleton_kir_address_space_v1(&fence.semantics.address_spaces)?,
        }),
        OperationKind::Gfx950LdsTranspose(Gfx950LdsTransposeOperationV1 {
            kind: Gfx950LdsTransposeOperationKindV1::Publish { .. },
            ..
        }) => Some(NormalizedSynchronizationV1 {
            execution_scope: Some(normalize_kir_scope_v1(SynchronizationScope::Workgroup)),
            memory_scope: normalize_kir_scope_v1(SynchronizationScope::Workgroup),
            ordering: normalize_kir_order_v1(MemoryOrdering::AcquireRelease)
                .ok_or(ProductionMirPlironTranslationErrorV1::SynchronizationMismatch)?,
            address_space: normalize_kir_address_space_v1(AddressSpace::Workgroup),
        }),
        _ => None,
    })
}
fn ranked_synchronization_contract_v1(
    operation: &ProductionRankedOperationV1,
) -> Option<NormalizedSynchronizationV1> {
    match operation {
        ProductionRankedOperationV1::Barrier {
            execution_scope,
            memory_scope,
            address_space,
            order,
        } => Some(NormalizedSynchronizationV1 {
            execution_scope: Some(normalize_ranked_hierarchy_v1(*execution_scope)),
            memory_scope: normalize_ranked_memory_scope_v1(*memory_scope),
            ordering: normalize_ranked_order_v1(*order),
            address_space: normalize_ranked_address_space_v1(*address_space),
        }),
        ProductionRankedOperationV1::Fence {
            memory_scope,
            address_space,
            order,
        } => Some(NormalizedSynchronizationV1 {
            execution_scope: None,
            memory_scope: normalize_ranked_memory_scope_v1(*memory_scope),
            ordering: normalize_ranked_order_v1(*order),
            address_space: normalize_ranked_address_space_v1(*address_space),
        }),
        _ => None,
    }
}
fn kir_tensor_contract_v1(operation: &Operation) -> Option<NormalizedTensorV1> {
    let OperationKind::Matrix(matrix) = &operation.kind else {
        return None;
    };
    let contract = matrix.tensor_layout?;
    Some(NormalizedTensorV1 {
        contract,
        active_lanes: matrix.active_lanes,
        convergence: normalize_kir_scope_v1(matrix.convergence.scope()),
    })
}
fn ranked_tensor_contract_v1(
    operation: &ProductionRankedOperationV1,
) -> Result<Option<NormalizedTensorV1>, ProductionMirPlironTranslationErrorV1> {
    let ProductionRankedOperationV1::TensorLayout {
        contract,
        convergence,
        active_lanes,
        ..
    } = operation
    else {
        return Ok(None);
    };
    let convergence = match convergence {
        dialect_kernel::TensorConvergenceAttr::UniformSubgroup => 1,
        dialect_kernel::TensorConvergenceAttr::UniformWorkgroup => 2,
        dialect_kernel::TensorConvergenceAttr::Divergent
        | dialect_kernel::TensorConvergenceAttr::Opaque => {
            return Err(ProductionMirPlironTranslationErrorV1::TensorContractMismatch);
        }
    };
    Ok(Some(NormalizedTensorV1 {
        contract: *contract,
        active_lanes: *active_lanes,
        convergence,
    }))
}
// The historical adapter and the exact-output adapter share extraction, but
// retain separate resource policies. This core never allocates.
trait OperationContractSinkV1<T> {
    type Error;
    fn work(&mut self, work: usize) -> Result<(), Self::Error>;
    fn emit(&mut self, contract: T) -> Result<(), Self::Error>;
    fn invalid(error: ProductionMirPlironTranslationErrorV1) -> Self::Error;
}

struct LegacyOperationContractSinkV1<T>(Vec<T>);

impl<T> OperationContractSinkV1<T> for LegacyOperationContractSinkV1<T> {
    type Error = ProductionMirPlironTranslationErrorV1;
    fn work(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn emit(&mut self, contract: T) -> Result<(), Self::Error> {
        self.0.push(contract);
        Ok(())
    }
    fn invalid(error: ProductionMirPlironTranslationErrorV1) -> Self::Error {
        error
    }
}

fn visit_kir_synchronization_contracts_v1<
    S: OperationContractSinkV1<NormalizedSynchronizationV1>,
>(
    body: &FunctionBody,
    sink: &mut S,
) -> Result<(), S::Error> {
    for block in &body.blocks {
        sink.work(1)?;
        for operation in &block.operations {
            sink.work(8)?;
            // Payload rejection precedes local-effect enumeration, as before.
            let contract = kir_synchronization_contract_v1(operation).map_err(S::invalid)?;
            let mut count = 0usize;
            operation.try_visit_local_memory_effects_v1(|effect| {
                sink.work(1)?;
                if matches!(
                    effect,
                    fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Synchronize { .. }
                        | fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Fence { .. }
                ) {
                    // Only zero, one, or more-than-one matters to this rule.
                    count = count.saturating_add(1);
                }
                Ok::<(), S::Error>(())
            })?;
            if count != usize::from(contract.is_some()) {
                return Err(S::invalid(
                    ProductionMirPlironTranslationErrorV1::SynchronizationMismatch,
                ));
            }
            if let Some(contract) = contract {
                sink.emit(contract)?;
            }
        }
    }
    Ok(())
}

fn visit_ranked_synchronization_contracts_v1<
    S: OperationContractSinkV1<NormalizedSynchronizationV1>,
>(
    kernel: &fe2o3_pliron::ProductionRankedKernelV1,
    sink: &mut S,
) -> Result<(), S::Error> {
    for block in kernel.blocks() {
        sink.work(1)?;
        for operation in block.operations() {
            sink.work(8)?;
            if let Some(contract) = ranked_synchronization_contract_v1(operation) {
                sink.emit(contract)?;
            }
        }
    }
    Ok(())
}

fn visit_kir_tensor_contracts_v1<S: OperationContractSinkV1<NormalizedTensorV1>>(
    body: &FunctionBody,
    sink: &mut S,
) -> Result<(), S::Error> {
    for block in &body.blocks {
        sink.work(1)?;
        for operation in &block.operations {
            sink.work(2 + std::mem::size_of::<NormalizedTensorV1>())?;
            if let Some(contract) = kir_tensor_contract_v1(operation) {
                sink.emit(contract)?;
            }
        }
    }
    Ok(())
}

fn visit_ranked_tensor_contracts_v1<S: OperationContractSinkV1<NormalizedTensorV1>>(
    kernel: &fe2o3_pliron::ProductionRankedKernelV1,
    sink: &mut S,
) -> Result<(), S::Error> {
    for block in kernel.blocks() {
        sink.work(1)?;
        for operation in block.operations() {
            sink.work(2 + std::mem::size_of::<NormalizedTensorV1>())?;
            if let Some(contract) = ranked_tensor_contract_v1(operation).map_err(S::invalid)? {
                sink.emit(contract)?;
            }
        }
    }
    Ok(())
}

fn kir_synchronization_contracts_v1(
    body: &FunctionBody,
) -> Result<Vec<NormalizedSynchronizationV1>, ProductionMirPlironTranslationErrorV1> {
    let mut sink = LegacyOperationContractSinkV1(Vec::new());
    visit_kir_synchronization_contracts_v1(body, &mut sink)?;
    sink.0.sort_unstable();
    Ok(sink.0)
}

fn ranked_synchronization_contracts_v1(
    kernel: &fe2o3_pliron::ProductionRankedKernelV1,
) -> Result<Vec<NormalizedSynchronizationV1>, ProductionMirPlironTranslationErrorV1> {
    let mut sink = LegacyOperationContractSinkV1(Vec::new());
    visit_ranked_synchronization_contracts_v1(kernel, &mut sink)?;
    sink.0.sort_unstable();
    Ok(sink.0)
}

fn kir_tensor_contracts_v1(
    body: &FunctionBody,
) -> Result<Vec<NormalizedTensorV1>, ProductionMirPlironTranslationErrorV1> {
    let mut sink = LegacyOperationContractSinkV1(Vec::new());
    visit_kir_tensor_contracts_v1(body, &mut sink)?;
    sink.0.sort_unstable();
    Ok(sink.0)
}

fn ranked_tensor_contracts_v1(
    kernel: &fe2o3_pliron::ProductionRankedKernelV1,
) -> Result<Vec<NormalizedTensorV1>, ProductionMirPlironTranslationErrorV1> {
    let mut sink = LegacyOperationContractSinkV1(Vec::new());
    visit_ranked_tensor_contracts_v1(kernel, &mut sink)?;
    sink.0.sort_unstable();
    Ok(sink.0)
}
