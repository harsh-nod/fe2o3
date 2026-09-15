use super::*;
use ty::adjustment::{
    Adjust, Adjustment, AllowTwoPhase, AutoBorrow, AutoBorrowMutability, DerefAdjustKind,
    OverloadedDeref, PointerCoercion,
};

struct Probe {
    ran: bool,
    mutations: bool,
}

struct BorrowFinder<'tcx>(Vec<&'tcx Expr<'tcx>>);
impl<'tcx> Visitor<'tcx> for BorrowFinder<'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(expression.kind, ExprKind::AddrOf(..)) {
            self.0.push(expression);
        }
        intravisit::walk_expr(self, expression);
    }
}

impl rustc_driver::Callbacks for Probe {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        config.input = rustc_session::config::Input::Str {
            name: rustc_span::FileName::Custom("context_entry_adjustments.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(
        &mut self,
        _: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        for name in ["custom_borrow", "mutable_borrow", "raw_borrow"] {
            let root = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == rustc_hir::def::DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap();
            let typeck = tcx.typeck(root);
            let mut finder = BorrowFinder(Vec::new());
            finder.visit_expr(tcx.hir_body_owned_by(root).value);
            let [borrow] = finder.0.as_slice() else {
                panic!("one explicit borrow in {name}");
            };
            let ExprKind::AddrOf(_, _, owner) = borrow.kind else {
                unreachable!()
            };
            let owner = typeck.expr_ty(owner);
            let raw = typeck.expr_ty(borrow);
            let adjusted = typeck.expr_ty_adjusted(borrow);
            let observed = typeck.expr_adjustments(borrow);
            if name != "custom_borrow" {
                assert!(!identity_shared_receiver_adjustments(
                    tcx, owner, raw, adjusted, observed
                ));
                continue;
            }
            assert!(matches!(
                observed,
                [
                    Adjustment {
                        kind: Adjust::Deref(DerefAdjustKind::Builtin),
                        ..
                    },
                    Adjustment {
                        kind: Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not)),
                        ..
                    }
                ]
            ));
            assert!(identity_shared_receiver_adjustments(
                tcx, owner, raw, adjusted, observed
            ));
            assert!(identity_shared_receiver_adjustments(
                tcx,
                owner,
                raw,
                raw,
                &[]
            ));
            // This is shared adjustment mechanics, not provider authentication.
            // The sibling HIR test still rejects this actual local custom call.
            if self.mutations {
                reject_mutations(tcx, owner, raw, adjusted, observed);
            }
        }
        self.ran = true;
        rustc_driver::Compilation::Stop
    }
}

fn reject_mutations<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner: Ty<'tcx>,
    raw: Ty<'tcx>,
    adjusted: Ty<'tcx>,
    observed: &[Adjustment<'tcx>],
) {
    let rejects = |chain: &[Adjustment<'tcx>]| {
        assert!(
            !identity_shared_receiver_adjustments(tcx, owner, raw, adjusted, chain),
            "accepted changed adjustment: {chain:?}"
        )
    };
    rejects(&observed[..1]);
    rejects(&observed[1..]);
    let mut reversed = observed.to_vec();
    reversed.reverse();
    rejects(&reversed);
    let mut extra = observed.to_vec();
    extra.push(observed[1].clone());
    rejects(&extra);
    for kind in [
        Adjust::Deref(DerefAdjustKind::Overloaded(OverloadedDeref {
            mutbl: rustc_hir::Mutability::Not,
            span: rustc_span::DUMMY_SP,
        })),
        Adjust::Deref(DerefAdjustKind::Pin),
        Adjust::NeverToAny,
        Adjust::Pointer(PointerCoercion::Unsize),
    ] {
        let mut changed = observed.to_vec();
        changed[0].kind = kind;
        rejects(&changed);
    }
    for kind in [
        Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut {
            allow_two_phase_borrow: AllowTwoPhase::No,
        })),
        Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut {
            allow_two_phase_borrow: AllowTwoPhase::Yes,
        })),
        Adjust::Borrow(AutoBorrow::RawPtr(rustc_hir::Mutability::Not)),
        Adjust::Borrow(AutoBorrow::Pin(rustc_hir::Mutability::Not)),
        Adjust::Pointer(PointerCoercion::MutToConstPointer),
    ] {
        let mut changed = observed.to_vec();
        changed[1].kind = kind;
        rejects(&changed);
    }
    for index in 0..2 {
        let mut changed = observed.to_vec();
        changed[index].target = tcx.types.u32;
        rejects(&changed);
    }
    assert!(!identity_shared_receiver_adjustments(
        tcx,
        tcx.types.u32,
        raw,
        adjusted,
        observed
    ));
    assert!(!identity_shared_receiver_adjustments(
        tcx,
        owner,
        tcx.types.u32,
        adjusted,
        observed
    ));
    assert!(!identity_shared_receiver_adjustments(
        tcx,
        owner,
        raw,
        tcx.types.u32,
        observed
    ));
}

#[test]
fn context_entry_accepts_observed_builtin_shared_identity_reborrow() {
    let mut probe = Probe {
        ran: false,
        mutations: false,
    };
    run_probe(&mut probe);
    assert!(probe.ran);
}

#[test]
fn context_entry_rejects_adjustment_kind_order_type_and_mutability_substitutions() {
    let mut probe = Probe {
        ran: false,
        mutations: true,
    };
    run_probe(&mut probe);
    assert!(probe.ran);
}
