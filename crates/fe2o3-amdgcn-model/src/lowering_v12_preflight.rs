//! Backend admission is narrower than structural Kernel IR verification.

use super::{LoweringDiagnosticCode, LoweringErrors, LoweringLocation};
use fe2o3_kernel_ir::{Module, OperationKind, Type};

/// Check the whole module before graph selection or textual emission. Uncalled
/// declarations, unreachable blocks, and dead results are still input syntax.
pub(super) fn reject_unsupported_v12_module(module: &Module) -> Result<(), LoweringErrors> {
    reject_unsupported_module(module, None, false)
}

pub(super) fn reject_unsupported_v16_module(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV16,
) -> Result<(), LoweringErrors> {
    reject_unsupported_module(owner.module(), Some(OrderedProfile::RegionV16), false)
}

pub(super) fn reject_unsupported_v17_module(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17,
) -> Result<(), LoweringErrors> {
    reject_unsupported_module(owner.module(), Some(OrderedProfile::ProgramV17), false)
}

/// Only a genuine V18 owner may retain its validated layout table. The complete
/// executable scan below still refuses every storage type and operation, so
/// unreferenced declarations never imply storage-aware lowering support.
pub(super) fn reject_unsupported_v18_module(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
) -> Result<(), LoweringErrors> {
    reject_unsupported_module(owner.module(), None, true)
}

#[derive(Clone, Copy, PartialEq)]
enum OrderedProfile {
    RegionV16,
    ProgramV17,
}

fn reject_unsupported_module(
    module: &Module,
    ordered: Option<OrderedProfile>,
    checked_v18_metadata: bool,
) -> Result<(), LoweringErrors> {
    if !checked_v18_metadata && !module.storage_layouts.is_empty() {
        return Err(LoweringErrors::one(
            LoweringLocation::module(module),
            LoweringDiagnosticCode::UnsupportedType,
            "legacy AMDGPU lowering does not admit module-owned storage layouts",
        ));
    }
    for function in &module.functions {
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            if contains_unsupported_type(ty) {
                return Err(LoweringErrors::one(
                    LoweringLocation::device_function(module, function),
                    LoweringDiagnosticCode::UnsupportedType,
                    "AMDGPU LLVM lowering does not support V12 vector or V15 execution types in function signatures",
                ));
            }
        }
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            if block
                .parameters
                .iter()
                .any(|parameter| contains_unsupported_type(&parameter.ty))
            {
                return Err(LoweringErrors::one(
                    LoweringLocation::device_block(module, function, block.id),
                    LoweringDiagnosticCode::UnsupportedType,
                    "AMDGPU LLVM lowering does not support V12 vector or V15 execution block parameters",
                ));
            }
            for (ordinal, operation) in block.operations.iter().enumerate() {
                // Keep this exhaustive: a new operation with embedded types or
                // compiler effects must receive an explicit backend decision.
                let embedded = match &operation.kind {
                    OperationKind::Gfx942OrderedRegion(_) => {
                        if ordered != Some(OrderedProfile::RegionV16) {
                            return Err(LoweringErrors::one(
                                LoweringLocation::device_operation(
                                    module, function, block.id, ordinal,
                                ),
                                LoweringDiagnosticCode::UnsupportedOperation,
                                "ordered regions require the exact canonical V16 owner entry point",
                            ));
                        }
                        None
                    }
                    OperationKind::Gfx942OrderedProgram(_) => {
                        if ordered != Some(OrderedProfile::ProgramV17) {
                            return Err(LoweringErrors::one(
                                LoweringLocation::device_operation(
                                    module, function, block.id, ordinal,
                                ),
                                LoweringDiagnosticCode::UnsupportedOperation,
                                "ordered programs require the exact canonical V17 owner entry point",
                            ));
                        }
                        None
                    }
                    OperationKind::Gfx942CompleteBodyDeclaration(_)
                    | OperationKind::Gfx942CompleteBodyStep(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "complete bodies require the exact canonical V19 owner entry point",
                        ));
                    }
                    OperationKind::Gfx942PhysicalEntryDeclaration(_)
                    | OperationKind::Gfx942PhysicalEntryStep(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "physical entries require the exact canonical V20 owner entry point",
                        ));
                    }
                    OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
                    | OperationKind::Gfx942PhysicalGlobalCopyStep(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "physical global copies require the exact canonical V21 owner entry point",
                        ));
                    }
                    OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
                    | OperationKind::Gfx942PhysicalLdsExchangeStep(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "physical LDS exchanges require the exact canonical V22 owner entry point",
                        ));
                    }
                    OperationKind::Storage(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "storage operations require storage-aware AMDGPU lowering",
                        ));
                    }
                    OperationKind::Execution(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "AMDGPU LLVM lowering does not support V15 execution operations",
                        ));
                    }
                    OperationKind::VerificationContract(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "AMDGPU LLVM lowering does not support V12 verification-contract events",
                        ));
                    }
                    OperationKind::VectorLoad(_)
                    | OperationKind::VectorStore(_)
                    | OperationKind::VectorLayoutConvert(_) => {
                        return Err(LoweringErrors::one(
                            LoweringLocation::device_operation(module, function, block.id, ordinal),
                            LoweringDiagnosticCode::UnsupportedOperation,
                            "AMDGPU LLVM lowering does not support V12 vector operations",
                        ));
                    }
                    OperationKind::Intrinsic(intrinsic) => Some(&intrinsic.result_type),
                    OperationKind::Cast { to, .. } => Some(to),
                    OperationKind::Alloca { element, .. } => Some(element),
                    OperationKind::WorkgroupMemory(memory) => Some(&memory.element),
                    OperationKind::Constant(_)
                    | OperationKind::MemoryIntrinsic(_)
                    | OperationKind::Unary { .. }
                    | OperationKind::Binary { .. }
                    | OperationKind::Compare { .. }
                    | OperationKind::Select { .. }
                    | OperationKind::Call { .. }
                    | OperationKind::SliceLength { .. }
                    | OperationKind::SliceData { .. }
                    | OperationKind::GetElementPointer { .. }
                    | OperationKind::Load { .. }
                    | OperationKind::GuardedLoad { .. }
                    | OperationKind::GuardedStore { .. }
                    | OperationKind::Store { .. }
                    | OperationKind::Barrier(_)
                    | OperationKind::Atomic(_)
                    | OperationKind::Fence(_)
                    | OperationKind::WorkgroupBarrier(_)
                    | OperationKind::Matrix(_)
                    | OperationKind::Gfx950LdsTranspose(_)
                    | OperationKind::Wave(_)
                    | OperationKind::InlineAssembly(_) => None,
                };
                if embedded.is_some_and(contains_unsupported_type)
                    || operation
                        .results
                        .iter()
                        .any(|result| contains_unsupported_type(&result.ty))
                {
                    return Err(LoweringErrors::one(
                        LoweringLocation::device_operation(module, function, block.id, ordinal),
                        LoweringDiagnosticCode::UnsupportedType,
                        "AMDGPU LLVM lowering does not support embedded or result V12 vector or V15 execution types",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn contains_unsupported_type(mut ty: &Type) -> bool {
    loop {
        match ty {
            Type::Vector(_) | Type::Execution(_) | Type::StorageObject(_) => return true,
            Type::Pointer(pointer) => ty = &pointer.pointee,
            Type::Slice(slice) => ty = &slice.element,
            Type::Unit | Type::Scalar(_) => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, CastKind, Constant, FixedVectorTypeV12,
        Function, IntrinsicKind, IntrinsicOperation, Operation, ScalarType, Signature, Terminator,
        ValueDef, ValueId, VectorLayoutV12, WorkgroupMemory, WorkgroupMemoryExtent,
    };

    fn vector() -> Type {
        Type::vector(FixedVectorTypeV12::new(
            ScalarType::F32,
            4,
            VectorLayoutV12::Contiguous,
        ))
    }

    fn module(operation: Operation) -> Module {
        let mut block = BasicBlock::new(BlockId(7));
        block.operations.push(operation);
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("preflight");
        module.functions.push(Function::internal_helper(
            "uncalled",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module
    }

    #[test]
    fn execution_v15_operations_are_rejected_before_target_emission() {
        use fe2o3_kernel_ir::ExecutionOperationV15 as Execution;
        for execution in [
            Execution::ContextIssue,
            Execution::WorkgroupDerive {
                context: ValueId(0),
            },
            Execution::ScopeEnd {
                workgroup: ValueId(0),
                discarded: vec![ValueId(1)],
            },
            Execution::MaskedTileLoadU32 {
                workgroup: ValueId(0),
                input: ValueId(1),
                base: ValueId(2),
                lanes: 3,
                elements: 2,
            },
            Execution::TileIntoFragmentU32 {
                tile: ValueId(0),
                lanes: 3,
                elements: 2,
            },
            Execution::FragmentIntoPartsU32 {
                fragment: ValueId(0),
                lanes: 3,
                elements: 2,
            },
        ] {
            let errors = reject_unsupported_v12_module(&module(Operation::new(
                vec![],
                OperationKind::Execution(execution),
            )))
            .unwrap_err();
            assert!(errors.contains(LoweringDiagnosticCode::UnsupportedOperation));
            assert_eq!(errors.diagnostics()[0].location.block, Some(BlockId(7)));
            assert_eq!(errors.diagnostics()[0].location.operation, Some(0));
        }
    }

    #[test]
    fn execution_v15_roles_cannot_hide_in_legacy_embedded_types() {
        use fe2o3_kernel_ir::ExecutionRoleV15 as Role;
        for role in [
            Role::Context,
            Role::Workgroup,
            Role::MaskedTileU32 {
                lanes: 3,
                elements: 2,
            },
            Role::LaneFragmentU32 {
                lanes: 3,
                elements: 2,
            },
        ] {
            let role = Type::Execution(role);
            for ty in [
                role.clone(),
                Type::slice(
                    Type::pointer(role, AddressSpace::Global, AccessMode::ReadOnly),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
            ] {
                for kind in [
                    OperationKind::Alloca {
                        element: ty.clone(),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        value: ValueId(0),
                        to: ty.clone(),
                    },
                    OperationKind::Intrinsic(IntrinsicOperation::new(
                        IntrinsicKind::LaunchExtent {
                            axis: fe2o3_kernel_ir::Axis::X,
                        },
                        ty.clone(),
                    )),
                    OperationKind::WorkgroupMemory(WorkgroupMemory {
                        element: ty.clone(),
                        extent: WorkgroupMemoryExtent::Static(1),
                        alignment: 4,
                    }),
                ] {
                    let errors =
                        reject_unsupported_v12_module(&module(Operation::new(vec![], kind)))
                            .unwrap_err();
                    assert!(errors.contains(LoweringDiagnosticCode::UnsupportedType));
                }
                let errors = reject_unsupported_v12_module(&module(Operation::new(
                    vec![ValueDef::new(ValueId(0), ty)],
                    OperationKind::Constant(Constant::I32(0)),
                )))
                .unwrap_err();
                assert!(errors.contains(LoweringDiagnosticCode::UnsupportedType));
            }
        }
    }

    #[test]
    fn every_legacy_embedded_type_is_checked_without_relying_on_results() {
        // These raw cases intentionally isolate the backend scanner; the public
        // API also verifies them and may reject malformed operands earlier.
        for kind in [
            OperationKind::Alloca {
                element: vector(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 16,
            },
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(0),
                to: Type::pointer(vector(), AddressSpace::Global, AccessMode::ReadOnly),
            },
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::LaunchExtent {
                    axis: fe2o3_kernel_ir::Axis::X,
                },
                vector(),
            )),
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: vector(),
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 16,
            }),
        ] {
            let errors =
                reject_unsupported_v12_module(&module(Operation::new(vec![], kind))).unwrap_err();
            assert!(errors.contains(LoweringDiagnosticCode::UnsupportedType));
            assert_eq!(errors.diagnostics()[0].location.block, Some(BlockId(7)));
            assert_eq!(errors.diagnostics()[0].location.operation, Some(0));
        }
    }

    #[test]
    fn result_only_and_deeply_nested_vectors_cannot_escape_preflight() {
        let mut ty = vector();
        for _ in 0..32 {
            ty = Type::slice(
                Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            );
        }
        let operation = Operation::new(
            vec![ValueDef::new(ValueId(1), ty)],
            OperationKind::Constant(Constant::I32(0)),
        );
        let errors = reject_unsupported_v12_module(&module(operation)).unwrap_err();
        assert!(errors.contains(LoweringDiagnosticCode::UnsupportedType));
    }
}

#[cfg(test)]
mod storage_profile_tests {
    use super::*;

    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, Function, Operation, ScalarType, Signature,
        Terminator, ValueId,
    };

    #[test]
    fn storage_syntax_and_unused_tables_refuse_before_legacy_emission() {
        let mut module = Module::new("storage_profile");
        assert!(reject_unsupported_v12_module(&module).is_ok());
        module
            .storage_layouts
            .push(fe2o3_kernel_ir::StorageLayoutV1 {
                size: 4,
                alignment: 4,
                kind: fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::U32),
            });
        assert!(
            reject_unsupported_v12_module(&module)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsupportedType)
        );
        module.storage_layouts.clear();
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::Project {
                base: ValueId(0),
                step: fe2o3_kernel_ir::StorageProjectionV1::Field(0),
            }),
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::internal_helper(
            "dead",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        assert!(
            reject_unsupported_v12_module(&module)
                .unwrap_err()
                .contains(LoweringDiagnosticCode::UnsupportedOperation)
        );
    }

    #[test]
    fn storage_pointees_cannot_enter_the_legacy_target_type_profile() {
        let storage = Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(0));
        for ty in [
            storage.clone(),
            Type::pointer(
                storage.clone(),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
            Type::slice(storage, AddressSpace::Global, AccessMode::ReadOnly),
        ] {
            assert!(contains_unsupported_type(&ty));
            let mut module = Module::new("dead_signature");
            module.functions.push(Function::external_import(
                "dead",
                Signature::new(vec![ty], vec![]),
            ));
            assert!(
                reject_unsupported_v12_module(&module)
                    .unwrap_err()
                    .contains(LoweringDiagnosticCode::UnsupportedType)
            );
        }
        assert!(!contains_unsupported_type(&Type::Scalar(ScalarType::U32)));
    }
}
