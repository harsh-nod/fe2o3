use super::*;
use crate::verification_execution_lifecycle_v15::tests::fixture;
use crate::{CanonicalKernelIrWorkBudgetV1, ValueDef};

fn instance_position(bytes: &[u8], operation: &ExecutionOperationV15) -> usize {
    let instance =
        encode_semantic_operation_instance_id(operation.semantic_instance_id_v3().unwrap());
    let matches: Vec<_> = bytes
        .windows(instance.len())
        .enumerate()
        .filter_map(|(position, window)| (window == instance).then_some(position))
        .collect();
    assert_eq!(matches.len(), 1);
    matches[0]
}

#[test]
fn execution_v15_wire_roundtrips_all_roles_operations_and_geometry_boundaries() {
    for elements in [1, 4, 125] {
        let module = fixture(elements);
        let bytes = encode_module_v15(&module).unwrap();
        assert_eq!(&bytes[8..10], &15_u16.to_le_bytes());
        let decoded = decode_module_v15(&bytes).unwrap();
        assert_eq!(module, decoded);
        crate::verify_module(&decoded).unwrap();
        assert_eq!(encode_module_v15(&decoded).unwrap(), bytes);
        assert!(encode_module_v8(&module).is_err());
        assert!(encode_module_v9(&module).is_err());
        assert!(encode_module_v12(&module).is_err());
        assert!(decode_module_v8(&bytes).is_err());
        assert!(decode_module_v9(&bytes).is_err());
        assert!(decode_module_v12(&bytes).is_err());
    }
    for role in [
        ExecutionRoleV15::Context,
        ExecutionRoleV15::Workgroup,
        ExecutionRoleV15::MaskedTileU32 {
            lanes: 1,
            elements: 1,
        },
        ExecutionRoleV15::LaneFragmentU32 {
            lanes: 256,
            elements: 125,
        },
    ] {
        let mut module = fixture(1);
        module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
            Type::Execution(role);
        // The codec preserves a raw role; semantic validity is checked separately.
        assert_eq!(
            decode_module_v15(&encode_module_v15(&module).unwrap()).unwrap(),
            module
        );
    }
}

#[test]
fn execution_v15_wire_keeps_old_bytes_and_refuses_historical_versions() {
    let mut plain = fixture(1);
    plain.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .clear();
    let old = encode_module_v12(&plain).unwrap();
    assert_eq!(
        encode_module_v12(&decode_module_v15(&old).unwrap()).unwrap(),
        old
    );
    for version in [0_u16, 13, 14, 16] {
        let mut bytes = old.clone();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        assert!(matches!(decode_module_v15(&bytes),
            Err(KernelIrDecodeError::UnknownVersion(actual)) if actual == version));
    }
}

#[test]
fn execution_v15_wire_rejects_wrong_registered_tag_and_invalid_raw_geometry() {
    let module = fixture(1);
    let mut bytes = encode_module_v15(&module).unwrap();
    let position = instance_position(
        &bytes,
        &ExecutionOperationV15::WorkgroupDerive {
            context: ValueId(10),
        },
    );
    assert_eq!(bytes[position - 5], 33);
    bytes[position - 5] = 32;
    assert!(matches!(
        decode_module_v15(&bytes),
        Err(KernelIrDecodeError::InvalidSemanticOperationInstance)
    ));
    for (lanes, elements) in [(0, 1), (257, 1), (1, 0), (1, 126)] {
        let mut module = fixture(1);
        module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind =
            OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                workgroup: ValueId(11),
                input: ValueId(0),
                base: ValueId(1),
                lanes,
                elements,
            });
        assert!(encode_module_v15(&module).is_err());
        module.functions[0].body.as_mut().unwrap().blocks[0].operations[2].results[0].ty =
            Type::Execution(ExecutionRoleV15::MaskedTileU32 { lanes, elements });
        assert!(encode_module_v15(&module).is_err());
    }
}

fn discarded_fixture() -> Module {
    let mut module = fixture(1);
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.truncate(3);
    let mut second = operations[2].clone();
    second.results = vec![ValueDef::new(
        ValueId(14),
        Type::Execution(ExecutionRoleV15::MaskedTileU32 {
            lanes: 64,
            elements: 1,
        }),
    )];
    operations.push(second);
    operations.push(Operation::new(
        vec![],
        OperationKind::Execution(ExecutionOperationV15::ScopeEnd {
            workgroup: ValueId(11),
            discarded: vec![ValueId(12), ValueId(14)],
        }),
    ));
    module
}

#[test]
fn execution_v15_wire_bounds_and_orders_scope_discard_before_allocation() {
    let module = discarded_fixture();
    crate::verify_module(&module).unwrap();
    let bytes = encode_module_v15(&module).unwrap();
    assert_eq!(decode_module_v15(&bytes).unwrap(), module);
    let position = instance_position(
        &bytes,
        &ExecutionOperationV15::ScopeEnd {
            workgroup: ValueId(11),
            discarded: vec![ValueId(12), ValueId(14)],
        },
    );
    for second in [12_u32, 11] {
        let mut altered = bytes.clone();
        // SO3 header20 + discard_count4 + workgroup4 + first discard4.
        altered[position + 32..position + 36].copy_from_slice(&second.to_le_bytes());
        assert!(matches!(
            decode_module_v15(&altered),
            Err(KernelIrDecodeError::InvalidSemanticOperationInstance)
        ));
    }
    let mut overbound = bytes;
    overbound[position + 20..position + 24]
        .copy_from_slice(&(MAX_VALUE_ARGUMENTS_V1 as u32).to_le_bytes());
    assert!(matches!(
        decode_module_v15(&overbound),
        Err(KernelIrDecodeError::InvalidSemanticOperationInstance)
    ));
    for discarded in [
        vec![ValueId(14), ValueId(12)],
        vec![ValueId(12), ValueId(12)],
        vec![ValueId(0); MAX_VALUE_ARGUMENTS_V1],
    ] {
        let mut module = module.clone();
        module.functions[0].body.as_mut().unwrap().blocks[0]
            .operations
            .last_mut()
            .unwrap()
            .kind = OperationKind::Execution(ExecutionOperationV15::ScopeEnd {
            workgroup: ValueId(11),
            discarded,
        });
        assert!(encode_module_v15(&module).is_err());
    }
}

#[test]
fn execution_v15_wire_work_is_prepaid_for_descriptor_and_operand_traversals() {
    let module = discarded_fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let bytes = encode_module_with_work_v1(&module, KERNEL_IR_VERSION_V15, &mut work).unwrap();
    let required = work.work();
    assert!(required > 1);
    for limit in [required, required - 1, 0] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let result = encode_module_with_work_v1(&module, KERNEL_IR_VERSION_V15, &mut work);
        if limit == required {
            assert_eq!(result.unwrap(), bytes);
        } else {
            assert!(matches!(result, Err(KernelIrEncodeError::WorkLimit(_))));
        }
    }
}
