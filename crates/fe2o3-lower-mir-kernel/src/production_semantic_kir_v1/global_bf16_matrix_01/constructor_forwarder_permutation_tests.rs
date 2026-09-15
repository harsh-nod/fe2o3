// Structural predicate tests only. Type equalities and ownership are retained
// from the existing fixture; no provider or admitted source proof is constructed.
// Each order maps NEW local ID to OLD local ID (Return=0, Argument(n)=n+1).
const MEASURED: [usize; 7] = [1, 6, 5, 3, 4, 2, 0];

fn orders() -> [[usize; 7]; 9] {
    std::array::from_fn(|index| {
        let mut order = [0, 1, 2, 3, 4, 5, 6];
        match index {
            0..7 => order.rotate_left(index),
            7 => return MEASURED,
            8 => order.reverse(),
            _ => unreachable!(),
        }
        order
    })
}

fn rebuild(
    body: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    entry: SemanticBlockIdV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    assert!(body.export().is_none());
    assert!(body.defined_capability_contract().is_none());
    SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        locals,
        entry,
        blocks,
    )
    .unwrap()
}

fn permute(
    body: &SemanticFunctionDeclV1,
    order: [usize; 7],
    reverse_blocks: bool,
) -> SemanticFunctionDeclV1 {
    assert_eq!(body.locals().len(), 7);
    assert_eq!(body.blocks().len(), 2);
    let mut inverse = [None; 7];
    for (new, old) in order.into_iter().enumerate() {
        assert!(inverse[old].replace(new as u32).is_none(), "bijection only");
    }
    assert!(inverse.iter().all(Option::is_some));
    let place = |p: &SemanticPlaceV1| {
        assert!(p.projections().is_empty(), "exact forwarding fixture");
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(inverse[p.local().index() as usize].unwrap()),
            vec![],
            p.ty(),
        )
        .unwrap()
    };
    let block_id = |old: SemanticBlockIdV1| {
        assert!(old.index() < 2);
        SemanticBlockIdV1::from_index(if reverse_blocks {
            1 - old.index()
        } else {
            old.index()
        })
    };
    let locals = order
        .into_iter()
        .enumerate()
        .map(|(new, old)| {
            let local = &body.locals()[old];
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([20 + new as u8; 32]),
                local.ty(),
                local.role(),
                local.source(),
            )
        })
        .collect();
    let blocks = (0..2)
        .map(|new| {
            let old = if reverse_blocks { 1 - new } else { new };
            let block = &body.blocks()[old];
            assert!(block.statements().is_empty());
            let terminator = match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    assert!(matches!(call.unwind(), SemanticUnwindActionV1::Unreachable));
                    let arguments = call
                        .arguments()
                        .iter()
                        .map(|operand| match operand {
                            SemanticOperandV1::Copy(p) => SemanticOperandV1::Copy(place(p)),
                            SemanticOperandV1::Move(p) => SemanticOperandV1::Move(place(p)),
                            SemanticOperandV1::Constant(c) => {
                                SemanticOperandV1::Constant(c.clone())
                            }
                        })
                        .collect();
                    let destination = call.destination().unwrap();
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                            call.callee(),
                            arguments,
                            call.variadic_argument_abis().to_vec(),
                            Some(SemanticCallDestinationV1::new(
                                place(destination.place()),
                                SemanticControlFlowEdgeV1::new(
                                    destination.edge().role(),
                                    block_id(destination.edge().target()),
                                ),
                            )),
                            call.unwind(),
                        )
                        .unwrap(),
                    )
                }
                SemanticTerminatorKindV1::Return => SemanticTerminatorKindV1::Return,
                _ => panic!("exact two-block forwarding fixture"),
            };
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([10 + new as u8; 32]),
                block.source(),
                vec![],
                SemanticTerminatorV1::new(block.terminator().source(), terminator),
            )
            .unwrap()
        })
        .collect();
    rebuild(body, locals, block_id(body.entry()), blocks)
}

fn call(body: &SemanticFunctionDeclV1) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = body.blocks()[body.entry().index() as usize]
        .terminator()
        .kind()
    else {
        panic!("fixture call at canonical entry")
    };
    call
}

fn with_call(
    body: &SemanticFunctionDeclV1,
    arguments: Vec<SemanticOperandV1>,
    destination: SemanticPlaceV1,
) -> SemanticFunctionDeclV1 {
    let old = call(body);
    let changed = SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
        old.callee(),
        arguments,
        old.variadic_argument_abis().to_vec(),
        Some(SemanticCallDestinationV1::new(
            destination,
            old.destination().unwrap().edge(),
        )),
        old.unwind(),
    )
    .unwrap();
    let mut blocks = body.blocks().to_vec();
    let index = body.entry().index() as usize;
    let block = &blocks[index];
    blocks[index] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        block.statements().to_vec(),
        SemanticTerminatorV1::new(
            block.terminator().source(),
            SemanticTerminatorKindV1::Call(changed),
        ),
    )
    .unwrap();
    rebuild(body, body.locals().to_vec(), body.entry(), blocks)
}

fn with_role(
    body: &SemanticFunctionDeclV1,
    local: usize,
    role: SemanticLocalRoleV1,
) -> SemanticFunctionDeclV1 {
    let mut locals = body.locals().to_vec();
    let old = &locals[local];
    locals[local] = SemanticLocalDeclV1::new(old.identity(), old.ty(), role, old.source());
    rebuild(body, locals, body.entry(), body.blocks().to_vec())
}

#[track_caller]
fn require(body: &SemanticFunctionDeclV1, expected: bool) {
    let checked = SemanticFunctionIdV1::from_index(7);
    let callables = [SemanticCallableDeclV1::defined(checked)];
    let expected = if expected {
        let receiver_roles = body
            .locals()
            .iter()
            .enumerate()
            .filter(|(_, local)| local.role() == SemanticLocalRoleV1::Argument(0))
            .map(|(index, _)| SemanticLocalIdV1::from_index(index as u32))
            .collect::<Vec<_>>();
        let [receiver] = receiver_roles.as_slice() else {
            panic!("one fixture receiver role")
        };
        Some(*receiver)
    } else {
        None
    };
    assert_eq!(exact_forwarder(body, &callables, checked), expected);
    assert_eq!(
        exact_forwarder_at(body, &callables, checked, Some(body.entry())),
        expected
    );
}

#[test]
fn measured_canonical_local_permutation_preserves_exact_forwarding() {
    let body = permute(&fixture(Mutation::None), MEASURED, true);
    assert_eq!(body.entry().index(), 1);
    let roles: Vec<_> = body.locals().iter().map(|local| local.role()).collect();
    use SemanticLocalRoleV1::{Argument, Return};
    assert_eq!(
        roles,
        [
            Argument(0),
            Argument(5),
            Argument(4),
            Argument(2),
            Argument(3),
            Argument(1),
            Return
        ]
    );
    let call = call(&body);
    let local_ids: Vec<_> = call
        .arguments()
        .iter()
        .map(|operand| match operand {
            SemanticOperandV1::Copy(place) => place.local().index(),
            _ => panic!("all five observed operands are Copy"),
        })
        .collect();
    assert_eq!(local_ids, [5, 3, 4, 2, 1]);
    assert_eq!(call.destination().unwrap().place().local().index(), 6);
    assert_eq!(call.destination().unwrap().edge().target().index(), 0);
    assert_eq!(
        call.destination().unwrap().edge().role(),
        SemanticEdgeRoleV1::CallReturn
    );
    require(&body, true);
    let checked = SemanticFunctionIdV1::from_index(7);
    assert_eq!(
        exact_forwarder_at(
            &body,
            &[SemanticCallableDeclV1::defined(checked)],
            checked,
            Some(body.entry())
        ),
        Some(SemanticLocalIdV1::from_index(0)),
        "observed receiver is local0, not local1"
    );
}

#[test]
fn every_source_role_can_occupy_every_local_slot_in_both_block_orders() {
    let original = fixture(Mutation::None);
    let mut covered = [[false; 7]; 7];
    for order in orders() {
        for (new, old) in order.into_iter().enumerate() {
            covered[old][new] = true;
        }
        for reverse_blocks in [false, true] {
            let body = permute(&original, order, reverse_blocks);
            assert_eq!(body.abi(), original.abi());
            assert!(
                body.locals()
                    .windows(2)
                    .all(|pair| pair[0].identity() < pair[1].identity())
            );
            assert!(
                body.blocks()
                    .windows(2)
                    .all(|pair| pair[0].identity() < pair[1].identity())
            );
            require(&body, true);
        }
    }
    assert!(covered.into_iter().flatten().all(|seen| seen));
}

#[test]
fn all_same_typed_geometry_swaps_reject_after_canonical_permutation() {
    for order in orders() {
        for reverse_blocks in [false, true] {
            let body = permute(&fixture(Mutation::None), order, reverse_blocks);
            require(&body, true);
            let original = call(&body);
            for left in 1..5 {
                for right in left + 1..5 {
                    let mut arguments = original.arguments().to_vec();
                    assert_eq!(arguments[left].ty(), arguments[right].ty());
                    arguments.swap(left, right);
                    let changed = with_call(
                        &body,
                        arguments,
                        original.destination().unwrap().place().clone(),
                    );
                    require(&changed, false);
                }
            }
        }
    }
}

#[test]
fn duplicate_missing_and_out_of_range_roles_reject_in_every_selected_order() {
    use SemanticLocalRoleV1::{Argument, Return, Temporary};
    let original = fixture(Mutation::None);
    assert_eq!(original.locals()[3].ty(), original.locals()[4].ty());
    let mutations = [(4, Argument(2)), (4, Argument(6)), (4, Return)]
        .into_iter()
        .chain((0..7).map(|local| (local, Temporary)));
    for (local, role) in mutations {
        let changed = with_role(&original, local, role);
        assert_eq!(changed.locals().len(), 7);
        assert_eq!(call(&changed), call(&original));
        assert_eq!(changed.abi(), original.abi());
        for order in orders() {
            for reverse_blocks in [false, true] {
                require(&permute(&original, order, reverse_blocks), true);
                require(&permute(&changed, order, reverse_blocks), false);
            }
        }
    }
}

#[test]
fn wrong_return_receiver_forward_and_each_move_reject_after_permutation() {
    for order in orders() {
        for reverse_blocks in [false, true] {
            let body = permute(&fixture(Mutation::None), order, reverse_blocks);
            require(&body, true);
            let original = call(&body);
            let receiver = SemanticLocalIdV1::from_index(
                order.iter().position(|old| *old == 1).unwrap() as u32,
            );
            let destination = original.destination().unwrap().place();
            assert_ne!(destination.local(), receiver);
            let wrong_return = SemanticPlaceV1::new(receiver, vec![], destination.ty()).unwrap();
            require(
                &with_call(&body, original.arguments().to_vec(), wrong_return),
                false,
            );
            let mut arguments = original.arguments().to_vec();
            arguments[0] = SemanticOperandV1::Copy(
                SemanticPlaceV1::new(receiver, vec![], body.abi().source_input_types()[0]).unwrap(),
            );
            require(&with_call(&body, arguments, destination.clone()), false);
            for index in 0..5 {
                let mut arguments = original.arguments().to_vec();
                let SemanticOperandV1::Copy(place) = &arguments[index] else {
                    panic!("positive Copy")
                };
                arguments[index] = SemanticOperandV1::Move(place.clone());
                require(&with_call(&body, arguments, destination.clone()), false);
            }
        }
    }
}

#[test]
fn role_mapping_does_not_replace_exact_callee_entry_or_return_edge() {
    let checked = SemanticFunctionIdV1::from_index(7);
    let callables = [SemanticCallableDeclV1::defined(checked)];
    for order in orders() {
        for reverse_blocks in [false, true] {
            let body = permute(&fixture(Mutation::None), order, reverse_blocks);
            require(&body, true);
            let other = SemanticBlockIdV1::from_index(1 - body.entry().index());
            assert!(exact_forwarder_at(&body, &callables, checked, Some(other)).is_none());
            assert!(exact_forwarder_at(&body, &callables, checked, None).is_none());
            assert!(
                exact_forwarder(
                    &body,
                    &[SemanticCallableDeclV1::defined(
                        SemanticFunctionIdV1::from_index(8)
                    )],
                    checked
                )
                .is_none()
            );
            require(
                &with_target(&body, body.entry(), SemanticEdgeRoleV1::CallReturn),
                false,
            );
            require(&with_target(&body, other, SemanticEdgeRoleV1::Goto), false);
        }
    }
}
