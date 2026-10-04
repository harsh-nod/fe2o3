use super::*;
use crate::{
    BasicBlock, BlockId, Diagnostic, DiagnosticLocation, Function, FunctionId, Module, ModuleId,
    Signature, TargetCapability, Terminator, ValueDef, WorkgroupMemory, WorkgroupMemoryExtent,
};

#[test]
fn pointer_result_diagnostics_preserve_exact_legacy_messages() {
    const ALLOCA_MESSAGE: &str = "result %1 has type Scalar(U32), expected Pointer(PointerType { pointee: Scalar(U32), address_space: Private, access: ReadWrite })";
    const SLICE_MESSAGE: &str = "result %1 has type Scalar(U32), expected Pointer(PointerType { pointee: Scalar(U32), address_space: Global, access: ReadOnly })";
    const WORKGROUP_MESSAGE: &str = "result %1 has type Scalar(U32), expected Pointer(PointerType { pointee: Scalar(U32), address_space: Workgroup, access: ReadWrite })";
    let scalar = Type::Scalar(ScalarType::U32);
    for (kind, parameters, parameter_values, message) in [
        (
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
            vec![],
            vec![],
            ALLOCA_MESSAGE,
        ),
        (
            OperationKind::SliceData { slice: ValueId(0) },
            vec![Type::slice(
                scalar.clone(),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            )],
            vec![ValueId(0)],
            SLICE_MESSAGE,
        ),
        (
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: scalar.clone(),
                extent: WorkgroupMemoryExtent::Static(4),
                alignment: 4,
            }),
            vec![],
            vec![],
            WORKGROUP_MESSAGE,
        ),
    ] {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(1), scalar.clone()),
            kind,
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("m");
        module.functions.push(Function::definition(
            "f",
            Signature::new(parameters, vec![]),
            parameter_values,
            vec![block],
        ));
        let expected = [Diagnostic {
            location: DiagnosticLocation {
                module: ModuleId::new("m"),
                function: Some(FunctionId::new("f")),
                kernel: None,
                block: Some(BlockId(0)),
                operation: Some(0),
            },
            code: DiagnosticCode::TypeMismatch,
            message: message.to_owned(),
        }];
        assert_eq!(
            crate::verify_module(&module).unwrap_err().diagnostics(),
            expected
        );
        assert_eq!(
            crate::verify_module_ref(&module).unwrap_err().diagnostics(),
            expected
        );
        let supported = std::collections::BTreeSet::from([TargetCapability::WorkgroupMemory]);
        assert_eq!(
            crate::verify_module_with_capabilities(&module, &supported)
                .unwrap_err()
                .diagnostics(),
            expected
        );
    }
}
