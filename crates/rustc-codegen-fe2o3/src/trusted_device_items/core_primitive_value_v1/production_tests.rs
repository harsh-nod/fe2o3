use super::*;
use rustc_middle::mir::BasicBlock;

pub(crate) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    called: &impl Fn(&str) -> Instance<'tcx>,
    body: &impl Fn(&str) -> &'tcx Body<'tcx>,
) {
    let mut fingerprints = Vec::new();
    for (name, route, helper, cast_kind) in [
        (
            "from_u32",
            "route_u32",
            Helper::U32ToU64,
            CastKind::IntToInt,
        ),
        ("from_u8", "route_u8", Helper::U8ToF32, CastKind::IntToFloat),
    ] {
        let instance = called(name);
        let source = tcx.instance_mir(instance.def);
        super::header_tests::check(tcx, instance, helper, source, called("foreign_from"));
        let original = prove_core_primitive_cast_v1(tcx, instance).unwrap();
        let proof: super::super::ReviewedCorePrimitiveCastV1<'tcx> =
            super::super::prove_core_primitive_cast_v1(tcx, instance).unwrap();
        assert_eq!(proof.instance(), instance);
        assert_eq!(proof, original);
        let expansion = proof.expand_mir(tcx);
        require_cast(tcx, &expansion, helper, cast_kind);
        assert!(proof.revalidate(tcx, instance, &expansion));
        assert_eq!(proof, prove_core_primitive_cast_v1(tcx, instance).unwrap());
        assert!(std::ptr::eq(source, tcx.instance_mir(instance.def)));
        assert_eq!(expansion.span, source.span);
        fingerprints.push(proof.expansion_fingerprint(tcx, &expansion));

        for other in ["from_u32", "from_u8", "wrong_width", "foreign_from"] {
            let other = called(other);
            if other != instance {
                assert!(!proof.revalidate(tcx, other, &expansion));
            }
        }
        let entry = BasicBlock::from_usize(0);
        for mutation in 0..7 {
            let mut changed = expansion.clone();
            match mutation {
                0 => {
                    let StatementKind::Assign(assignment) =
                        &mut changed.basic_blocks_mut()[entry].statements[0].kind
                    else {
                        unreachable!()
                    };
                    let Rvalue::Cast(kind, _, _) = &mut assignment.1 else {
                        unreachable!()
                    };
                    *kind = CastKind::Transmute;
                }
                1 | 2 => {
                    let StatementKind::Assign(assignment) =
                        &mut changed.basic_blocks_mut()[entry].statements[0].kind
                    else {
                        unreachable!()
                    };
                    let Rvalue::Cast(_, operand, _) = &mut assignment.1 else {
                        unreachable!()
                    };
                    *operand = if mutation == 1 {
                        Operand::Move(Local::from_usize(1).into())
                    } else {
                        Operand::Copy(Local::from_usize(0).into())
                    };
                }
                3 => changed.local_decls[Local::from_usize(1)].ty = tcx.types.u16,
                4 => {
                    changed.basic_blocks_mut()[entry].terminator_mut().kind =
                        TerminatorKind::Goto { target: entry };
                }
                5 => {
                    changed.basic_blocks_mut()[entry]
                        .statements
                        .push(Statement::new(
                            SourceInfo::outermost(source.span),
                            StatementKind::Nop,
                        ));
                }
                6 => changed.basic_blocks_mut()[entry].is_cleanup = true,
                _ => unreachable!(),
            }
            assert!(
                !proof.revalidate(tcx, instance, &changed),
                "{name}:{mutation}"
            );
            assert_ne!(
                proof.expansion_fingerprint(tcx, &expansion),
                proof.expansion_fingerprint(tcx, &changed)
            );
        }
        let mut changed = prove_core_primitive_cast_v1(tcx, instance).unwrap();
        changed.source_fingerprint[0] ^= 1;
        assert!(!changed.revalidate(tcx, instance, &expansion));

        // The locally rebuilt source is only a body-proof test input. It cannot
        // become an authenticated production instance or replace live core MIR.
        let retained = body(route);
        assert_eq!(retained.basic_blocks.len(), 11);
        assert!(reviewed_body(tcx, retained, helper));
        assert!(prove_body(tcx, instance, retained, helper).is_none());
        // Only this private mutation test can associate a changed body with the
        // core owner. Replay must still reject its noncanonical source proof.
        let mut bound = retained.clone();
        bound.source = source.source;
        let retained_proof = prove_body(tcx, instance, &bound, helper).unwrap();
        assert_ne!(retained_proof, proof);
        let retained_expansion = expand_body(&bound, helper, tcx);
        require_cast(tcx, &retained_expansion, helper, cast_kind);
        assert!(!retained_proof.revalidate(tcx, instance, &retained_expansion));
        assert!(
            prove_core_primitive_cast_v1(
                tcx,
                Instance {
                    def: retained.source.instance,
                    args: ty::List::empty(),
                }
            )
            .is_none()
        );
        let mut malformed = bound;
        malformed.basic_blocks_mut()[entry]
            .statements
            .push(Statement::new(
                SourceInfo::outermost(retained.span),
                StatementKind::Nop,
            ));
        assert!(prove_body(tcx, instance, &malformed, helper).is_none());
    }
    assert_ne!(fingerprints[0], fingerprints[1]);
    for name in [
        "ne_u32",
        "ne_f32",
        "wrong_width",
        "wrong_target",
        "wrong_identity",
        "foreign_from",
    ] {
        assert!(
            prove_core_primitive_cast_v1(tcx, called(name)).is_none(),
            "{name}"
        );
    }
}

fn require_cast<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, helper: Helper<'tcx>, kind: CastKind) {
    assert!(reviewed_body(tcx, body, helper));
    assert_eq!(body.arg_count, 1);
    assert_eq!(body.local_decls.len(), 2);
    assert_eq!(body.source_scopes.len(), 1);
    assert!(body.var_debug_info.is_empty());
    assert_eq!(body.basic_blocks.len(), 1);
    let block = &body.basic_blocks[BasicBlock::from_usize(0)];
    assert!(!block.is_cleanup);
    assert!(matches!(block.terminator().kind, TerminatorKind::Return));
    let [statement] = block.statements.as_slice() else {
        panic!("cast expansion must contain exactly one operation")
    };
    let StatementKind::Assign(assignment) = &statement.kind else {
        panic!("cast expansion must assign its result")
    };
    assert_eq!(assignment.0, Local::from_usize(0).into());
    assert!(matches!(
        &assignment.1,
        Rvalue::Cast(actual, Operand::Copy(source), target)
            if *actual == kind && *source == Local::from_usize(1).into()
                && *target == helper.output(tcx)
    ));
}
