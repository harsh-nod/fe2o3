use super::*;

fn entry() -> Module {
    module_for(atomic_operation(ExecutionAtomicKindV1::BindGlobalView), 0)
}

#[test]
fn atomic_entry_custody_readwrite_round_trip_preserves_atomic_role() {
    let module = entry();
    assert_eq!(
        module.functions[0].signature.parameters[2],
        Type::slice(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadWrite
        )
    );
    let Type::ExecutionCapability(capability) = &target_operation(&module).results[0].ty else {
        panic!("atomic view result");
    };
    assert!(matches!(
        capability.role,
        ExecutionCapabilityRoleV1::MemoryView {
            access: ExecutionMemoryAccessV1::AtomicReadWrite,
            initialization: ExecutionMemoryInitializationV1::FullyInitialized,
            space: ExecutionMemoryAddressSpaceV1::Global,
            atomic_scope: Some(ExecutionMemoryScopeV1::Device),
            ..
        }
    ));
    let payload = encode_execution_capability_contract_v1(target_contract(&module)).unwrap();
    let owner = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let bytes = owner.into_canonical_bytes();
    assert_eq!(&bytes[8..10], &KERNEL_IR_VERSION_V13.to_le_bytes());
    let (replayed, decoded) =
        VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(bytes).unwrap();
    replayed.revalidate().unwrap();
    assert_eq!(decoded, module);
    assert_eq!(
        encode_execution_capability_contract_v1(target_contract(&decoded)).unwrap(),
        payload
    );
}

#[test]
fn atomic_entry_custody_historical_readonly_and_writeonly_fail_verification() {
    let baseline = entry();
    let payload = encode_execution_capability_contract_v1(target_contract(&baseline)).unwrap();
    for access in [AccessMode::ReadOnly, AccessMode::WriteOnly] {
        let mut changed = baseline.clone();
        let Type::Slice(slice) = &mut changed.functions[0].signature.parameters[2] else {
            unreachable!();
        };
        slice.access = access;
        // Historical readonly bytes remain parseable, not qualified for execution.
        let bytes = encode_module_v13(&changed).unwrap();
        assert_eq!(decode_module_v13(&bytes).unwrap(), changed);
        assert_eq!(
            encode_execution_capability_contract_v1(target_contract(&changed)).unwrap(),
            payload
        );
        assert_invalid_execution_capability(&changed, "atomic entry physical access");
        match VerifiedCanonicalKernelIrV13::from_canonical_bytes(bytes).unwrap_err() {
            VerifiedCanonicalKernelIrErrorV13::Verification(errors) => {
                assert!(
                    errors.contains(DiagnosticCode::InvalidExecutionCapability),
                    "{errors}"
                );
            }
            other => {
                panic!("must fail semantic verification, not historical byte parsing: {other:?}")
            }
        }
    }
}

#[test]
fn atomic_entry_custody_does_not_widen_result_role_or_scope() {
    for access in [
        ExecutionMemoryAccessV1::ReadOnly,
        ExecutionMemoryAccessV1::ExclusiveReadWrite,
    ] {
        let mut changed = entry();
        let Type::ExecutionCapability(capability) =
            &mut target_operation_mut(&mut changed).results[0].ty
        else {
            unreachable!();
        };
        let ExecutionCapabilityRoleV1::MemoryView {
            access: result_access,
            ..
        } = &mut capability.role
        else {
            unreachable!();
        };
        *result_access = access;
        assert_invalid_execution_capability(&changed, "atomic entry result access");
    }
    let mut changed = entry();
    let Type::ExecutionCapability(capability) =
        &mut target_operation_mut(&mut changed).results[0].ty
    else {
        unreachable!();
    };
    let ExecutionCapabilityRoleV1::MemoryView { atomic_scope, .. } = &mut capability.role else {
        unreachable!();
    };
    *atomic_scope = Some(ExecutionMemoryScopeV1::Workgroup);
    assert_invalid_execution_capability(&changed, "atomic entry result scope");
}
