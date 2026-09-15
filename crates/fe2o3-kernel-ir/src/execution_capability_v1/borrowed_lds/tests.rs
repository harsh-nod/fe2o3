use crate::*;
mod fixture {
    use crate::*;
    include!("fixture.rs");
}
use fixture::*;

fn invalid(module: &Module) {
    let errors = verify_module(module).expect_err("borrowed allocation mutation must reject");
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|d| d.code == DiagnosticCode::InvalidExecutionCapability),
        "{errors:?}"
    );
}

#[test]
fn borrowed_lds_exact_source_pair_allocates_once_then_converts_uninitialized_handle() {
    let m = module(true, true);
    verify_module(&m).unwrap();
    let allocation = contract_at(&m, 2);
    assert_eq!(
        allocation.signature.arguments().collect::<Vec<_>>(),
        [id(12)]
    );
    assert_eq!(allocation.operands, [ValueId(1)]);
    assert_ne!(id(12), id(11));
    assert_eq!(
        allocation.operation.memory_effects(),
        [MemoryEffect::Allocate(AddressSpace::Workgroup)]
    );
    assert!(!allocation.operation.transitions_epoch());
    let converted = contract_at(&m, 3);
    assert_eq!(converted.operands, [ValueId(90)]);
    assert!(converted.operation.memory_effects().is_empty());
    assert_eq!(allocation.epoch_before, converted.epoch_before);
    assert_eq!(allocation.workgroup_brand, converted.workgroup_brand);
    assert_eq!(allocation.provenance, converted.provenance);
    assert_eq!(
        decode_module_v13(&encode_module_v13(&m).unwrap()).unwrap(),
        m
    );
}

#[test]
fn borrowed_lds_op30_exact_revision_three_and_five_round_trip_and_truncation() {
    for occurrence in [false, true] {
        let m = module(occurrence, false);
        verify_module(&m).unwrap();
        let c = contract_at(&m, 2);
        let bytes = encode_execution_capability_contract_v1(c).unwrap();
        assert_eq!(&bytes[..2], &[if occurrence { 5 } else { 3 }, 30]);
        assert_eq!(
            decode_execution_capability_contract_v1(&bytes, c.operands.clone()),
            Some(c.clone())
        );
        for revision in [0, 1, 2, 3, 4, 5, 6, 7, 255] {
            if revision == bytes[0] {
                continue;
            }
            let mut changed = bytes.clone();
            changed[0] = revision;
            assert!(
                decode_execution_capability_contract_v1(&changed, c.operands.clone()).is_none(),
                "revision {revision}"
            );
        }
        for end in 0..bytes.len() {
            assert!(
                decode_execution_capability_contract_v1(&bytes[..end], c.operands.clone())
                    .is_none()
            );
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(decode_execution_capability_contract_v1(&trailing, c.operands.clone()).is_none());
    }
}

#[test]
fn borrowed_lds_keeps_old_op2_full_bytes_and_owned_signature() {
    let m = module(false, false);
    let mut old = contract_at(&m, 2).clone();
    old.operation = ExecutionCapabilityOperationV1::LdsAllocate {
        workgroup: id(11),
        lds: id(90),
        element: id(92),
        layout: ExecutionElementLayoutV1 {
            byte_size: 4,
            byte_alignment: 4,
        },
        elements: 64,
    };
    old.signature = ExecutionCapabilitySignatureV1::new(&[id(11)], id(90)).unwrap();
    old.obligations =
        ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(&old.operation));
    // Independent literal field ordering for the frozen revision1/op2 fixture.
    let mut golden = vec![1, 2];
    for tag in [11, 90, 92] {
        golden.extend_from_slice(&[tag; 32]);
    }
    golden.extend_from_slice(&4u32.to_le_bytes());
    golden.extend_from_slice(&4u16.to_le_bytes());
    golden.extend_from_slice(&64u64.to_le_bytes());
    golden.push(1);
    for tag in [11, 90] {
        golden.extend_from_slice(&[tag; 32]);
    }
    golden.extend_from_slice(&18u16.to_le_bytes());
    golden.extend_from_slice(b"borrowed_lds_entry");
    for tag in 1..=6 {
        golden.extend_from_slice(&[tag; 32]);
    }
    for tag in [7, 8] {
        golden.push(1);
        golden.extend_from_slice(&[tag; 32]);
    }
    golden.push(0);
    assert_eq!(old.obligations.bits(), 0x23);
    golden.extend_from_slice(&0x23u16.to_le_bytes());
    golden.extend_from_slice(&[9; 32]);
    golden.extend_from_slice(&[80; 32]);
    golden.extend_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        encode_execution_capability_contract_v1(&old).unwrap(),
        golden
    );
    assert_eq!(
        decode_execution_capability_contract_v1(&golden, old.operands.clone()),
        Some(old.clone())
    );
    old.signature = ExecutionCapabilitySignatureV1::new(&[id(12)], id(90)).unwrap();
    assert!(!old.is_complete());
}

#[test]
fn borrowed_lds_rejects_substituted_reference_owner_epoch_layout_source_and_obligations() {
    for change in 0..13 {
        let mut m = module(true, true);
        let c = contract_mut(&mut m, 2);
        match change {
            0 => c.signature = ExecutionCapabilitySignatureV1::new(&[id(11)], id(90)).unwrap(),
            1 => c.operands[0] = ValueId(0),
            2 => c.operands[0] = ValueId(90),
            3 => c.workgroup_brand = Some([99; 32]),
            4 => c.epoch_before = Some([99; 32]),
            5 => c.epoch_after = Some([99; 32]),
            6 => c.provenance.issuance = [99; 32],
            7 => c.source.occurrence = None,
            8 => {
                c.source.occurrence = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                    [99; 32], [101; 32], [102; 32], 80, 0,
                )
            }
            9 => {
                c.obligations = ExecutionSafetyObligationsV1::from_bits(
                    c.obligations.bits() & !ExecutionSafetyObligationsV1::LIFETIME_VALIDITY,
                )
            }
            10 => {
                c.obligations = ExecutionSafetyObligationsV1::from_bits(
                    c.obligations.bits() & !ExecutionSafetyObligationsV1::ALIASING_VALIDITY,
                )
            }
            11 | 12 => {
                let ExecutionCapabilityOperationV1::LdsAllocateBorrowed {
                    workgroup_reference,
                    layout,
                    ..
                } = &mut c.operation
                else {
                    unreachable!()
                };
                if change == 11 {
                    *workgroup_reference = id(11);
                } else {
                    layout.byte_size = 8;
                }
            }
            _ => unreachable!(),
        }
        invalid(&m);
    }
}

#[test]
fn borrowed_lds_requires_dominating_direct_issuer_and_real_context() {
    let mut m = module(false, false);
    operations_mut(&mut m).swap(1, 2);
    invalid(&m);
    let mut m = module(false, false);
    let issued = operations_mut(&mut m).remove(1).results.remove(0);
    m.functions[0].body.as_mut().unwrap().blocks[0]
        .parameters
        .push(issued);
    invalid(&m);
    let mut m = module(false, false);
    operations_mut(&mut m)[0].kind = OperationKind::Constant(Constant::U32(0));
    invalid(&m);
}

#[test]
fn borrowed_lds_rejects_skipped_issuer_normal_path() {
    let mut m = module(false, false);
    let body = m.functions[0].body.as_mut().unwrap();
    let allocation = body.blocks[0].operations.pop().unwrap();
    let issuer = body.blocks[0].operations.pop().unwrap();
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(8), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    ));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(8),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut issued = BasicBlock::new(BlockId(1));
    issued.operations.push(issuer);
    issued.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut consumer = BasicBlock::new(BlockId(2));
    consumer.operations.push(allocation);
    consumer.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([issued, consumer]);
    invalid(&m);
}

#[test]
fn borrowed_lds_zero_overflow_or_missing_identity_is_not_a_bounded_allocation() {
    for change in 0..3 {
        let mut m = module(false, false);
        let c = contract_mut(&mut m, 2);
        let ExecutionCapabilityOperationV1::LdsAllocateBorrowed {
            element, elements, ..
        } = &mut c.operation
        else {
            unreachable!()
        };
        match change {
            0 => *elements = 0,
            1 => *elements = u64::MAX,
            2 => *element = ExecutionTypeIdentityV1::new([0; 32]),
            _ => unreachable!(),
        }
        assert!(!c.is_complete());
        assert!(encode_execution_capability_contract_v1(c).is_none());
        invalid(&m);
    }
}
