//! Backend admission is narrower than structural Kernel IR verification.

use super::{LoweringDiagnosticCode, LoweringErrors, LoweringLocation};
use fe2o3_kernel_ir::{Module, OperationKind, Type};

/// Check the whole module before graph selection or textual emission. Uncalled
/// declarations, unreachable blocks, and dead results are still input syntax.
pub(super) fn reject_unsupported_v12_module(module: &Module) -> Result<(), LoweringErrors> {
    for function in &module.functions {
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            if contains_vector(ty) {
                return Err(LoweringErrors::one(
                    LoweringLocation::device_function(module, function),
                    LoweringDiagnosticCode::UnsupportedType,
                    "AMDGPU LLVM lowering does not support V12 vector types in function signatures",
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
                .any(|parameter| contains_vector(&parameter.ty))
            {
                return Err(LoweringErrors::one(
                    LoweringLocation::device_block(module, function, block.id),
                    LoweringDiagnosticCode::UnsupportedType,
                    "AMDGPU LLVM lowering does not support V12 vector block parameters",
                ));
            }
            for (ordinal, operation) in block.operations.iter().enumerate() {
                // Keep this exhaustive: a new operation with embedded types or
                // compiler effects must receive an explicit backend decision.
                let embedded = match &operation.kind {
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
                if embedded.is_some_and(contains_vector)
                    || operation
                        .results
                        .iter()
                        .any(|result| contains_vector(&result.ty))
                {
                    return Err(LoweringErrors::one(
                        LoweringLocation::device_operation(module, function, block.id, ordinal),
                        LoweringDiagnosticCode::UnsupportedType,
                        "AMDGPU LLVM lowering does not support embedded or result V12 vector types",
                    ));
                }
            }
        }
    }
    Ok(())
}

fn contains_vector(mut ty: &Type) -> bool {
    loop {
        match ty {
            Type::Vector(_) => return true,
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
