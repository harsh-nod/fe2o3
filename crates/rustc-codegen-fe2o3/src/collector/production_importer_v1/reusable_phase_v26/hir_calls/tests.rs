//! Real Rust adjustment mechanics only; the ordinary fixture grants no phase authority.
use super::*;
use rustc_middle::ty::adjustment::{AllowTwoPhase, OverloadedDeref, PointerCoercion};

const SOURCE: &str = r#"
#![no_std]
pub struct Storage;
pub fn sink(_: &mut Storage) {}
pub fn call(mut storage: Storage) { sink(&mut storage); }
"#;

struct BorrowFinder<'tcx>(Vec<&'tcx Expr<'tcx>>);
impl<'tcx> Visitor<'tcx> for BorrowFinder<'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if matches!(expression.kind, ExprKind::AddrOf(..)) {
            self.0.push(expression);
        }
        intravisit::walk_expr(self, expression);
    }
}

struct Probe {
    mutations: bool,
    ran: bool,
}

impl rustc_driver::Callbacks for Probe {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        config.input = rustc_session::config::Input::Str {
            name: rustc_span::FileName::Custom("phase_storage_reborrow.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(
        &mut self,
        _: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> rustc_driver::Compilation {
        let root = tcx
            .iter_local_def_id()
            .find(|id| {
                tcx.def_kind(*id) == rustc_hir::def::DefKind::Fn
                    && tcx.item_name(id.to_def_id()).as_str() == "call"
            })
            .unwrap();
        let caller = Instance::mono(tcx, root.to_def_id());
        let typeck = tcx.typeck(root);
        let mut finder = BorrowFinder(Vec::new());
        finder.visit_expr(tcx.hir_body_owned_by(root).value);
        let [borrow] = finder.0.as_slice() else {
            panic!("one explicit storage borrow");
        };
        let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, storage) = borrow.kind else {
            panic!();
        };
        let storage = typed(tcx, caller, typeck.expr_ty(storage)).unwrap();
        let raw = typeck.expr_ty(borrow);
        let adjusted = typeck.expr_ty_adjusted(borrow);
        let reference = typed(tcx, caller, raw).unwrap();
        let observed = typeck.expr_adjustments(borrow);
        assert!(matches!(
            observed,
            [
                Adjustment {
                    kind: Adjust::Deref(DerefAdjustKind::Builtin),
                    ..
                },
                Adjustment {
                    kind: Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut { .. })),
                    ..
                },
            ]
        ));
        let accepts = |chain: &[Adjustment<'tcx>]| {
            identity_storage_reborrow(tcx, caller, storage, reference, raw, adjusted, chain)
                .unwrap()
        };
        assert!(accepts(observed));
        assert!(accepts(&[]));
        for allow_two_phase_borrow in [AllowTwoPhase::No, AllowTwoPhase::Yes] {
            let mut chain = observed.to_vec();
            chain[1].kind = Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut {
                allow_two_phase_borrow,
            }));
            assert!(accepts(&chain));
        }
        if self.mutations {
            assert!(!accepts(&observed[..1]));
            assert!(!accepts(&observed[1..]));
            let mut reversed = observed.to_vec();
            reversed.reverse();
            assert!(!accepts(&reversed));
            let mut extra = observed.to_vec();
            extra.push(observed[1].clone());
            assert!(!accepts(&extra));
            for kind in [
                Adjust::Deref(DerefAdjustKind::Overloaded(OverloadedDeref {
                    mutbl: Mutability::Mut,
                    span: rustc_span::DUMMY_SP,
                })),
                Adjust::Deref(DerefAdjustKind::Pin),
                Adjust::NeverToAny,
                Adjust::Pointer(PointerCoercion::Unsize),
            ] {
                let mut changed = observed.to_vec();
                changed[0].kind = kind;
                assert!(!accepts(&changed));
            }
            for kind in [
                Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not)),
                Adjust::Borrow(AutoBorrow::RawPtr(Mutability::Mut)),
                Adjust::Borrow(AutoBorrow::Pin(Mutability::Mut)),
                Adjust::Pointer(PointerCoercion::MutToConstPointer),
            ] {
                let mut changed = observed.to_vec();
                changed[1].kind = kind;
                assert!(!accepts(&changed));
            }
            for index in 0..2 {
                let mut changed = observed.to_vec();
                changed[index].target = tcx.types.u32;
                assert!(!accepts(&changed));
            }
            for (storage, reference, raw, adjusted) in [
                (tcx.types.u32, reference, raw, adjusted),
                (storage, tcx.types.u32, raw, adjusted),
                (storage, reference, tcx.types.u32, adjusted),
                (storage, reference, raw, tcx.types.u32),
            ] {
                assert!(
                    !identity_storage_reborrow(
                        tcx, caller, storage, reference, raw, adjusted, observed
                    )
                    .unwrap()
                );
            }
        }
        self.ran = true;
        rustc_driver::Compilation::Stop
    }
}

fn run(mutations: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-phase-storage-reborrow");
    let args = vec![
        "rustc".into(),
        "--crate-name=phase_storage_reborrow".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        format!("--out-dir={}", scratch.path().display()),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-".into(),
    ];
    let mut probe = Probe {
        mutations,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
fn phase_storage_accepts_observed_builtin_identity_mutable_reborrow() {
    run(false);
}

#[test]
fn phase_storage_rejects_changed_kind_order_pointee_reference_and_targets() {
    run(true);
}
