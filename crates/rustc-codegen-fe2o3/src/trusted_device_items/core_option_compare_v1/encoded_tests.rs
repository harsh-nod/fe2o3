use super::*;
use rustc_middle::mir::AggregateKind;

pub(super) fn check_profiles(tcx: TyCtxt<'_>) {
    for (name, expected) in [("eq_u64", (9, 14, 2, 14)), ("ne_u64", (2, 4, 1, 1))] {
        let instance = helper(tcx, name);
        let body = tcx.instance_mir(instance.def);
        assert_eq!(
            (
                body.basic_blocks.len(),
                body.local_decls.len(),
                body.source_scopes.len(),
                body.basic_blocks
                    .iter()
                    .map(|block| block.statements.len())
                    .sum::<usize>(),
            ),
            expected,
            "actual encoded AMD core profile: {name}",
        );
    }
}

pub(super) fn check_ne_mutations<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) {
    let entry = BasicBlock::from_usize(0);
    let done = BasicBlock::from_usize(1);
    for mutation in 0..5 {
        let mut changed = body.clone();
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks.as_mut()[done].statements[0].kind
        else {
            unreachable!()
        };
        match mutation {
            0 => assignment.0 = Local::from_usize(3).into(),
            1 => assignment.1 = Rvalue::Use(Operand::Move(Local::from_usize(3).into())),
            2 => {
                assignment.1 =
                    Rvalue::UnaryOp(UnOp::Not, Operand::Copy(Local::from_usize(3).into()))
            }
            3 => {
                assignment.1 =
                    Rvalue::UnaryOp(UnOp::Not, Operand::Move(Local::from_usize(0).into()))
            }
            4 => {
                changed.basic_blocks.as_mut()[entry]
                    .statements
                    .push(Statement::new(
                        SourceInfo::outermost(DUMMY_SP),
                        StatementKind::StorageLive(Local::from_usize(3)),
                    ));
                changed.basic_blocks.as_mut()[done]
                    .statements
                    .push(Statement::new(
                        SourceInfo::outermost(DUMMY_SP),
                        StatementKind::StorageDead(Local::from_usize(3)),
                    ));
            }
            _ => unreachable!(),
        }
        assert!(
            !reviewed_body(tcx, instance, &changed, contract),
            "encoded ne result, operand kind, or mixed profile"
        );
    }
}

pub(super) fn check_eq_mutations<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    contract: &Contract<'tcx>,
) {
    let reject = |changed: &Body<'tcx>, detail| {
        assert!(
            !reviewed_body(tcx, instance, changed, contract),
            "encoded eq: {detail}"
        );
    };
    let entry = BasicBlock::from_usize(0);
    // The observed profile is fixed, while the proof also permits block renumbering.
    let invalid = BasicBlock::from_usize(1);
    let some = BasicBlock::from_usize(2);
    let none = BasicBlock::from_usize(3);
    let both = BasicBlock::from_usize(7);
    let done = BasicBlock::from_usize(8);
    for local in body.local_decls.indices() {
        let mut changed = body.clone();
        changed.local_decls[local].ty = tcx.types.u8;
        reject(&changed, "exact local types including retained references");
    }
    for mutation in 0..5 {
        let mut changed = body.clone();
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks.as_mut()[entry].statements[0].kind
        else {
            unreachable!()
        };
        let Rvalue::Aggregate(kind, operands) = &mut assignment.1 else {
            unreachable!()
        };
        let left = rustc_abi::FieldIdx::from_usize(0);
        let right = rustc_abi::FieldIdx::from_usize(1);
        match mutation {
            0 => operands.raw.swap(0, 1),
            1 => operands[right] = operands[left].clone(),
            2 => operands[left] = Operand::Move(Local::from_usize(1).into()),
            3 => *kind = Box::new(AggregateKind::Array(shared(tcx, contract.option))),
            4 => assignment.0 = Local::from_usize(9).into(),
            _ => unreachable!(),
        }
        reject(
            &changed,
            "ordered tuple references, operand kind, or aggregate type",
        );
    }
    for (block, statement, field) in [
        (entry, 1, 0),
        (some, 0, 1),
        (none, 0, 1),
        (both, 0, 0),
        (both, 2, 1),
    ] {
        for mutation in 0..6 {
            let mut changed = body.clone();
            let StatementKind::Assign(assignment) =
                &mut changed.basic_blocks.as_mut()[block].statements[statement].kind
            else {
                unreachable!()
            };
            let Rvalue::Use(Operand::Copy(place)) = &mut assignment.1 else {
                unreachable!()
            };
            match mutation {
                0 => place.local = Local::from_usize(1),
                1 => {
                    place.projection = tcx.mk_place_elems(&[ProjectionElem::Field(
                        rustc_abi::FieldIdx::from_usize(1 - field),
                        shared(tcx, contract.option),
                    )])
                }
                2 => {
                    place.projection = tcx.mk_place_elems(&[ProjectionElem::Field(
                        rustc_abi::FieldIdx::from_usize(field),
                        shared(tcx, contract.payload),
                    )])
                }
                3 => place.projection = tcx.mk_place_elems(&[]),
                4 => assignment.1 = Rvalue::Use(Operand::Move(*place)),
                5 => assignment.0 = Local::from_usize(3).into(),
                _ => unreachable!(),
            }
            reject(
                &changed,
                "tuple projection retains exact side, reference type, and value",
            );
        }
    }
    for (block, statement, local) in [(entry, 2, 6), (some, 1, 4), (none, 1, 5)] {
        let mut changed = body.clone();
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks.as_mut()[block].statements[statement].kind
        else {
            unreachable!()
        };
        assignment.1 = Rvalue::Discriminant(Place {
            local: Local::from_usize(1),
            projection: tcx.mk_place_elems(&[ProjectionElem::Deref]),
        });
        reject(
            &changed,
            "discriminant must use its exact forwarded reference",
        );
        for mutation in 0..8 {
            let mut changed = body.clone();
            let TerminatorKind::SwitchInt { discr, targets } =
                &mut changed.basic_blocks.as_mut()[block].terminator_mut().kind
            else {
                unreachable!()
            };
            let n = targets.target_for_value(contract.none_discriminant);
            let s = targets.target_for_value(contract.some_discriminant);
            match mutation {
                0 => {
                    *targets = SwitchTargets::new(
                        [
                            (contract.none_discriminant, s),
                            (contract.some_discriminant, n),
                        ]
                        .into_iter(),
                        invalid,
                    )
                }
                1 => {
                    *targets = SwitchTargets::new(
                        [
                            (contract.none_discriminant, n),
                            (contract.some_discriminant, s),
                        ]
                        .into_iter(),
                        done,
                    )
                }
                2 => {
                    *targets = SwitchTargets::new(
                        [
                            (contract.none_discriminant, n),
                            (contract.none_discriminant, s),
                        ]
                        .into_iter(),
                        invalid,
                    )
                }
                3 => {
                    *targets = SwitchTargets::new(
                        [
                            (contract.none_discriminant, n),
                            (contract.some_discriminant, BasicBlock::from_usize(9)),
                        ]
                        .into_iter(),
                        invalid,
                    )
                }
                4 => *targets = SwitchTargets::static_if(contract.none_discriminant, n, s),
                5 => *discr = Operand::Move(Local::from_usize(3).into()),
                6 => *discr = Operand::Copy(Local::from_usize(local).into()),
                7 => {
                    *targets = SwitchTargets::new(
                        [
                            (contract.none_discriminant, n),
                            (contract.some_discriminant, n),
                        ]
                        .into_iter(),
                        invalid,
                    )
                }
                _ => unreachable!(),
            }
            reject(
                &changed,
                "all discriminant branches, invalid sink, and distinct block roles",
            );
        }
    }
    for (statement, other) in [(1, 13), (3, 12)] {
        for mutation in 0..9 {
            let mut changed = body.clone();
            let StatementKind::Assign(assignment) =
                &mut changed.basic_blocks.as_mut()[both].statements[statement].kind
            else {
                unreachable!()
            };
            let Rvalue::Ref(_, borrow, place) = &mut assignment.1 else {
                unreachable!()
            };
            match mutation {
                0 => place.local = Local::from_usize(other),
                1 => place.projection = tcx.mk_place_elems(&[]),
                2 => place.projection = tcx.mk_place_elems(&[ProjectionElem::Deref]),
                3 | 4 | 5 => {
                    let mut projection = place.projection.to_vec();
                    if mutation == 3 {
                        projection[1] =
                            ProjectionElem::Downcast(None, rustc_abi::VariantIdx::from_usize(0));
                    } else {
                        projection[2] = ProjectionElem::Field(
                            rustc_abi::FieldIdx::from_usize(usize::from(mutation == 4)),
                            if mutation == 5 {
                                tcx.types.u8
                            } else {
                                contract.payload
                            },
                        );
                    }
                    place.projection = tcx.mk_place_elems(&projection);
                }
                6 => {
                    *borrow = BorrowKind::Mut {
                        kind: rustc_middle::mir::MutBorrowKind::Default,
                    }
                }
                7 => assignment.0 = Local::from_usize(0).into(),
                8 => assignment.1 = Rvalue::Use(Operand::Move(*place)),
                _ => unreachable!(),
            }
            reject(
                &changed,
                "both payload references require exact Some fields and shared borrows",
            );
        }
    }
    for block in [4, 5, 6].map(BasicBlock::from_usize) {
        for mutation in 0..3 {
            let mut changed = body.clone();
            match mutation {
                0 => {
                    let StatementKind::Assign(assignment) =
                        &mut changed.basic_blocks.as_mut()[block].statements[0].kind
                    else {
                        unreachable!()
                    };
                    assignment.1 = Rvalue::Use(Operand::Constant(Box::new(ConstOperand {
                        span: DUMMY_SP,
                        user_ty: None,
                        const_: Const::from_bool(tcx, block.as_usize() != 4),
                    })));
                }
                1 => {
                    changed.basic_blocks.as_mut()[block].terminator_mut().kind =
                        TerminatorKind::Goto { target: both }
                }
                2 => {
                    changed.basic_blocks.as_mut()[block].terminator_mut().kind =
                        TerminatorKind::Goto { target: invalid }
                }
                _ => unreachable!(),
            }
            reject(
                &changed,
                "both None and mixed variants retain exact results without payload access",
            );
        }
    }
}
