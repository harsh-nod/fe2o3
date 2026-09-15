use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum CarrierMutation {
    None,
    ProjectedMove,
    MoveThenLeafCopy,
    MoveThenWholeCopy,
    DoubleLeafMove,
    DeadBeforeLeafMove,
    UninitializedBeforeLeafMove,
    BorrowAfterLeafMove,
    SiblingMoveAfterLeafMove,
    SiblingMoveBeforeLeafMove,
    SiblingMovePolicyReference,
    SiblingMoveThenSiblingCopy,
    SiblingMoveThenWholeCopy,
    DoubleSiblingMove,
    DeadBeforeSiblingMove,
    UninitializedBeforeSiblingMove,
    DuplicateDefinition,
    BorrowedCarrier,
    PointeeObservation,
    TwoMatrixLeaves,
}

// Retained source transport before the existing canonical Bind. No binding,
// issuer or planner record is fabricated or granted by the tuple's shape.
pub(in super::super) fn source(mutation: CarrierMutation) -> AdmittedInertSemanticMirV1 {
    use SemanticLocalRoleV1::Temporary as Tmp;
    let seed = super::source(Mutation::None);
    let two = mutation == CarrierMutation::TwoMatrixLeaves;
    let second = if two { 5 } else if mutation == CarrierMutation::SiblingMovePolicyReference { 6 } else { 0 };
    let pair = second != 0;
    let fields = vec![ty(5), ty(second)];
    let aggregate = aggregate_type(13, &[5, second], &[0, 8], if pair { 16 } else { 8 }, pair);
    let tuple = SemanticTypeDeclV1::new(
        aggregate.identity(),
        aggregate.layout_identity(),
        aggregate.layout().clone(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_rustc_layout_is_noundef(true)
            .with_scalar_pointee_info(Some(pointee(0, 1)), pair.then_some(pointee(0, 1))),
    );
    let tuple_reference = reference(14, 13).with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false)
            .with_rustc_layout_is_noundef(true)
            .with_scalar_pointee_info(Some(pointee(if pair { 16 } else { 8 }, 8)), None),
    );
    let mut types = seed.types().to_vec();
    assert_eq!(types.len(), 13);
    types.extend([tuple, tuple_reference]);
    let old = &seed.functions()[0];
    let mut locals = old
        .locals()
        .iter()
        .map(|local| (local.ty().index(), local.role()))
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 13);
    locals.extend([(13, Tmp), (13, Tmp), (5, Tmp), (second, Tmp), (14, Tmp)]);
    if matches!(
        mutation,
        CarrierMutation::MoveThenLeafCopy
            | CarrierMutation::DoubleLeafMove
            | CarrierMutation::MoveThenWholeCopy
    ) {
        locals.extend([(5, Tmp), (13, Tmp)]);
    }
    if matches!(
        mutation,
        CarrierMutation::SiblingMoveThenSiblingCopy
            | CarrierMutation::SiblingMoveThenWholeCopy
            | CarrierMutation::DoubleSiblingMove
    ) {
        locals.extend([(second, Tmp), (13, Tmp)]);
    }
    let tuple_value = || {
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![copy(5, 5), if pair { copy(second, second) } else { zero(0) }],
            )
            .unwrap(),
        )
    };
    let field = |index, result| {
        SemanticPlaceV1::new(
            local_id(14),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty(result))
                    .unwrap(),
            ],
            ty(result),
        )
        .unwrap()
    };
    let mut statements = old.blocks()[3].statements().to_vec();
    statements.push(assign(13, 13, tuple_value()));
    if mutation == CarrierMutation::DuplicateDefinition {
        statements.push(assign(13, 13, tuple_value()));
    }
    statements.push(assign(
        14,
        13,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(13, 13))),
    ));
    if mutation == CarrierMutation::DeadBeforeLeafMove {
        statements.push(SemanticStatementV1::new(
            loc(),
            SemanticStatementKindV1::StorageDead(local_id(14)),
        ));
    }
    if mutation == CarrierMutation::UninitializedBeforeLeafMove {
        statements.push(SemanticStatementV1::new(
            loc(),
            SemanticStatementKindV1::Deinitialize(place(14, 13)),
        ));
    }
    let sibling_move = matches!(
        mutation,
        CarrierMutation::SiblingMoveAfterLeafMove
            | CarrierMutation::SiblingMoveBeforeLeafMove
            | CarrierMutation::SiblingMovePolicyReference
            | CarrierMutation::SiblingMoveThenSiblingCopy
            | CarrierMutation::SiblingMoveThenWholeCopy
            | CarrierMutation::DoubleSiblingMove
            | CarrierMutation::DeadBeforeSiblingMove
            | CarrierMutation::UninitializedBeforeSiblingMove
    );
    let leaf_move = sibling_move || matches!(
        mutation,
        CarrierMutation::ProjectedMove
            | CarrierMutation::MoveThenLeafCopy
            | CarrierMutation::MoveThenWholeCopy
            | CarrierMutation::DoubleLeafMove
            | CarrierMutation::DeadBeforeLeafMove
            | CarrierMutation::UninitializedBeforeLeafMove
            | CarrierMutation::BorrowAfterLeafMove
    );
    statements.push(assign(
        15,
        5,
        SemanticRvalueKindV1::Use(if leaf_move {
            SemanticOperandV1::Move(field(0, 5))
        } else {
            SemanticOperandV1::Copy(field(0, 5))
        }),
    ));
    if mutation == CarrierMutation::DeadBeforeSiblingMove {
        statements.push(SemanticStatementV1::new(
            loc(),
            SemanticStatementKindV1::StorageDead(local_id(14)),
        ));
    }
    if mutation == CarrierMutation::UninitializedBeforeSiblingMove {
        statements.push(SemanticStatementV1::new(
            loc(),
            SemanticStatementKindV1::Deinitialize(field(1, second)),
        ));
    }
    statements.push(assign(
        16,
        second,
        SemanticRvalueKindV1::Use(if sibling_move {
            SemanticOperandV1::Move(field(1, second))
        } else {
            SemanticOperandV1::Copy(field(1, second))
        }),
    ));
    if mutation == CarrierMutation::SiblingMoveBeforeLeafMove {
        let last = statements.len() - 1;
        statements.swap(last - 1, last);
    }
    if matches!(
        mutation,
        CarrierMutation::SiblingMoveThenSiblingCopy | CarrierMutation::DoubleSiblingMove
    ) {
        statements.push(assign(
            18,
            second,
            SemanticRvalueKindV1::Use(if mutation == CarrierMutation::DoubleSiblingMove {
                SemanticOperandV1::Move(field(1, second))
            } else {
                SemanticOperandV1::Copy(field(1, second))
            }),
        ));
    }
    if mutation == CarrierMutation::SiblingMoveThenWholeCopy {
        statements.push(assign(19, 13, SemanticRvalueKindV1::Use(copy(14, 13))));
    }
    if mutation == CarrierMutation::MoveThenLeafCopy || mutation == CarrierMutation::DoubleLeafMove
    {
        statements.push(assign(
            18,
            5,
            SemanticRvalueKindV1::Use(if mutation == CarrierMutation::DoubleLeafMove {
                SemanticOperandV1::Move(field(0, 5))
            } else {
                SemanticOperandV1::Copy(field(0, 5))
            }),
        ));
    }
    if mutation == CarrierMutation::MoveThenWholeCopy {
        statements.push(assign(19, 13, SemanticRvalueKindV1::Use(copy(14, 13))));
    }
    if mutation == CarrierMutation::BorrowedCarrier
        || mutation == CarrierMutation::BorrowAfterLeafMove
    {
        statements.push(borrow(17, 14, 14, 13));
    }
    if mutation == CarrierMutation::PointeeObservation {
        let observed = SemanticPlaceV1::new(
            local_id(15),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(2)).unwrap()],
            ty(2),
        )
        .unwrap();
        statements.push(assign(
            2,
            2,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(observed)),
        ));
    }
    let mut blocks = old.blocks().to_vec();
    blocks[3] = block(
        3,
        statements,
        call(1, vec![copy(15, 5), copy(6, 6)], 7, 7, 4),
    );
    let root = function(30, true, &[], 0, &locals, blocks)
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let mut functions = seed.functions().to_vec();
    functions[0] = root;
    InertSemanticMirRequestV1::new_with_callables(
        seed.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        seed.callables().to_vec(),
        seed.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}
