//! Closed local classifications. These enumerate obligations, not their proofs.
use super::*;
use fe2o3_kernel_ir::{KirLocalMemoryEffectRefV1 as Effect, OperationKind as Op};

pub(super) fn operation(ordinal: usize, op: &Op) -> Result<(OperationClass, Obligations)> {
    use Obligation as O;
    use OperationClass as C;
    let scalar = Obligations::NONE.with(O::ExactScalarSemantics);
    let memory = scalar.with(O::Provenance).with(O::Bounds).with(O::Lifetime);
    let ordered = Obligations::NONE.with(O::Ordering);
    // Deliberately exhaustive. Later wire families cannot inherit a "pure" default.
    let result = match op {
        Op::Constant(_) => (C::Constant, scalar),
        Op::Intrinsic(intrinsic) => {
            match intrinsic.kind {
                fe2o3_kernel_ir::IntrinsicKind::InvocationIndex { .. }
                | fe2o3_kernel_ir::IntrinsicKind::LaunchExtent { .. } => {}
            }
            (C::Intrinsic, scalar.with(O::Launch))
        }
        Op::MemoryIntrinsic(_) => (
            C::MemoryIntrinsic,
            memory.with(O::Initialization).with(O::RaceFreedom),
        ),
        Op::Unary { .. } => (C::Unary, scalar.with(O::TrapBehavior)),
        Op::Binary { .. } => (C::Binary, scalar.with(O::TrapBehavior)),
        Op::Compare { .. } => (C::Compare, scalar),
        Op::Cast { .. } => (C::Cast, scalar.with(O::TrapBehavior)),
        Op::Select { .. } => (C::Select, scalar),
        Op::Call { .. } => (
            C::Call,
            scalar
                .with(O::CallEffects)
                .with(O::CallControl)
                .with(O::TrapBehavior)
                .with(O::Convergence),
        ),
        Op::Alloca { .. } => (C::Alloca, memory.with(O::Initialization)),
        Op::SliceLength { .. } => (C::SliceLength, scalar.with(O::Provenance)),
        Op::SliceData { .. } => (C::SliceData, scalar.with(O::Provenance)),
        Op::GetElementPointer { .. } => (C::GetElementPointer, memory),
        Op::Load { .. } => (C::Load, memory.with(O::Initialization).with(O::RaceFreedom)),
        Op::GuardedLoad { .. } => (
            C::GuardedLoad,
            memory
                .with(O::Initialization)
                .with(O::RaceFreedom)
                .with(O::Control),
        ),
        Op::GuardedStore { .. } => (
            C::GuardedStore,
            memory.with(O::RaceFreedom).with(O::Control),
        ),
        Op::Store { .. } => (C::Store, memory.with(O::RaceFreedom)),
        Op::Barrier(_) => (C::Barrier, ordered.with(O::Convergence)),
        Op::Atomic(_) => (
            C::Atomic,
            memory
                .with(O::Initialization)
                .with(O::Ordering)
                .with(O::RaceFreedom),
        ),
        Op::Fence(_) => (C::Fence, ordered),
        Op::WorkgroupBarrier(_) => (C::WorkgroupBarrier, ordered.with(O::Convergence)),
        Op::WorkgroupMemory(_) => (
            C::WorkgroupMemory,
            memory.with(O::Initialization).with(O::Launch),
        ),
        Op::Matrix(_) => (
            C::Matrix,
            scalar.with(O::Tensor).with(O::Convergence).with(O::Target),
        ),
        Op::Gfx950LdsTranspose(_) => (
            C::LdsTranspose,
            memory.with(O::Tensor).with(O::Convergence).with(O::Target),
        ),
        Op::Wave(_) => (C::Wave, scalar.with(O::Convergence).with(O::Target)),
        Op::InlineAssembly(_) => (
            C::InlineAssembly,
            memory
                .with(O::Assembly)
                .with(O::Ordering)
                .with(O::TrapBehavior)
                .with(O::Convergence)
                .with(O::Target)
                .with(O::CallControl),
        ),
        Op::VerificationContract(_) => (
            C::VerificationContract,
            ordered.with(O::Contract).with(O::ReferenceRefinement),
        ),
        Op::VectorLoad(_) => (
            C::VectorLoad,
            memory.with(O::Initialization).with(O::RaceFreedom),
        ),
        Op::VectorStore(_) => (C::VectorStore, memory.with(O::RaceFreedom)),
        Op::VectorLayoutConvert(_) => (C::VectorLayoutConvert, scalar.with(O::Tensor)),
        Op::Storage(_) => return Err(Error::UnsupportedStorageOperation { ordinal }),
        Op::Execution(_) => {
            return Err(Error::UnsupportedOperation {
                ordinal,
                wire_version: 15,
            });
        }
        Op::Gfx942OrderedRegion(_) => {
            return Err(Error::UnsupportedOperation {
                ordinal,
                wire_version: 16,
            });
        }
        Op::Gfx942OrderedProgram(_) => {
            return Err(Error::UnsupportedOperation {
                ordinal,
                wire_version: 17,
            });
        }
        Op::Gfx942CompleteBodyDeclaration(_) | Op::Gfx942CompleteBodyStep(_) => {
            return Err(Error::UnsupportedOperation {
                ordinal,
                wire_version: 19,
            });
        }
        Op::Gfx942PhysicalEntryDeclaration(_) | Op::Gfx942PhysicalEntryStep(_) => {
            return Err(Error::UnsupportedOperation {
                ordinal,
                wire_version: 20,
            });
        }
        Op::Gfx942PhysicalGlobalCopyDeclaration(_) | Op::Gfx942PhysicalGlobalCopyStep(_) => {
            return Err(Error::UnsupportedOperation {
                ordinal,
                wire_version: 21,
            });
        }
        Op::Gfx942PhysicalLdsExchangeDeclaration(_) | Op::Gfx942PhysicalLdsExchangeStep(_) => {
            return Err(Error::UnsupportedOperation {
                ordinal,
                wire_version: 22,
            });
        }
    };
    Ok(result)
}

pub(super) fn effect(effect: Effect<'_>) -> Obligations {
    use Obligation as O;
    let memory = Obligations::NONE
        .with(O::Provenance)
        .with(O::Bounds)
        .with(O::Lifetime);
    match effect {
        Effect::Allocate(_) => memory.with(O::Initialization),
        Effect::Read(_) => memory.with(O::Initialization).with(O::RaceFreedom),
        Effect::Write(_) => memory.with(O::RaceFreedom),
        Effect::VolatileRead(_) => memory
            .with(O::Initialization)
            .with(O::RaceFreedom)
            .with(O::Ordering),
        Effect::VolatileWrite(_) => memory.with(O::RaceFreedom).with(O::Ordering),
        Effect::Atomic { .. } => memory
            .with(O::Initialization)
            .with(O::RaceFreedom)
            .with(O::Ordering),
        Effect::Synchronize { .. } => Obligations::NONE.with(O::Ordering).with(O::Convergence),
        Effect::Fence { .. } => Obligations::NONE.with(O::Ordering),
    }
}

pub(super) fn function(body: bool) -> (Role, Obligations) {
    if body {
        (Role::DefinedFunction, Obligations::NONE)
    } else {
        (
            Role::Declaration,
            Obligations::NONE
                .with(Obligation::CallEffects)
                .with(Obligation::CallControl),
        )
    }
}

pub(super) fn call() -> Obligations {
    Obligations::NONE
        .with(Obligation::CallEffects)
        .with(Obligation::CallControl)
        .with(Obligation::TrapBehavior)
        .with(Obligation::Convergence)
}

pub(super) fn metadata(kind: CanonicalRankedMetadataKindV1) -> Obligations {
    use CanonicalRankedMetadataKindV1 as K;
    use Obligation as O;
    let pending = Obligations::NONE.with(O::SourceMetadata);
    match kind {
        K::Launch => pending.with(O::Launch),
        K::Memory => pending
            .with(O::Provenance)
            .with(O::Bounds)
            .with(O::Initialization)
            .with(O::RaceFreedom),
        K::Lifetime => pending.with(O::Lifetime),
        K::Tensor => pending.with(O::Tensor).with(O::Convergence),
        K::Pipeline => pending.with(O::Contract).with(O::Ordering),
        K::Numerical => pending.with(O::ExactScalarSemantics),
        K::Refinement => pending.with(O::ReferenceRefinement),
        K::Assembly => pending
            .with(O::Assembly)
            .with(O::Target)
            .with(O::TrapBehavior)
            .with(O::Convergence),
    }
}

/// Every declared capability occurrence and every operation-derived requirement
/// remains anchored to N. The visitor is allocation-free; the caller pays for
/// each publication separately from operation classification before callbacks.
pub(super) fn requirements<O>(
    inventory: &Inventory<'_, O>,
    module: &fe2o3_kernel_ir::Module,
    budget: &mut Budget<'_>,
    mut visit: impl FnMut(RequirementOwner, usize, &mut Budget<'_>) -> Result<()>,
) -> Result<()> {
    for ordinal in 0..module.required_capabilities.len() {
        visit(RequirementOwner::Module, ordinal, budget)?;
    }
    for (kernel, row) in inventory.kernels().iter().enumerate() {
        budget.charge_work(1)?;
        for ordinal in 0..row.kernel.required_capabilities.len() {
            visit(RequirementOwner::Kernel(kernel), ordinal, budget)?;
        }
    }
    for (function, row) in inventory.functions().iter().enumerate() {
        budget.charge_work(1)?;
        for ordinal in 0..row.function.required_capabilities.len() {
            visit(RequirementOwner::Function(function), ordinal, budget)?;
        }
    }
    for (operation, row) in inventory.operations().iter().enumerate() {
        budget.charge_work(
            row.operation
                .required_capability_visitation_work_v1()
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut ordinal = 0usize;
        row.operation.try_visit_required_capabilities_v1(|_| {
            visit(RequirementOwner::Operation(operation), ordinal, budget)?;
            ordinal = add(ordinal, 1)?;
            Ok::<_, Error>(())
        })?;
        budget.charge_work(1)?;
        if let Op::Atomic(atomic) = &row.operation.kind {
            let pointer = inventory
                .definition_for_value(row.coordinate.block.function, atomic.pointer, budget)
                .map_err(|error| match error {
                    crate::CanonicalKirInventoryErrorV1::Resource(error) => Error::Resource(error),
                    crate::CanonicalKirInventoryErrorV1::InconsistentOwner => {
                        Error::InconsistentInventory
                    }
                })?
                .ok_or(Error::InconsistentInventory)?;
            budget.charge_work(1)?;
            if fe2o3_kernel_ir::atomic_pointer_capability_v1(atomic, pointer.ty).is_some() {
                visit(RequirementOwner::AtomicPointer(operation), 0, budget)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod storage_profile_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AddressSpace, MemoryAccess, StorageCopyOverlapV1, StorageOperationV1, StorageProjectionV1,
        ValueId,
    };

    #[test]
    fn storage_has_no_legacy_ranked_classification_or_implied_wire_version() {
        let access = MemoryAccess::new(AddressSpace::Private, 4);
        for storage in [
            StorageOperationV1::Project {
                base: ValueId(0),
                step: StorageProjectionV1::Field(0),
            },
            StorageOperationV1::ReadValue {
                address: ValueId(0),
                access,
            },
            StorageOperationV1::WriteValue {
                address: ValueId(0),
                value: ValueId(1),
                access,
            },
            StorageOperationV1::CopyObject {
                source: ValueId(0),
                destination: ValueId(1),
                source_access: access,
                destination_access: access,
                overlap: StorageCopyOverlapV1::MayOverlap,
            },
        ] {
            assert_eq!(
                operation(7, &Op::Storage(storage)),
                Err(Error::UnsupportedStorageOperation { ordinal: 7 })
            );
        }
        assert!(operation(7, &Op::Constant(fe2o3_kernel_ir::Constant::U32(9))).is_ok());
    }

    #[test]
    fn target_declarations_and_steps_keep_exact_closed_wire_refusals() {
        use fe2o3_kernel_ir::*;

        // Inert typed payloads are sufficient here: classification must reject
        // the family before treating any payload as source or native authority.
        let origin = Gfx942PhysicalEntryOriginVNext {
            root_axes: [[1; 32]; 5],
            mir_body: [2; 32],
            source_signature: [3; 32],
            rustc_fn_abi: [4; 32],
            frontend_bytes_sha256: [5; 32],
        };
        let site = Gfx942PhysicalEntrySourceSiteVNext {
            semantic_block_identity: [6; 32],
            ..Gfx942PhysicalEntrySourceSiteVNext::ZERO
        };
        let block = Gfx942PhysicalEntryBlockContractVNext::ZERO;
        let operations = [
            (
                19,
                Op::Gfx942CompleteBodyDeclaration(Gfx942CompleteBodyDeclarationVNext {
                    origin: Gfx942CompleteBodyOriginVNext {
                        root_axes: origin.root_axes,
                        mir_body: origin.mir_body,
                        semantic_block: [6; 32],
                        source_signature: origin.source_signature,
                        rustc_fn_abi: origin.rustc_fn_abi,
                        frontend_bytes_sha256: origin.frontend_bytes_sha256,
                        raw_block: 0,
                    },
                    registers: Gfx942OrderedProgramRegistersV1::new(3, 4, [0, 1, 2]).unwrap(),
                    parameters: [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
                    labels: [0; 8],
                    block_count: 1,
                    instruction_count: 1,
                }),
            ),
            (
                19,
                Op::Gfx942CompleteBodyStep(Gfx942CompleteBodyStepVNext {
                    authored_block: 0,
                    authored_instruction: 0,
                    instruction: Gfx942ProgramInstructionV1::from_descriptor(0).unwrap(),
                    operands: [Some(ValueId(1)), None],
                }),
            ),
            (
                20,
                Op::Gfx942PhysicalEntryDeclaration(Gfx942PhysicalEntryDeclarationVNext {
                    origin,
                    begin_site: site,
                    parameters: [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
                    block_count: 1,
                    native_instruction_count: 1,
                    workgroup: [64, 1, 1],
                    maximum_workgroups: [2, 1, 1],
                    blocks: [block; 4],
                }),
            ),
            (
                20,
                Op::Gfx942PhysicalEntryStep(Gfx942PhysicalEntryStepVNext {
                    site,
                    native_ordinal: 0,
                    instruction: Gfx942PhysicalEntryInstructionVNext {
                        opcode: Gfx942PhysicalEntryOpcodeV20::Endpgm0,
                        destination: 0,
                        source0: 0,
                        source1: 0,
                        immediate: 0,
                    },
                    operands: [None; 6],
                }),
            ),
            (
                21,
                Op::Gfx942PhysicalGlobalCopyDeclaration(Gfx942PhysicalGlobalCopyDeclarationV1 {
                    origin,
                    begin_site: site,
                    parameters: [ValueId(0), ValueId(1)],
                    native_instruction_count: 1,
                    workgroup: [64, 1, 1],
                    maximum_workgroups: [2, 1, 1],
                    block,
                }),
            ),
            (
                21,
                Op::Gfx942PhysicalGlobalCopyStep(Gfx942PhysicalGlobalCopyStepV1 {
                    site,
                    native_ordinal: 0,
                    instruction: Gfx942PhysicalGlobalCopyInstructionV1 {
                        opcode: Gfx942PhysicalGlobalCopyOpcodeV1::Endpgm0,
                        destination: 0,
                        source0: 0,
                        source1: 0,
                        immediate: 0,
                    },
                    operands: [None; 5],
                }),
            ),
            (
                22,
                Op::Gfx942PhysicalLdsExchangeDeclaration(Gfx942PhysicalLdsExchangeDeclarationV1 {
                    origin,
                    begin_site: site,
                    parameters: [ValueId(0), ValueId(1)],
                    native_instruction_count: 1,
                    workgroup: [64, 1, 1],
                    maximum_workgroups: [2, 1, 1],
                    lds_frame: Gfx942PhysicalLdsExchangeFrameV1 {
                        byte_offset: 0,
                        byte_length: 512,
                        alignment: 4,
                        publication_epoch: 1,
                    },
                    block,
                }),
            ),
            (
                22,
                Op::Gfx942PhysicalLdsExchangeStep(Gfx942PhysicalLdsExchangeStepV1 {
                    site,
                    native_ordinal: 0,
                    instruction: Gfx942PhysicalLdsExchangeInstructionV1 {
                        opcode: Gfx942PhysicalLdsExchangeOpcodeV1::Endpgm0,
                        destination: 0,
                        source0: 0,
                        source1: 0,
                        immediate: 0,
                    },
                    operands: [None; 5],
                }),
            ),
        ];
        for (index, (wire_version, op)) in operations.iter().enumerate() {
            let ordinal = index * 17 + 3;
            assert_eq!(
                operation(ordinal, op),
                Err(Error::UnsupportedOperation {
                    ordinal,
                    wire_version: *wire_version,
                })
            );
        }
        assert_eq!(
            operation(149, &Op::Constant(Constant::U32(7))),
            Ok((
                OperationClass::Constant,
                Obligations::NONE.with(Obligation::ExactScalarSemantics)
            ))
        );
    }
}
