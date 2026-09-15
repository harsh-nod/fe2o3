use super::*;

pub(in crate::semantic_mir_v1) const BIND_ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

fn remap_place(place: &mut SemanticPlaceV1, map: &[SemanticLocalIdV1]) {
    assert!(
        place.projections.is_empty(),
        "closed fixture has no projections"
    );
    place.local = map[place.local.index() as usize];
}

fn remap_operand(operand: &mut SemanticOperandV1, map: &[SemanticLocalIdV1]) {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => remap_place(place, map),
        SemanticOperandV1::Constant(_) => {}
    }
}

/// Choose source identities that sort into the requested order, then apply
/// their exact index maps. Only test fixtures are reordered, never import data.
pub(in crate::semantic_mir_v1) fn canonical_permute(
    body: &mut SemanticFunctionDeclV1,
    local_order: &[usize],
    block_order: &[usize],
) {
    assert_eq!(local_order.len(), body.locals.len());
    assert_eq!(block_order.len(), body.blocks.len());
    for order in [local_order, block_order] {
        let mut sorted = order.to_vec();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..order.len()).collect::<Vec<_>>());
    }
    for (index, old) in local_order.iter().copied().enumerate() {
        body.locals[old].identity = SemanticLocalIdentityV1([index as u8 + 1; 32]);
    }
    let mut locals = body.locals.iter().cloned().enumerate().collect::<Vec<_>>();
    locals.sort_by_key(|(_, local)| local.identity());
    let mut local_map = vec![SemanticLocalIdV1(0); locals.len()];
    for (index, (old, _)) in locals.iter().enumerate() {
        local_map[*old] = SemanticLocalIdV1(index as u32);
    }
    body.locals = locals.into_iter().map(|(_, local)| local).collect();
    for (index, old) in block_order.iter().copied().enumerate() {
        body.blocks[old].identity = SemanticBlockIdentityV1([index as u8 + 1; 32]);
    }
    let mut blocks = body.blocks.iter().cloned().enumerate().collect::<Vec<_>>();
    blocks.sort_by_key(|(_, block)| block.identity());
    let mut block_map = vec![SemanticBlockIdV1(0); blocks.len()];
    for (index, (old, _)) in blocks.iter().enumerate() {
        block_map[*old] = SemanticBlockIdV1(index as u32);
    }
    body.blocks = blocks.into_iter().map(|(_, block)| block).collect();
    body.entry = block_map[body.entry.index() as usize];
    for block in &mut body.blocks {
        for statement in &mut block.statements {
            let SemanticStatementKindV1::Assign(assignment) = &mut statement.kind else {
                panic!("unexpected fixture statement")
            };
            remap_place(&mut assignment.destination, &local_map);
            let SemanticRvalueKindV1::Aggregate(aggregate) = &mut assignment.value.kind else {
                panic!("unexpected fixture rvalue")
            };
            for operand in &mut aggregate.operands {
                remap_operand(operand, &local_map);
            }
        }
        match &mut block.terminator.kind {
            SemanticTerminatorKindV1::Call(call) => {
                for argument in &mut call.arguments {
                    remap_operand(argument, &local_map);
                }
                let destination = call.destination.as_mut().unwrap();
                remap_place(&mut destination.place, &local_map);
                destination.edge.target = block_map[destination.edge.target.index() as usize];
                assert!(matches!(call.unwind, SemanticUnwindActionV1::Unreachable));
            }
            SemanticTerminatorKindV1::Return => {}
            _ => panic!("unexpected fixture terminator"),
        }
    }
    assert!(
        body.locals
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    assert!(
        body.blocks
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
}

fn permuted_fixture() -> Fixture {
    let mut f = fixture();
    canonical_permute(&mut f.functions[0], &[1, 0], &[1, 0]);
    canonical_permute(&mut f.functions[1], &[1, 0], &[1, 0]);
    canonical_permute(&mut f.functions[2], &[1, 2, 0], &[0]);
    f
}

fn entry_call(body: &mut SemanticFunctionDeclV1) -> &mut SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) =
        &mut body.blocks[body.entry.index() as usize].terminator.kind
    else {
        panic!("expected original call")
    };
    call
}

#[test]
fn defined_math_permutation_getter_bridge_roles_and_blocks() {
    for getter_locals in [[0, 1], [1, 0]] {
        for getter_blocks in [[0, 1], [1, 0]] {
            for bridge_locals in [[0, 1], [1, 0]] {
                for bridge_blocks in [[0, 1], [1, 0]] {
                    let mut f = fixture();
                    canonical_permute(&mut f.functions[0], &getter_locals, &getter_blocks);
                    canonical_permute(&mut f.functions[1], &bridge_locals, &bridge_blocks);
                    let before = f.functions.clone();
                    let record = f.derive().unwrap();
                    assert_eq!(record, f.derive().unwrap());
                    validate_math_derive_attachment(&f.functions[0], record).unwrap();
                    f.functions[0]
                        .clone()
                        .with_defined_capability_contract(
                            SemanticDefinedCapabilityContractV1::KernelMathDerive(record),
                        )
                        .unwrap();
                    assert_eq!(f.functions, before, "validation preserves canonical bodies");
                }
            }
        }
    }
}

#[test]
fn defined_math_permutation_bind_all_local_orders() {
    for order in BIND_ORDERS {
        let mut f = fixture();
        canonical_permute(&mut f.functions[2], &order, &[0]);
        let before = f.functions.clone();
        let record = f.bind().unwrap();
        validate_math_bind_attachment(&f.functions[2], record).unwrap();
        f.functions[2]
            .clone()
            .with_defined_capability_contract(SemanticDefinedCapabilityContractV1::PolicyMathBind(
                record,
            ))
            .unwrap();
        assert_eq!(record.reference_arguments(), [0, 1]);
        assert_eq!(record.reference_fields(), [0, 1]);
        assert_eq!(f.functions, before);
    }
}

#[test]
fn defined_math_permutation_rejects_invalid_control_flow() {
    for function in [0, 1] {
        for mutation in 0..8 {
            let mut f = permuted_fixture();
            let body = &mut f.functions[function];
            match mutation {
                0 => body.entry = SemanticBlockIdV1(2),
                1 => body.entry = SemanticBlockIdV1(0), // The Return block, not the call.
                2 => {
                    let entry = body.entry;
                    entry_call(body).destination.as_mut().unwrap().edge.target = entry;
                }
                3 => {
                    entry_call(body).destination.as_mut().unwrap().edge.target =
                        SemanticBlockIdV1(2)
                }
                4 => {
                    entry_call(body).destination.as_mut().unwrap().edge.role =
                        SemanticEdgeRoleV1::Goto
                }
                5 => body.blocks[0].terminator.kind = SemanticTerminatorKindV1::Unreachable,
                6 => {
                    let mut blocks = body.blocks.to_vec();
                    blocks.push(block(3, vec![], SemanticTerminatorKindV1::Return));
                    body.blocks = blocks.into_boxed_slice();
                }
                7 => entry_call(body).destination = None,
                _ => unreachable!(),
            }
            assert!(
                f.derive().is_err(),
                "function {function}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn defined_math_permutation_rejects_wrong_local_custody() {
    for function in [0, 1] {
        for mutation in 0..5 {
            let mut f = permuted_fixture();
            let body = &mut f.functions[function];
            match mutation {
                0 => {
                    let destination = &mut entry_call(body).destination.as_mut().unwrap().place;
                    destination.local = SemanticLocalIdV1(1 - destination.local.index());
                }
                1 => body.locals[0].role = SemanticLocalRoleV1::Return,
                2 => body.locals[0].ty = body.locals[1].ty,
                3 => {
                    let mut locals = body.locals.to_vec();
                    locals.push(locals[0].clone());
                    body.locals = locals.into_boxed_slice();
                }
                4 => body.locals[1].role = SemanticLocalRoleV1::Temporary,
                _ => unreachable!(),
            }
            assert!(
                f.derive().is_err(),
                "function {function}, mutation {mutation}"
            );
        }
    }
}

#[test]
fn defined_math_permutation_rejects_wrong_bind_reference_edges() {
    for mutation in 0..7 {
        let mut f = permuted_fixture();
        let body = &mut f.functions[2];
        let SemanticStatementKindV1::Assign(assignment) = &mut body.blocks[0].statements[0].kind
        else {
            panic!()
        };
        let SemanticRvalueKindV1::Aggregate(aggregate) = &mut assignment.value.kind else {
            panic!()
        };
        match mutation {
            0 => aggregate.operands.swap(0, 1),
            1 => {
                aggregate.operands[0] =
                    SemanticOperandV1::Copy(place(1, f.bind_types.math_reference))
            }
            2 => {
                aggregate.operands[0] =
                    SemanticOperandV1::Move(place(0, f.bind_types.math_reference))
            }
            3 => assignment.destination.local = SemanticLocalIdV1(0),
            4 => body.locals[1].role = SemanticLocalRoleV1::Argument(0),
            5 => body.entry = SemanticBlockIdV1(1),
            6 => body.locals[2].role = SemanticLocalRoleV1::Temporary,
            _ => unreachable!(),
        }
        assert!(f.bind().is_err(), "Bind mutation {mutation}");
    }
}

#[test]
fn defined_math_permutation_keeps_body_commitments_exact() {
    let original = fixture();
    let changed = permuted_fixture();
    let old_derive = original.derive().unwrap();
    let old_bind = original.bind().unwrap();
    let derive = changed.derive().unwrap();
    let bind = changed.bind().unwrap();
    assert_ne!(old_derive.body_identity(), derive.body_identity());
    assert_ne!(
        old_derive.bridge().body_identity(),
        derive.bridge().body_identity()
    );
    assert_ne!(old_bind.body_identity(), bind.body_identity());
    assert!(validate_math_derive_attachment(&changed.functions[0], old_derive).is_err());
    assert!(validate_math_bind_attachment(&changed.functions[2], old_bind).is_err());
    validate_math_derive_attachment(&changed.functions[0], derive).unwrap();
    validate_math_bind_attachment(&changed.functions[2], bind).unwrap();
}
