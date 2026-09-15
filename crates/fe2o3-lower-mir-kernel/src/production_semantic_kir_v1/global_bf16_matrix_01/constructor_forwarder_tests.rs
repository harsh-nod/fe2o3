use super::*;

mod permutation_tests {
    use super::*;
    include!("constructor_forwarder_permutation_tests.rs");
}

mod observation_tests {
    use super::super::super::constructor_forwarder_observation as observation;
    use super::*;
    include!("constructor_forwarder_observation_tests.rs");
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    ForwardReceiver,
    ReorderGeometry,
    MoveGlobal,
    MutableReceiver,
    MutableGlobal,
    ReturnOtherLocal,
    InsertStatement,
    ChangedReturn,
    ExtraBlock,
    WrongLocalRole,
}

// This tests the structural predicate with the real semantic model, not an
// admission or provider record. The physical ABI is not a premise of this leaf.
fn fixture(mutation: Mutation) -> SemanticFunctionDeclV1 {
    let ty = SemanticTypeIdV1::from_index;
    let source = SemanticSourceProvenanceV1::unavailable();
    let inputs = vec![ty(0), ty(1), ty(2), ty(2), ty(2), ty(2)];
    let output = ty(3);
    let mut ownership = vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
        SemanticSourceArgumentOwnershipV1::ByValue,
    ];
    if matches!(
        mutation,
        Mutation::MutableReceiver | Mutation::MutableGlobal
    ) {
        ownership[usize::from(matches!(mutation, Mutation::MutableGlobal))] =
            SemanticSourceArgumentOwnershipV1::UniqueBorrow;
    }
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        6,
        inputs.clone(),
        output,
        inputs
            .iter()
            .map(|&t| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    t,
                    SemanticAbiPassModeV1::Ignore,
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let mut arguments: Vec<_> = (2..7)
        .map(|local| SemanticOperandV1::Copy(place(local, inputs[local as usize - 1])))
        .collect();
    match mutation {
        Mutation::ForwardReceiver => arguments[0] = SemanticOperandV1::Copy(place(1, inputs[0])),
        Mutation::ReorderGeometry => arguments.swap(1, 2),
        Mutation::MoveGlobal => arguments[0] = SemanticOperandV1::Move(place(2, inputs[1])),
        _ => {}
    }
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        arguments,
        Some(SemanticCallDestinationV1::new(
            place(
                if matches!(mutation, Mutation::ReturnOtherLocal) {
                    1
                } else {
                    0
                },
                output,
            ),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(1),
            ),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let statements = if matches!(mutation, Mutation::InsertStatement) {
        vec![SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Nop,
        )]
    } else {
        vec![]
    };
    let mut blocks = vec![
        block(10, statements, SemanticTerminatorKindV1::Call(call)),
        block(
            11,
            vec![],
            if matches!(mutation, Mutation::ChangedReturn) {
                SemanticTerminatorKindV1::Unreachable
            } else {
                SemanticTerminatorKindV1::Return
            },
        ),
    ];
    if matches!(mutation, Mutation::ExtraBlock) {
        blocks.push(block(12, vec![], SemanticTerminatorKindV1::Return));
    }
    let locals = std::iter::once(output)
        .chain(inputs)
        .enumerate()
        .map(|(index, ty)| {
            let role = if index == 0 {
                SemanticLocalRoleV1::Return
            } else if index == 1 && matches!(mutation, Mutation::WrongLocalRole) {
                SemanticLocalRoleV1::Temporary
            } else {
                SemanticLocalRoleV1::Argument(index as u32 - 1)
            };
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([20 + index as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([31; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([32; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([33; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([34; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn accepts(mutation: Mutation) -> Option<SemanticLocalIdV1> {
    let checked = SemanticFunctionIdV1::from_index(7);
    exact_forwarder(
        &fixture(mutation),
        &[SemanticCallableDeclV1::defined(checked)],
        checked,
    )
}

#[test]
fn exact_unused_receiver_forwarder_preserves_all_five_checked_inputs() {
    assert_eq!(
        accepts(Mutation::None),
        Some(SemanticLocalIdV1::from_index(1))
    );
}

#[test]
fn receiver_substitution_and_same_type_geometry_reordering_reject() {
    assert!(accepts(Mutation::ForwardReceiver).is_none());
    assert!(accepts(Mutation::ReorderGeometry).is_none());
}

#[test]
fn mutable_or_moved_input_cannot_be_shared_forwarding() {
    assert!(accepts(Mutation::MoveGlobal).is_none());
    assert!(accepts(Mutation::MutableReceiver).is_none());
    assert!(accepts(Mutation::MutableGlobal).is_none());
}

#[test]
fn checked_callee_identity_must_be_exact_not_only_same_signature() {
    let body = fixture(Mutation::None);
    assert!(
        exact_forwarder(
            &body,
            &[SemanticCallableDeclV1::defined(
                SemanticFunctionIdV1::from_index(8)
            )],
            SemanticFunctionIdV1::from_index(7)
        )
        .is_none()
    );
    assert!(exact_forwarder(&body, &[], SemanticFunctionIdV1::from_index(7)).is_none());
}

#[test]
fn return_control_flow_and_statement_roster_must_remain_exact() {
    assert!(accepts(Mutation::ReturnOtherLocal).is_none());
    assert!(accepts(Mutation::InsertStatement).is_none());
    assert!(accepts(Mutation::ChangedReturn).is_none());
    assert!(accepts(Mutation::ExtraBlock).is_none());
}

#[test]
fn positional_type_equality_does_not_replace_argument_role() {
    assert!(accepts(Mutation::WrongLocalRole).is_none());
}

fn with_blocks(
    body: &SemanticFunctionDeclV1,
    entry: SemanticBlockIdV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        body.identity(),
        body.role(),
        body.item_definition_identity(),
        body.monomorphization_identity(),
        body.generic_type_arguments_identity(),
        body.const_generic_arguments_identity(),
        body.source(),
        body.abi().clone(),
        body.locals().to_vec(),
        entry,
        blocks,
    )
    .unwrap()
}

fn with_target(
    body: &SemanticFunctionDeclV1,
    target: SemanticBlockIdV1,
    role: SemanticEdgeRoleV1,
) -> SemanticFunctionDeclV1 {
    let entry = body.entry().index() as usize;
    let mut blocks = body.blocks().to_vec();
    let block = &blocks[entry];
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        panic!("fixture entry call")
    };
    let call = SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
        call.callee(),
        call.arguments().to_vec(),
        call.variadic_argument_abis().to_vec(),
        Some(SemanticCallDestinationV1::new(
            call.destination().unwrap().place().clone(),
            SemanticControlFlowEdgeV1::new(role, target),
        )),
        call.unwind(),
    )
    .unwrap();
    blocks[entry] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        block.statements().to_vec(),
        SemanticTerminatorV1::new(
            block.terminator().source(),
            SemanticTerminatorKindV1::Call(call),
        ),
    )
    .unwrap();
    with_blocks(body, body.entry(), blocks)
}

fn reordered(mutation: Mutation) -> SemanticFunctionDeclV1 {
    let original = fixture(mutation);
    let remapped = with_target(
        &original,
        SemanticBlockIdV1::from_index(0),
        SemanticEdgeRoleV1::CallReturn,
    );
    let mut blocks = remapped.blocks().to_vec();
    blocks.swap(0, 1);
    with_blocks(&remapped, SemanticBlockIdV1::from_index(1), blocks)
}

#[test]
fn global_bf16_forwarder_follows_canonical_entry_and_return_edge_in_both_orders() {
    let checked = SemanticFunctionIdV1::from_index(7);
    let callables = [SemanticCallableDeclV1::defined(checked)];
    for body in [fixture(Mutation::None), reordered(Mutation::None)] {
        let receiver = Some(SemanticLocalIdV1::from_index(1));
        assert_eq!(exact_forwarder(&body, &callables, checked), receiver);
        assert_eq!(
            exact_forwarder_at(&body, &callables, checked, Some(body.entry())),
            receiver
        );
        assert!(exact_forwarder_at(&body, &callables, checked, None).is_none());
        assert!(
            exact_forwarder_at(
                &body,
                &callables,
                checked,
                Some(SemanticBlockIdV1::from_index(1 - body.entry().index()))
            )
            .is_none()
        );
    }
}

#[test]
fn global_bf16_forwarder_nonzero_entry_preserves_every_existing_shape_negative() {
    let checked = SemanticFunctionIdV1::from_index(7);
    let callables = [SemanticCallableDeclV1::defined(checked)];
    for (case, mutation) in [
        Mutation::ForwardReceiver,
        Mutation::ReorderGeometry,
        Mutation::MoveGlobal,
        Mutation::MutableReceiver,
        Mutation::MutableGlobal,
        Mutation::ReturnOtherLocal,
        Mutation::InsertStatement,
        Mutation::ChangedReturn,
        Mutation::ExtraBlock,
        Mutation::WrongLocalRole,
    ]
    .into_iter()
    .enumerate()
    {
        let original = fixture(mutation);
        let permuted = reordered(mutation);
        assert!(
            exact_forwarder_at(&original, &callables, checked, Some(original.entry())).is_none(),
            "original case {case}"
        );
        assert!(
            exact_forwarder_at(&permuted, &callables, checked, Some(permuted.entry())).is_none(),
            "permuted case {case}"
        );
    }
    let permuted = reordered(Mutation::None);
    assert!(
        exact_forwarder_at(
            &permuted,
            &[SemanticCallableDeclV1::defined(
                SemanticFunctionIdV1::from_index(8)
            )],
            checked,
            Some(permuted.entry())
        )
        .is_none()
    );
    assert!(exact_forwarder_at(&permuted, &[], checked, Some(permuted.entry())).is_none());
}

#[test]
fn global_bf16_forwarder_rejects_bypassed_entry_self_return_and_foreign_edges() {
    let checked = SemanticFunctionIdV1::from_index(7);
    let callables = [SemanticCallableDeclV1::defined(checked)];
    for body in [fixture(Mutation::None), reordered(Mutation::None)] {
        let other = SemanticBlockIdV1::from_index(1 - body.entry().index());
        let bypass = with_blocks(&body, other, body.blocks().to_vec());
        assert!(exact_forwarder_at(&bypass, &callables, checked, Some(other)).is_none());
        let invalid_entry = with_blocks(
            &body,
            SemanticBlockIdV1::from_index(2),
            body.blocks().to_vec(),
        );
        assert!(
            exact_forwarder_at(
                &invalid_entry,
                &callables,
                checked,
                Some(invalid_entry.entry())
            )
            .is_none()
        );
        for (target, role) in [
            (body.entry(), SemanticEdgeRoleV1::CallReturn),
            (
                SemanticBlockIdV1::from_index(2),
                SemanticEdgeRoleV1::CallReturn,
            ),
            (other, SemanticEdgeRoleV1::Goto),
        ] {
            let changed = with_target(&body, target, role);
            assert!(
                exact_forwarder_at(&changed, &callables, checked, Some(changed.entry())).is_none()
            );
        }
    }
}
