use super::*;
use crate::{
    AccessMode, AddressSpace, Axis, BasicBlock, BlockId, CastKind, Constant, Function,
    IntrinsicKind, IntrinsicOperation, Operation, ScalarType, Signature, Terminator, ValueDef,
    ValueId, WorkgroupMemory, WorkgroupMemoryExtent, verify_module, verify_module_ref,
    verify_module_with_capabilities,
};

fn nested_pointer(depth: usize) -> Type {
    (0..depth).fold(Type::Scalar(ScalarType::U32), |pointee, _| {
        Type::pointer(pointee, AddressSpace::Private, AccessMode::ReadWrite)
    })
}

fn module_with_operation(kind: OperationKind, results: Vec<ValueDef>) -> Module {
    let function = Function::internal_helper(
        "function",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(7),
            parameters: vec![],
            operations: vec![Operation::new(results, kind)],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    let mut module = Module::new("module");
    module.functions.push(function);
    module
}

#[test]
fn public_verifier_accepts_the_exact_wire_type_depth_boundary() {
    let function = Function::external_import(
        "function",
        Signature::new(vec![nested_pointer(crate::MAX_TYPE_DEPTH_V1)], vec![]),
    );
    let mut module = Module::new("module");
    module.functions.push(function);

    assert!(verify_module(&module).is_ok());
    assert_eq!(verify_module_ref(&module).unwrap().module(), &module);
    assert!(verify_module_with_capabilities(&module, &BTreeSet::new()).is_ok());
}

#[test]
fn public_verifier_rejects_deep_in_memory_types_before_recursive_formatting() {
    let function = Function::external_import(
        "deep-function",
        Signature::new(vec![nested_pointer(crate::MAX_TYPE_DEPTH_V1 + 64)], vec![]),
    );
    let mut module = Module::new("deep-module");
    module.functions.push(function);

    let diagnostics = verify_module(&module).unwrap_err().into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, DiagnosticCode::ResourceLimit);
    assert_eq!(
        diagnostics[0].message,
        PUBLIC_VERIFIER_TYPE_DEPTH_MESSAGE_V1
    );
    assert_eq!(diagnostics[0].location.module, module.id);
    assert_eq!(
        diagnostics[0].location.function.as_ref().unwrap().as_str(),
        "deep-function"
    );
    assert_eq!(diagnostics[0].location.block, None);
    assert_eq!(diagnostics[0].location.operation, None);
}

#[test]
fn preflight_covers_every_recursive_operation_type_owner() {
    let deep = nested_pointer(crate::MAX_TYPE_DEPTH_V1 + 1);
    let cases = [
        module_with_operation(
            OperationKind::Constant(Constant::U32(0)),
            vec![ValueDef::new(ValueId(0), deep.clone())],
        ),
        module_with_operation(
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(0),
                to: deep.clone(),
            },
            vec![],
        ),
        module_with_operation(
            OperationKind::Alloca {
                element: deep.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
            vec![],
        ),
        module_with_operation(
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::LaunchExtent { axis: Axis::X },
                deep.clone(),
            )),
            vec![],
        ),
        module_with_operation(
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element: deep,
                extent: WorkgroupMemoryExtent::Static(1),
                alignment: 4,
            }),
            vec![],
        ),
    ];

    for module in cases {
        let location = first_excessive_type_depth_location_v1(&module).unwrap();
        assert_eq!(location.module, module.id);
        assert_eq!(location.block, Some(BlockId(7)));
        assert_eq!(location.operation, Some(0));
    }
}

#[test]
fn preflight_covers_signature_and_block_parameter_types() {
    let deep = nested_pointer(crate::MAX_TYPE_DEPTH_V1 + 1);
    let signature_function =
        Function::external_import("signature", Signature::new(vec![], vec![deep.clone()]));
    let mut signature_module = Module::new("signature-module");
    signature_module.functions.push(signature_function);
    let location = first_excessive_type_depth_location_v1(&signature_module).unwrap();
    assert_eq!(location.function.unwrap().as_str(), "signature");
    assert_eq!(location.block, None);

    let function = Function::internal_helper(
        "block",
        Signature::new(vec![], vec![]),
        vec![],
        vec![BasicBlock {
            id: BlockId(9),
            parameters: vec![ValueDef::new(ValueId(0), deep)],
            operations: vec![],
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    let mut block_module = Module::new("block-module");
    block_module.functions.push(function);
    let location = first_excessive_type_depth_location_v1(&block_module).unwrap();
    assert_eq!(location.function.unwrap().as_str(), "block");
    assert_eq!(location.block, Some(BlockId(9)));
    assert_eq!(location.operation, None);
}

#[test]
fn public_shared_engine_preserves_ordinary_diagnostics_and_capability_checks() {
    let module = Module::new("");
    let diagnostics = verify_module(&module).unwrap_err().into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidIdentity);

    let supported = BTreeSet::from([TargetCapability::DynamicWorkgroupMemory]);
    let diagnostics = verify_module_with_capabilities(&Module::new("module"), &supported)
        .unwrap_err()
        .into_diagnostics();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidCapability);
}
