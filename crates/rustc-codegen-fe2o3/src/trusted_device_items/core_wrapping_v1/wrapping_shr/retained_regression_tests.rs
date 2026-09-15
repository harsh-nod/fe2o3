//! Regressions shared by the captured replay and actual cached-profile probes.

use super::*;
use rustc_middle::mir::interpret::{AllocInit, Allocation, GlobalAlloc};
use rustc_middle::mir::{MirPhase, RuntimePhase};

pub(super) fn exercise<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    proof: ReviewedU32WrappingShrV1<'tcx>,
    instances: &[Instance<'tcx>],
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
) where
    'tcx: 'a,
{
    let source = fetch(root);
    let original = proof.expansion_fingerprint(tcx, source);
    let expanded = proof.expand_source_mir(tcx, source);
    let fingerprint = proof.expansion_fingerprint(tcx, &expanded);
    assert_eq!(proof.instance(), root);
    assert_eq!(expanded.source, source.source);
    assert_eq!(expanded.arg_count, source.arg_count);
    assert_eq!(expanded.local_decls.len(), 4);
    assert!(
        expanded
            .local_decls
            .iter()
            .all(|decl| decl.ty == tcx.types.u32)
    );
    assert_eq!(expanded.source_scopes.len(), 1);
    assert_eq!(expanded.basic_blocks.len(), 1);
    assert_eq!(
        expanded.basic_blocks[BasicBlock::from_usize(0)]
            .statements
            .len(),
        2
    );
    assert!(retained::returns(&expanded, 0));
    assert!(
        matches!(assignment(&expanded.basic_blocks[BasicBlock::from_usize(0)].statements[0], 3),
        Some(Rvalue::BinaryOp(BinOp::BitAnd, values))
            if retained::operand(&values.0, 2, false)
                && scalar(tcx, &values.1, tcx.types.u32) == Some(31))
    );
    assert!(retained::binary(
        &expanded,
        0,
        1,
        0,
        BinOp::Shr,
        1,
        false,
        3,
        false
    ));
    let sig = signature(tcx, root);
    assert_eq!(sig.abi, ExternAbi::Rust);
    assert_eq!(sig.safety, Safety::Safe);
    assert!(!sig.c_variadic);
    assert_eq!(sig.inputs(), [tcx.types.u32, tcx.types.u32]);
    assert_eq!(sig.output(), tcx.types.u32);
    assert_ne!(original, fingerprint);
    assert_eq!(original, proof.expansion_fingerprint(tcx, fetch(root)));
    assert_eq!(
        fingerprint,
        proof.expansion_fingerprint(tcx, &proof.expand_source_mir(tcx, source))
    );
    let mut edited_expansion = expanded.clone();
    set_value(
        &mut edited_expansion,
        BasicBlock::from_usize(0),
        1,
        Rvalue::BinaryOp(BinOp::Shl, Box::new((copy(1), copy(3)))),
    );
    assert_ne!(
        fingerprint,
        proof.expansion_fingerprint(tcx, &edited_expansion)
    );

    for (&instance, helper) in instances.iter().zip(HELPERS) {
        let source = fetch(instance);
        let reject = |changed: &Body<'tcx>| {
            assert!(
                prove_with(tcx, root, &|callee| if callee == instance {
                    changed
                } else {
                    fetch(callee)
                })
                .is_none(),
                "accepted {helper:?} regression: {changed:?}"
            );
        };
        for phase in [
            MirPhase::Built,
            MirPhase::Runtime(RuntimePhase::Initial),
            MirPhase::Runtime(RuntimePhase::PostCleanup),
        ] {
            let mut changed = source.clone();
            changed.phase = phase;
            reject(&changed);
        }
        for (block, data) in source.basic_blocks.iter_enumerated() {
            let mut changed = source.clone();
            changed.basic_blocks_mut()[block]
                .terminator_mut()
                .source_info
                .scope = SourceScope::from_usize(source.source_scopes.len());
            reject(&changed);
            for (index, statement) in data.statements.iter().enumerate() {
                let StatementKind::Assign(assigned) = &statement.kind else {
                    panic!("retained assignments only")
                };
                let mut changed = source.clone();
                let StatementKind::Assign(changed_assignment) =
                    &mut changed.basic_blocks_mut()[block].statements[index].kind
                else {
                    unreachable!()
                };
                changed_assignment.0 =
                    Local::from_usize(if local(assigned.0, 0) { 1 } else { 0 }).into();
                reject(&changed);
                let mut changed = source.clone();
                changed.basic_blocks_mut()[block].statements[index]
                    .source_info
                    .scope = SourceScope::from_usize(source.source_scopes.len());
                reject(&changed);
                let count = match &assigned.1 {
                    Rvalue::BinaryOp(..) => 2,
                    Rvalue::Aggregate(_, values) => values.len(),
                    Rvalue::Use(_) | Rvalue::UnaryOp(..) | Rvalue::Cast(..) => 1,
                    Rvalue::RawPtr(..) => 0,
                    other => panic!("unreviewed retained rvalue {other:?}"),
                };
                for slot in 0..count {
                    let mut value = assigned.1.clone();
                    let operand = match &mut value {
                        Rvalue::BinaryOp(_, values) => {
                            if slot == 0 {
                                &mut values.0
                            } else {
                                &mut values.1
                            }
                        }
                        Rvalue::Aggregate(_, values) => &mut values.raw[slot],
                        Rvalue::Use(value)
                        | Rvalue::UnaryOp(_, value)
                        | Rvalue::Cast(_, value, _) => value,
                        _ => unreachable!(),
                    };
                    *operand = match operand {
                        Operand::Copy(place) => Operand::Move(*place),
                        Operand::Move(place) => Operand::Copy(*place),
                        _ => continue,
                    };
                    let mut changed = source.clone();
                    set_value(&mut changed, block, index, value);
                    reject(&changed);
                }
                if let Rvalue::Use(Operand::Constant(message)) = &assigned.1
                    && let Const::Val(ConstValue::Slice { alloc_id, meta }, ty) = message.const_
                {
                    let Some(GlobalAlloc::Memory(allocation)) = tcx.try_get_global_alloc(alloc_id)
                    else {
                        panic!("captured message allocation")
                    };
                    let bytes = allocation
                        .inner()
                        .get_bytes_strip_provenance(
                            &tcx,
                            rustc_middle::mir::interpret::alloc_range(
                                rustc_abi::Size::ZERO,
                                rustc_abi::Size::from_bytes(meta),
                            ),
                        )
                        .unwrap();
                    // Keep the same length and type so this checks the content,
                    // not merely the allocation or string-slice shape.
                    let mut wrong = bytes.to_vec();
                    wrong[0] ^= 1;
                    let wrong =
                        tcx.reserve_and_set_memory_alloc(tcx.mk_const_alloc(
                            Allocation::from_bytes_byte_aligned_immutable(wrong, ()),
                        ));
                    for (alloc_id, meta) in
                        [(wrong, meta), (alloc_id, meta - 1), (alloc_id, u64::MAX)]
                    {
                        let mut changed = source.clone();
                        set_value(
                            &mut changed,
                            block,
                            index,
                            Rvalue::Use(constant(Const::Val(
                                ConstValue::Slice { alloc_id, meta },
                                ty,
                            ))),
                        );
                        reject(&changed);
                    }
                    let short = Allocation::from_bytes_byte_aligned_immutable(&bytes[1..], ());
                    let mut mutable = allocation.inner().clone();
                    mutable.mutability = rustc_hir::Mutability::Mut;
                    let mut uninitialized = Allocation::new(
                        rustc_abi::Size::from_bytes(meta),
                        rustc_abi::Align::ONE,
                        AllocInit::Uninit,
                        (),
                    );
                    uninitialized.mutability = rustc_hir::Mutability::Not;
                    for memory in [short, mutable, uninitialized] {
                        let alloc_id = tcx.reserve_and_set_memory_alloc(tcx.mk_const_alloc(memory));
                        let mut changed = source.clone();
                        set_value(
                            &mut changed,
                            block,
                            index,
                            Rvalue::Use(constant(Const::Val(
                                ConstValue::Slice { alloc_id, meta },
                                ty,
                            ))),
                        );
                        reject(&changed);
                    }
                }
            }
        }
        let mut changed = source.clone();
        changed.span = if source.span == DUMMY_SP {
            tcx.def_span(rustc_hir::def_id::CRATE_DEF_ID)
        } else {
            DUMMY_SP
        };
        assert_ne!(source.span, changed.span, "changed source span");
        let changed_proof = prove_with(tcx, root, &|callee| {
            if callee == instance {
                &changed
            } else {
                fetch(callee)
            }
        })
        .expect("spans do not affect semantics");
        assert_ne!(
            fingerprint,
            changed_proof.expansion_fingerprint(tcx, &expanded),
            "the final expansion binds every original {helper:?} body"
        );
    }
}
