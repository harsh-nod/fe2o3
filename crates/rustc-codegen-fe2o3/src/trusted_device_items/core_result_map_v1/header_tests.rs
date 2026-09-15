use super::mutations::{append, statement};
use super::*;
use rustc_middle::mir::{BasicBlock, MentionedItem, MirPhase, Promoted, RuntimePhase, SourceScope};
use rustc_middle::ty;
use rustc_span::{DUMMY_SP, Spanned};

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>) {
    let instance = helper(tcx, "owned");
    let c = contract(tcx, instance).unwrap();
    let source = tcx.instance_mir(instance.def);
    let accepts = |body: &Body<'tcx>| reviewed_body(tcx, instance, body, &c);
    assert!(accepts(source));
    body::check_work_boundaries(tcx, instance, source, &c);
    for mutation in 0..19 {
        let mut b = source.clone();
        match mutation {
            0 => b.source.instance = helper(tcx, "foreign").def,
            1 => b.source.promoted = Some(Promoted::from_usize(0)),
            2 => b.phase = MirPhase::Runtime(RuntimePhase::Initial),
            3 => b.injection_phase = Some(b.phase),
            4 => b.is_polymorphic = false,
            5 => {
                b.coroutine = Some(
                    tcx.iter_local_def_id()
                        .filter(|id| tcx.def_kind(*id) == DefKind::Closure)
                        .find_map(|id| tcx.optimized_mir(id).coroutine.clone())
                        .expect("fixture coroutine"),
                )
            }
            6 => b.spread_arg = Some(Local::from_usize(2)),
            7 => b.arg_count = 1,
            8 => b.required_consts = None,
            9 => {
                b.required_consts = Some(vec![rustc_middle::mir::ConstOperand {
                    span: DUMMY_SP,
                    user_ty: None,
                    const_: Const::from_bool(tcx, false),
                }])
            }
            10 => {
                b.mentioned_items = Some(vec![Spanned {
                    node: MentionedItem::Fn(tcx.types.u32),
                    span: DUMMY_SP,
                }])
            }
            11 => b.local_decls[Local::from_usize(0)].ty = c.input,
            12 => b.local_decls[Local::from_usize(1)].ty = c.output,
            13 => b.local_decls[Local::from_usize(2)].ty = c.parameters[0],
            14 => b.source_scopes.raw[0].parent_scope = Some(SourceScope::from_usize(0)),
            15 => b.source_scopes.raw[0].inlined_parent_scope = Some(SourceScope::from_usize(0)),
            16 => {
                b.local_decls[Local::from_usize(0)].source_info.scope =
                    SourceScope::from_usize(MAX_SCOPES)
            }
            17 => b.basic_blocks_mut()[BasicBlock::from_usize(0)].terminator = None,
            18 => {
                #[allow(deprecated)]
                let error = rustc_span::ErrorGuaranteed::unchecked_error_guaranteed();
                b.tainted_by_errors = Some(error);
            }
            _ => unreachable!(),
        }
        assert!(!accepts(&b), "header mutation={mutation}");
    }
    for count in [MAX_LOCALS - 1, MAX_LOCALS, MAX_LOCALS + 1] {
        let mut b = source.clone();
        b.local_decls
            .raw
            .resize(count, source.local_decls[Local::from_usize(3)].clone());
        assert_eq!(accepts(&b), count <= MAX_LOCALS, "local count={count}");
    }
    for count in [MAX_BLOCKS - 1, MAX_BLOCKS, MAX_BLOCKS + 1] {
        let mut b = source.clone();
        while b.basic_blocks.len() < count {
            append(&mut b, TerminatorKind::Unreachable, false);
        }
        assert_eq!(accepts(&b), count <= MAX_BLOCKS, "block count={count}");
    }
    for count in [MAX_STATEMENTS - 1, MAX_STATEMENTS, MAX_STATEMENTS + 1] {
        let mut b = source.clone();
        let old = b
            .basic_blocks
            .iter()
            .map(|block| block.statements.len())
            .sum::<usize>();
        b.basic_blocks_mut()[BasicBlock::from_usize(0)]
            .statements
            .extend((old..count).map(|_| statement(StatementKind::Nop)));
        assert_eq!(
            accepts(&b),
            count <= MAX_STATEMENTS,
            "statement count={count}"
        );
    }
    for count in [MAX_SCOPES - 1, MAX_SCOPES, MAX_SCOPES + 1] {
        let mut b = source.clone();
        let mut scope = source.source_scopes.raw[0].clone();
        scope.parent_scope = Some(SourceScope::from_usize(0));
        b.source_scopes.raw.resize(count, scope);
        assert_eq!(accepts(&b), count <= MAX_SCOPES, "scope count={count}");
    }
    for count in [MAX_DEBUG_INFO - 1, MAX_DEBUG_INFO, MAX_DEBUG_INFO + 1] {
        let mut b = source.clone();
        b.var_debug_info
            .resize(count, source.var_debug_info[0].clone());
        assert_eq!(accepts(&b), count <= MAX_DEBUG_INFO, "debug count={count}");
    }
    let mut b = source.clone();
    b.user_type_annotations
        .push(ty::CanonicalUserTypeAnnotation {
            user_ty: Box::new(ty::CanonicalUserType {
                max_universe: ty::UniverseIndex::ROOT,
                var_kinds: ty::List::empty(),
                value: ty::UserType::new(ty::UserTypeKind::Ty(tcx.types.u64)),
            }),
            span: DUMMY_SP,
            inferred_ty: tcx.types.u64,
        });
    assert!(!accepts(&b), "annotations bounded before any body work");
    let mut b = source.clone();
    b.mentioned_items = Some(vec![
        Spanned {
            node: MentionedItem::Fn(c.callback),
            span: DUMMY_SP
        };
        MAX_MENTIONED_ITEMS + 1
    ]);
    assert!(!accepts(&b), "mentioned-items bound");
}
