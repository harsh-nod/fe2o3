use super::*;
use crate::semantic_mir_v1::*;
use crate::semantic_option_dominance::{
    MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1, semantic_unchecked_arithmetic_violation_v1,
};

const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

type Block = (Vec<SemanticStatementV1>, SemanticTerminatorKindV1);

fn identity(index: usize) -> [u8; 32] {
    let mut identity = [0; 32];
    identity[..8].copy_from_slice(&(index as u64).to_le_bytes());
    identity
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}

fn moved(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(local, ty))
}

fn constant(bits: u128, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    let size = if ty == BOOL {
        1
    } else if ty == U64 {
        8
    } else {
        4
    };
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, size).unwrap()),
    ))
}

fn overflow(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), BOOL).unwrap()],
            BOOL,
        )
        .unwrap(),
    )
}

fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}

fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local, ty),
        SemanticRvalueV1::new(ty, value),
    )))
}

fn alias(local: u32, source: SemanticOperandV1) -> SemanticStatementV1 {
    assign(local, source.ty(), SemanticRvalueKindV1::Use(source))
}

fn checked(
    local: u32,
    left: SemanticOperandV1,
    right: SemanticOperandV1,
    operation: SemanticCheckedBinaryOpV1,
) -> SemanticStatementV1 {
    assign(
        local,
        PAIR,
        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
            operation, left, right,
        )),
    )
}

fn unchecked(
    left: SemanticOperandV1,
    right: SemanticOperandV1,
    operation: SemanticUncheckedBinaryOpV1,
) -> SemanticStatementV1 {
    assign(
        if left.ty() == U64 { 10 } else { 0 },
        left.ty(),
        SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
            operation, left, right,
        )),
    )
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}

fn switch(discriminant: SemanticOperandV1, one: u32, zero: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                1,
                edge(SemanticEdgeRoleV1::SwitchValue, one),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, zero),
        )
        .unwrap(),
    }
}

fn fixture() -> Vec<Block> {
    vec![
        (
            vec![
                checked(
                    3,
                    copy(1, U32),
                    copy(2, U32),
                    SemanticCheckedBinaryOpV1::Add,
                ),
                alias(4, overflow(3)),
            ],
            switch(moved(4, BOOL), 1, 2),
        ),
        (vec![], SemanticTerminatorKindV1::Return),
        (
            vec![unchecked(
                copy(1, U32),
                copy(2, U32),
                SemanticUncheckedBinaryOpV1::Add,
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]
}

fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn function(blocks: Vec<Block>) -> SemanticFunctionDeclV1 {
    let locals = [
        U32, U32, U32, PAIR, BOOL, U32, U32, PAIR, BOOL, POINTER, U64, U64, BOOL, U32,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(identity(index)),
            ty,
            match index {
                0 => SemanticLocalRoleV1::Return,
                1 => SemanticLocalRoleV1::Argument(0),
                2 => SemanticLocalRoleV1::Argument(1),
                11 => SemanticLocalRoleV1::Argument(2),
                12 => SemanticLocalRoleV1::Argument(3),
                _ => SemanticLocalRoleV1::Temporary,
            },
            SemanticSourceProvenanceV1::unavailable(),
        )
    })
    .collect();
    let blocks = blocks
        .into_iter()
        .enumerate()
        .map(|(index, (statements, terminator))| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(identity(index)),
                SemanticSourceProvenanceV1::unavailable(),
                statements,
                SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
            )
            .unwrap()
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(identity(30)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(identity(31)),
        SemanticMonomorphizationIdentityV1::from_sha256(identity(32)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(33)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(34)),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(identity(35)),
            SemanticLayoutIdentityV1::from_sha256(identity(36)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct(U32), direct(U32), direct(U64), direct(BOOL)],
            direct(U32),
        )
        .unwrap(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn accepts(blocks: Vec<Block>, expected: bool) {
    let function = function(blocks);
    let before = function.clone();
    let violation = semantic_unchecked_arithmetic_violation_v1(&function).unwrap();
    assert_eq!(violation.is_none(), expected, "{violation:?}");
    assert_eq!(function, before, "the proof must not rewrite retained MIR");
}

#[test]
fn copy_move_operand_and_tuple_flag_aliases_retain_the_exact_producer() {
    for (checked_op, unchecked_op) in [
        (
            SemanticCheckedBinaryOpV1::Add,
            SemanticUncheckedBinaryOpV1::Add,
        ),
        (
            SemanticCheckedBinaryOpV1::Subtract,
            SemanticUncheckedBinaryOpV1::Subtract,
        ),
        (
            SemanticCheckedBinaryOpV1::Multiply,
            SemanticUncheckedBinaryOpV1::Multiply,
        ),
    ] {
        let mut blocks = fixture();
        blocks[0].0 = vec![
            alias(5, copy(1, U32)),
            alias(6, copy(2, U32)),
            checked(3, copy(5, U32), copy(6, U32), checked_op),
            alias(7, moved(3, PAIR)),
            alias(8, overflow(7)),
            alias(4, moved(8, BOOL)),
        ];
        blocks[2].0 = vec![unchecked(moved(5, U32), moved(6, U32), unchecked_op)];
        accepts(blocks.clone(), true);
        blocks[2].0 = vec![unchecked(copy(1, U32), copy(2, U32), unchecked_op)];
        accepts(blocks, true);
    }
}

#[test]
fn saved_copies_keep_their_value_but_overwritten_operands_do_not() {
    let mut blocks = fixture();
    blocks[0].0.insert(0, alias(5, copy(1, U32)));
    blocks[2].0 = vec![
        alias(1, constant(9, U32)),
        unchecked(copy(5, U32), copy(2, U32), SemanticUncheckedBinaryOpV1::Add),
    ];
    accepts(blocks.clone(), true);
    blocks[2].0[1] = unchecked(copy(1, U32), copy(2, U32), SemanticUncheckedBinaryOpV1::Add);
    accepts(blocks, false);

    let mut blocks = fixture();
    blocks[0]
        .0
        .splice(0..0, [alias(5, copy(1, U32)), alias(1, constant(9, U32))]);
    blocks[2].0 = vec![unchecked(
        copy(5, U32),
        copy(2, U32),
        SemanticUncheckedBinaryOpV1::Add,
    )];
    accepts(blocks, false);
}

#[test]
fn opaque_definitions_are_distinct_versions_even_at_the_same_local() {
    let value = || SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::BitXor,
        left: copy(1, U32),
        right: copy(2, U32),
    };
    let mut blocks = fixture();
    blocks[0].0.insert(0, assign(1, U32, value()));
    accepts(blocks.clone(), true);
    blocks[2].0.insert(0, assign(1, U32, value()));
    accepts(blocks, false);
}

#[test]
fn overwritten_flags_and_flags_from_an_older_operand_version_are_rejected() {
    let mut blocks = fixture();
    blocks[0].0.push(alias(4, constant(0, BOOL)));
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[0].0.extend([
        alias(1, constant(9, U32)),
        checked(
            3,
            copy(1, U32),
            copy(2, U32),
            SemanticCheckedBinaryOpV1::Add,
        ),
    ]);
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[0].0[1] = alias(4, copy(8, BOOL));
    blocks[0].0.push(alias(8, overflow(3)));
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[0].0.push(alias(8, moved(4, BOOL)));
    accepts(blocks, false);
    let mut blocks = fixture();
    let SemanticOperandV1::Copy(flag) = overflow(3) else {
        unreachable!()
    };
    blocks[0].0.insert(
        1,
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            flag,
            SemanticRvalueV1::new(BOOL, SemanticRvalueKindV1::Use(constant(0, BOOL))),
        ))),
    );
    accepts(blocks, false);
}

fn diamond() -> Vec<Block> {
    let mut blocks = fixture();
    let consumer = std::mem::take(&mut blocks[2].0);
    blocks[2].1 = switch(copy(12, BOOL), 3, 4);
    blocks.extend([
        (
            vec![alias(5, copy(1, U32)), alias(6, copy(2, U32))],
            goto(5),
        ),
        (
            vec![alias(5, moved(1, U32)), alias(6, moved(2, U32))],
            goto(5),
        ),
        (consumer, SemanticTerminatorKindV1::Return),
    ]);
    blocks[5].0[0] = unchecked(copy(5, U32), copy(6, U32), SemanticUncheckedBinaryOpV1::Add);
    blocks
}

#[test]
fn joins_require_all_reaching_operand_values_to_agree() {
    accepts(diamond(), true);
    let mut blocks = diamond();
    blocks[4].0[0] = alias(5, copy(2, U32));
    accepts(blocks, false);
    let mut blocks = diamond();
    blocks[4].0.remove(0);
    accepts(blocks, false);
    let mut blocks = diamond();
    blocks[4].0.insert(0, alias(1, constant(9, U32)));
    accepts(blocks, false);
}

#[test]
fn flag_joins_must_retain_one_exact_checked_definition() {
    let mut blocks = fixture();
    blocks[0].1 = switch(copy(12, BOOL), 3, 4);
    blocks.extend([
        (vec![alias(8, copy(4, BOOL))], goto(5)),
        (vec![alias(8, moved(4, BOOL))], goto(5)),
        (vec![], switch(copy(8, BOOL), 1, 2)),
    ]);
    accepts(blocks.clone(), true);
    blocks[4].0 = vec![
        checked(
            7,
            copy(1, U32),
            copy(2, U32),
            SemanticCheckedBinaryOpV1::Add,
        ),
        alias(8, overflow(7)),
    ];
    accepts(blocks, false);
}

#[test]
fn aliases_do_not_waive_overflow_bypass_or_rejoined_edges() {
    let mut blocks = fixture();
    blocks[0].1 = SemanticTerminatorKindV1::SwitchInt {
        discriminant: moved(4, BOOL),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, 2),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
        )
        .unwrap(),
    };
    accepts(blocks, true);
    let mut blocks = fixture();
    blocks[1].0 = std::mem::take(&mut blocks[2].0);
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[1].1 = goto(2);
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[0].1 = switch(copy(4, BOOL), 2, 2);
    accepts(blocks, false);
    let mut blocks = fixture();
    let producer = std::mem::replace(&mut blocks[0], (vec![], switch(copy(12, BOOL), 3, 4)));
    blocks.extend([(vec![], goto(2)), producer]);
    accepts(blocks, false);
}

#[test]
fn width_operation_and_operand_order_remain_exact() {
    let mut blocks = fixture();
    blocks[0].0[0] = checked(
        3,
        constant(7, U32),
        constant(11, U32),
        SemanticCheckedBinaryOpV1::Add,
    );
    blocks[2].0[0] = unchecked(
        constant(7, U64),
        constant(11, U64),
        SemanticUncheckedBinaryOpV1::Add,
    );
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[2].0[0] = unchecked(
        copy(1, U32),
        copy(2, U32),
        SemanticUncheckedBinaryOpV1::Subtract,
    );
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[2].0[0] = unchecked(copy(2, U32), copy(1, U32), SemanticUncheckedBinaryOpV1::Add);
    accepts(blocks, false);
    let mut blocks = fixture();
    blocks[2].0.insert(
        0,
        assign(
            10,
            U64,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: copy(1, U32),
            },
        ),
    );
    blocks[2].0[1] = unchecked(
        copy(10, U64),
        copy(11, U64),
        SemanticUncheckedBinaryOpV1::Add,
    );
    accepts(blocks, false);
}

#[test]
fn borrow_escape_lifetime_and_move_kills_do_not_leave_alias_proofs() {
    for kill in [
        assign(
            9,
            POINTER,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1, U32),
            },
        ),
        assign(
            9,
            POINTER,
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(1, U32),
            },
        ),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::Deinitialize(place(1, U32))),
        alias(5, moved(1, U32)),
    ] {
        let mut blocks = fixture();
        blocks[2].0.insert(0, kill);
        accepts(blocks, false);
    }
}

#[test]
fn projected_writes_and_unknown_memory_writes_are_barriers() {
    let pointer_target = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(9),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let access = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Agent,
    );
    for kill in [
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            pointer_target.clone(),
            SemanticRvalueV1::new(U32, SemanticRvalueKindV1::Use(constant(0, U32))),
        ))),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            pointer_target.clone(),
            constant(0, U32),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
        statement(SemanticStatementKindV1::AtomicRmw(
            SemanticAtomicRmwV1::new(
                place(13, U32),
                pointer_target.clone(),
                constant(0, U32),
                SemanticAtomicRmwOpV1::Exchange,
                access,
            ),
        )),
        statement(SemanticStatementKindV1::AtomicCompareExchange(
            SemanticAtomicCompareExchangeV1::new(
                place(7, PAIR),
                pointer_target,
                constant(0, U32),
                constant(1, U32),
                access,
                SemanticAtomicOrderingV1::Relaxed,
                false,
            ),
        )),
    ] {
        let mut blocks = fixture();
        blocks[2].0.insert(0, kill);
        accepts(blocks, false);
    }
}

#[test]
fn long_alias_chains_are_iterative_and_uninitialized_cycles_are_not_values() {
    let mut blocks = fixture();
    let checked = std::mem::take(&mut blocks[0].0);
    blocks[0].0.push(alias(5, copy(1, U32)));
    for index in 0..2048 {
        let (destination, source) = if index % 2 == 0 { (6, 5) } else { (5, 6) };
        blocks[0].0.push(alias(destination, copy(source, U32)));
    }
    blocks[0].0.extend(checked);
    blocks[2].0[0] = unchecked(copy(5, U32), copy(2, U32), SemanticUncheckedBinaryOpV1::Add);
    accepts(blocks, true);

    let mut blocks = fixture();
    blocks[0]
        .0
        .splice(0..0, [alias(5, copy(6, U32)), alias(6, copy(5, U32))]);
    blocks[2].0[0] = unchecked(copy(5, U32), copy(2, U32), SemanticUncheckedBinaryOpV1::Add);
    accepts(blocks, false);
}

#[test]
fn calls_and_drop_edges_kill_values_even_without_an_explicit_local_write() {
    for terminator in [
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(1),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(13, U32),
                    edge(SemanticEdgeRoleV1::CallReturn, 3),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
        SemanticTerminatorKindV1::Drop {
            place: place(13, U32),
            drop_glue: SemanticFunctionIdV1::from_index(1),
            target: edge(SemanticEdgeRoleV1::DropReturn, 3),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
    ] {
        let mut blocks = fixture();
        let consumer = std::mem::replace(&mut blocks[2], (vec![], terminator));
        blocks.push(consumer);
        accepts(blocks, false);
    }
}

#[test]
fn backedges_preserve_only_invariant_values_not_repeated_definition_sites() {
    let mut blocks = fixture();
    let mut consumer = std::mem::replace(&mut blocks[2], (vec![], goto(3)));
    consumer.1 = goto(3);
    blocks.push(consumer);
    accepts(blocks.clone(), true);
    blocks[3].0.push(alias(1, constant(9, U32)));
    accepts(blocks, false);

    let mut blocks = fixture();
    let producer = std::mem::replace(&mut blocks[0], (vec![], goto(3)));
    blocks[2].1 = goto(3);
    blocks.push(producer);
    accepts(blocks, false);
}

#[test]
fn provenance_queries_share_the_fixed_work_budget() {
    let function = function(fixture());
    let mut budget = WorkBudgetV1::default();
    let dominators = DominatorIntervalsV1::analyze(&function, &mut budget).unwrap();
    let mut provenance = ArithmeticProvenanceV1::new(&function, &dominators, &mut budget).unwrap();
    budget.used = MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1 - 2;
    assert!(matches!(
        provenance.same_value(
            &copy(1, U32),
            ArithmeticSiteV1 {
                block: 0,
                statement: 0
            },
            &copy(1, U32),
            ArithmeticSiteV1 {
                block: 2,
                statement: 0
            },
            &mut budget
        ),
        Err(SemanticOptionDominanceErrorV1::WorkLimit { .. })
    ));
}
