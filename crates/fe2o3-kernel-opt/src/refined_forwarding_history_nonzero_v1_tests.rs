use super::*;
use fe2o3_kernel_ir::{CheckedBinaryOperator, ComparePredicate, Constant};
use std::mem::{size_of, size_of_val};

fn block(
    id: u32,
    parameters: Vec<ValueDef>,
    operations: Vec<Operation>,
    terminator: Terminator,
) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.parameters = parameters;
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}
fn jump(target: u32, values: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: values.iter().copied().map(ValueId).collect(),
    }
}
fn mutating_module() -> Module {
    let ty = Type::Scalar(ScalarType::U64);
    let private = MemoryAccess::new(AddressSpace::Private, 8);
    let mut module = Module::new("full-history-checked-add-private-load");
    module.functions.push(Function::internal_helper(
        "loop_copy",
        Signature::new(
            vec![
                ty.clone(),
                ty.clone(),
                Type::pointer(ty.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(7), ValueId(8)],
        vec![
            block(
                10,
                vec![],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(2), ty.clone()),
                        Kind::Constant(Constant::U64(1)),
                    ),
                    Operation::effect_free(
                        ValueDef::new(
                            ValueId(100),
                            Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                        ),
                        Kind::Alloca {
                            element: ty.clone(),
                            count: None,
                            address_space: AddressSpace::Private,
                            alignment: 8,
                        },
                    ),
                ],
                jump(20, &[0]),
            ),
            block(
                20,
                vec![ValueDef::new(ValueId(3), ty.clone())],
                vec![Operation::effect_free(
                    ValueDef::new(ValueId(4), Type::BOOL),
                    Kind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(3),
                        rhs: ValueId(1),
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(4),
                    then_target: BlockId(30),
                    then_arguments: vec![],
                    else_target: BlockId(50),
                    else_arguments: vec![],
                },
            ),
            block(
                30,
                vec![],
                vec![Operation::new(
                    vec![],
                    Kind::Store {
                        pointer: ValueId(100),
                        value: ValueId(3),
                        access: private,
                    },
                )],
                Terminator::ConditionalBranch {
                    condition: ValueId(8),
                    then_target: BlockId(40),
                    then_arguments: vec![],
                    else_target: BlockId(45),
                    else_arguments: vec![],
                },
            ),
            block(
                40,
                vec![],
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(101), ty.clone()),
                        Kind::Load {
                            pointer: ValueId(100),
                            access: private,
                        },
                    ),
                    Operation::new(
                        vec![],
                        Kind::Store {
                            pointer: ValueId(7),
                            value: ValueId(101),
                            access: MemoryAccess::new(AddressSpace::Global, 8),
                        },
                    ),
                ],
                jump(45, &[]),
            ),
            block(
                45,
                vec![],
                vec![Operation::checked_binary(
                    ValueDef::new(ValueId(5), ty),
                    ValueDef::new(ValueId(6), Type::BOOL),
                    CheckedBinaryOperator::Add,
                    ValueId(3),
                    ValueId(2),
                )],
                jump(20, &[5]),
            ),
            block(50, vec![], vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
fn operation(owner: &Owner, site: Site) -> &Operation {
    &owner.module().functions[site.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[site.block.block as usize]
        .operations[site.operation as usize]
}
fn require_two_rewrites(inputs: Inputs<'_>) {
    assert!(inputs.selected_allocations.is_empty());
    let splits = inputs
        .refinement_origins
        .iter()
        .filter(|row| matches!(row, Refinement::CheckedAddSplit { .. }))
        .count();
    let forwards = inputs
        .forwarding_origins
        .iter()
        .filter(|row| row.store.is_some())
        .count();
    assert_eq!((splits, forwards), (1, 1));
    assert_ne!(
        inputs.licm.canonical().canonical_bytes(),
        inputs.refined.canonical().canonical_bytes()
    );
    assert_ne!(
        inputs.refined.canonical().canonical_bytes(),
        inputs.output.canonical().canonical_bytes()
    );
    assert_ne!(
        inputs.licm.canonical().canonical_bytes(),
        inputs.output.canonical().canonical_bytes()
    );
    let Refinement::CheckedAddSplit {
        input,
        sum_output,
        false_output,
        ..
    } = *inputs
        .refinement_origins
        .iter()
        .find(|row| matches!(row, Refinement::CheckedAddSplit { .. }))
        .unwrap()
    else {
        unreachable!()
    };
    let old = operation(inputs.licm, input);
    let sum = operation(inputs.refined, sum_output);
    let overflow = operation(inputs.refined, false_output);
    let Kind::Binary {
        op: BinaryOp::Checked(CheckedBinaryOperator::Add),
        lhs,
        rhs,
    } = old.kind
    else {
        panic!("actual checked update")
    };
    assert_eq!(old.results.len(), 2);
    assert_eq!(sum.results, old.results[..1]);
    assert_eq!(overflow.results, old.results[1..]);
    assert_eq!(
        sum.kind,
        Kind::Binary {
            op: BinaryOp::Add,
            lhs,
            rhs
        }
    );
    assert_eq!(overflow.kind, Kind::Constant(Constant::Bool(false)));
    let row = inputs
        .forwarding_origins
        .iter()
        .find(|row| row.store.is_some())
        .unwrap();
    let load = operation(inputs.refined, row.input);
    let store_site = row.store.unwrap();
    let store = operation(inputs.refined, store_site);
    let copy = operation(inputs.output, row.output);
    assert_ne!(store_site.block, row.input.block);
    let Kind::Store {
        pointer,
        value,
        access,
    } = store.kind
    else {
        panic!("actual private store")
    };
    assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 8));
    assert_eq!(load.kind, Kind::Load { pointer, access });
    assert_eq!(copy.results, load.results);
    assert_eq!(
        copy.kind,
        Kind::Binary {
            op: BinaryOp::BitOr,
            lhs: value,
            rhs: value
        }
    );
    assert!(
        inputs.output.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|op| matches!(
                op.kind,
                Kind::Store {
                    pointer: ValueId(7),
                    value: ValueId(101),
                    ..
                }
            ))
    );
}

#[derive(Clone, Copy)]
enum Expected {
    Complete,
    RefinementRefusal,
    ForwardingRefusal,
}

fn transport(inputs: Inputs<'_>, floor: usize, expected: Expected) {
    let sibling = vec![0x6du8; 37];
    let inherited = floor + size_of_val(&sibling) + sibling.capacity();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(inherited).unwrap();
    budget.charge_work(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let wire = encode_refined_forwarding_history_v1(inputs, &mut budget).unwrap();
    let wire_storage = wire.storage().retained_storage();
    budget.reserve_storage(wire_storage).unwrap();
    let frame_storage;
    {
        let frame =
            read_refined_forwarding_history_v1(wire.canonical_bytes(), &mut budget).unwrap();
        frame_storage = frame.storage().retained_storage();
        budget.reserve_storage(frame_storage).unwrap();
        assert_eq!(frame.fields.len(), 27);
        assert_eq!(frame.limits(), inputs.limits);
        assert!(!frame.grants_authority());
        let owners = [
            inputs.prefix.prefix.prefix.prefix.input,
            inputs.prefix.prefix.prefix.prefix.intermediate,
            inputs.prefix.prefix.prefix.prefix.stored,
            inputs.prefix.prefix.prefix.prefix.output,
            inputs.prefix.prefix.prefix.output,
            inputs.prefix.prefix.output,
            inputs.prefix.output,
            inputs.promoted,
            inputs.preheaders,
            inputs.licm,
            inputs.refined,
            inputs.output,
        ];
        let roles = [
            RefinedForwardingHistoryRoleV1::B,
            RefinedForwardingHistoryRoleV1::C,
            RefinedForwardingHistoryRoleV1::S,
            RefinedForwardingHistoryRoleV1::O,
            RefinedForwardingHistoryRoleV1::I,
            RefinedForwardingHistoryRoleV1::J,
            RefinedForwardingHistoryRoleV1::K,
            RefinedForwardingHistoryRoleV1::P,
            RefinedForwardingHistoryRoleV1::H,
            RefinedForwardingHistoryRoleV1::L,
            RefinedForwardingHistoryRoleV1::R,
            RefinedForwardingHistoryRoleV1::F,
        ];
        let decoded = materialize_refined_forwarding_history_v1(&frame, &mut budget).unwrap();
        let decoded_storage = decoded.storage().retained_storage();
        budget.reserve_storage(decoded_storage).unwrap();
        for (index, (owner, role)) in owners.into_iter().zip(roles).enumerate() {
            assert_eq!(frame.fields[index], owner.canonical().canonical_bytes());
            assert_eq!(
                decoded.graph(role).canonical().canonical_bytes(),
                owner.canonical().canonical_bytes()
            );
            assert!(!std::ptr::eq(decoded.graph(role), owner));
            for prior in &roles[..index] {
                assert!(!std::ptr::eq(decoded.graph(role), decoded.graph(*prior)));
            }
        }
        assert!(!decoded.grants_authority());
        assert!(!decoded.authenticates_execution());
        let live_floor = budget.storage();
        match expected {
            Expected::Complete => {
                let receipt = decoded.check_semantics(&mut budget).unwrap();
                let receipt_storage = receipt.storage().retained_storage();
                budget.reserve_storage(receipt_storage).unwrap();
                assert_eq!(receipt.refinement().origins(), inputs.refinement_origins);
                assert_eq!(receipt.forwarding().origins(), inputs.forwarding_origins);
                assert_eq!(
                    receipt
                        .refinement()
                        .origins()
                        .iter()
                        .filter(|row| matches!(row, Refinement::CheckedAddSplit { .. }))
                        .count(),
                    1
                );
                assert_eq!(
                    receipt
                        .forwarding()
                        .origins()
                        .iter()
                        .filter(|row| row.store.is_some())
                        .count(),
                    1
                );
                assert!(std::ptr::eq(
                    receipt.refinement().input(),
                    decoded.graph(RefinedForwardingHistoryRoleV1::L)
                ));
                assert!(std::ptr::eq(
                    receipt.refinement().output(),
                    receipt.forwarding().input()
                ));
                assert!(std::ptr::eq(
                    receipt.refinement().output(),
                    decoded.graph(RefinedForwardingHistoryRoleV1::R)
                ));
                assert!(std::ptr::eq(
                    receipt.output(),
                    decoded.graph(RefinedForwardingHistoryRoleV1::F)
                ));
                assert!(!receipt.grants_authority());
                assert!(!receipt.authenticates_execution());
                for _ in 0..2 {
                    receipt.replay(inputs.limits, &mut budget).unwrap();
                }
                drop(receipt);
                budget.release_storage(receipt_storage).unwrap();
            }
            Expected::RefinementRefusal => assert!(matches!(
                decoded.check_semantics(&mut budget),
                Err(CanonicalRefinedForwardingHistoryErrorV1::Refinement(_))
            )),
            Expected::ForwardingRefusal => assert!(matches!(
                decoded.check_semantics(&mut budget),
                Err(CanonicalRefinedForwardingHistoryErrorV1::Forwarding(_))
            )),
        }
        assert_eq!(budget.storage(), live_floor);
        drop(decoded);
        budget.release_storage(decoded_storage).unwrap();
    }
    budget.release_storage(frame_storage).unwrap();
    drop(wire);
    budget.release_storage(wire_storage).unwrap();
    assert_eq!(budget.storage(), inherited);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(sibling, [0x6d; 37]);
}

#[test]
fn history_wire_nonzero_refinement_and_forwarding_replay_actual_twelve_graphs() {
    with_history_module(&mutating_module(), 0, |inputs, floor| {
        require_two_rewrites(inputs);
        transport(inputs, floor, Expected::Complete);
    });
}

#[test]
fn history_wire_nonzero_refinement_removed_or_changed_origins_refuse() {
    with_history_module(&mutating_module(), 0, |inputs, floor| {
        require_two_rewrites(inputs);
        for mutation in 0..3 {
            let mut origins = inputs.refinement_origins.to_vec();
            let at = origins
                .iter()
                .position(|row| matches!(row, Refinement::CheckedAddSplit { .. }))
                .unwrap();
            match mutation {
                0 => {
                    origins.remove(at);
                }
                1 => {
                    let Refinement::CheckedAddSplit {
                        sum_output,
                        false_output,
                        ..
                    } = &mut origins[at]
                    else {
                        unreachable!()
                    };
                    *false_output = *sum_output;
                }
                2 => {
                    let Refinement::CheckedAddSplit {
                        induction_row_ordinal,
                        ..
                    } = &mut origins[at]
                    else {
                        unreachable!()
                    };
                    *induction_row_ordinal += 1;
                }
                _ => unreachable!(),
            }
            let mut changed = inputs;
            changed.refinement_origins = &origins;
            transport(
                changed,
                floor + size_of_val(&origins) + origins.capacity() * size_of::<Refinement>(),
                Expected::RefinementRefusal,
            );
        }
    });
}

#[test]
fn history_wire_nonzero_forwarding_removed_or_changed_origins_refuse() {
    with_history_module(&mutating_module(), 0, |inputs, floor| {
        require_two_rewrites(inputs);
        for mutation in 0..3 {
            let mut origins = inputs.forwarding_origins.to_vec();
            let at = origins.iter().position(|row| row.store.is_some()).unwrap();
            match mutation {
                0 => {
                    origins.remove(at);
                }
                1 => {
                    origins[at].store = Some(origins[at].input);
                }
                2 => {
                    origins[at].store = None;
                }
                _ => unreachable!(),
            }
            let mut changed = inputs;
            changed.forwarding_origins = &origins;
            transport(
                changed,
                floor + size_of_val(&origins) + origins.capacity() * size_of::<Forwarding>(),
                Expected::ForwardingRefusal,
            );
        }
    });
}
