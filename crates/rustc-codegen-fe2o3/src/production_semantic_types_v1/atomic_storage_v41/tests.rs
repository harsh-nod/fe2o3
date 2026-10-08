use super::*;
use crate::test_temp_dir::TestTempDir;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::ty::layout::LayoutOf;
const SOURCE: &str = r#"
use core::sync::atomic::{AtomicI32,AtomicU32,AtomicU64};
use core::cell::UnsafeCell;
pub struct Atomic<T> { v: UnsafeCell<T> }
pub fn probe(a: AtomicI32,b: AtomicU32,c:AtomicU64,d:UnsafeCell<u32>,e:Atomic<u32>) {}
"#;
#[derive(Default)]
struct Observe {
    done: bool,
}
impl Callbacks for Observe {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let def = tcx
            .iter_local_def_id()
            .find(|id| {
                tcx.def_kind(id.to_def_id()) == DefKind::Fn
                    && tcx.item_name(id.to_def_id()).as_str() == "probe"
            })
            .unwrap();
        let signature =
            tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(def).instantiate_identity());
        let inputs = signature.inputs();
        assert_eq!(inputs.len(), 5);
        for (index, kind) in [
            SemanticRustTypeKindV1::AtomicI32,
            SemanticRustTypeKindV1::AtomicU32,
        ]
        .into_iter()
        .enumerate()
        {
            let observed = authenticated_shape(tcx, inputs[index]);
            let normalized =
                tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), inputs[index]);
            let normalized_observed = normalized
                .as_ref()
                .ok()
                .map(|ty| authenticated_shape(tcx, *ty));
            let root_facts = match inputs[index].kind() {
                TyKind::Adt(definition, arguments) => Some((
                    definition.did(),
                    tcx.def_path_str(definition.did()),
                    tcx.crate_name(definition.did().krate),
                    arguments,
                )),
                _ => None,
            };
            assert!(
                matches!(&observed, Ok(Some(_))),
                "actual atomic classifier refused: index={index}; input={:?}; kind={:?}; root={root_facts:?}; sized={:?}; atomic_diagnostic={:?}; normalized={normalized:?}; normalized_observed={normalized_observed:?}; original={observed:?}",
                inputs[index],
                inputs[index].kind(),
                tcx.lang_items().sized_trait(),
                tcx.get_diagnostic_item(Symbol::intern("Atomic"))
            );
            let (actual, chain) = observed.unwrap().unwrap();
            // Canonical DefKeys remain the original core definition even when
            // the pretty printer selects a public std re-export.
            let TyKind::Adt(atomic, _) = *inputs[index].kind() else {
                panic!("original atomic ADT");
            };
            let core = tcx.lang_items().sized_trait().unwrap().krate;
            assert!(original_core_type_path(
                tcx,
                atomic.did(),
                core,
                &["sync", "atomic", "Atomic"]
            ));
            for wrong in [
                &["sync", "atomic", "AtomicI32"][..],
                &["atomic", "sync", "Atomic"][..],
                &["sync", "Atomic"][..],
                &["sync", "atomic", "private", "Atomic"][..],
            ] {
                assert!(!original_core_type_path(tcx, atomic.did(), core, wrong));
            }
            assert!(!original_core_type_path(
                tcx,
                atomic.did(),
                def.to_def_id().krate,
                &["sync", "atomic", "Atomic"]
            ));
            let TyKind::Adt(aligned, _) = *chain[1].kind() else {
                panic!("original aligned ADT");
            };
            assert!(original_core_type_path(
                tcx,
                aligned.did(),
                core,
                &["sync", "atomic", "private", "Align4"]
            ));
            assert!(!original_core_type_path(
                tcx,
                aligned.did(),
                core,
                &["sync", "atomic", "Align4"]
            ));

            assert_eq!(actual, kind);
            assert_eq!(
                chain[2],
                if index == 0 {
                    tcx.types.i32
                } else {
                    tcx.types.u32
                }
            );
            // Neither the cell nor the aligned child independently grants a tag.
            for child in chain {
                assert!(authenticated_shape(tcx, child).unwrap().is_none());
            }

            // The table is derived from this callback's original Ty/layout producers,
            // not fabricated semantic tags or a claim of an admitted transaction.
            let mut original = std::collections::BTreeMap::new();
            for ty in std::iter::once(inputs[index]).chain(chain) {
                let ty = tcx
                    .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), ty)
                    .unwrap();
                original.insert(
                    crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, ty),
                    ty,
                );
            }
            assert_eq!(original.len(), 4);
            let target = crate::rustc_semantic_adapter_v1::canonical_target_layout_v1(
                &crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx).unwrap(),
            );
            let layout_cx =
                rustc_middle::ty::layout::LayoutCx::new(tcx, TypingEnv::fully_monomorphized());
            let producers: Vec<_> = original
                .into_iter()
                .map(|(identity, ty)| {
                    let layout = layout_cx.layout_of(ty).unwrap();
                    crate::rustc_semantic_plan_v1::RetainedSemanticTypeProducerV1 {
                        identity,
                        ty,
                        layout,
                        rustc_layout_sha256:
                            crate::rustc_semantic_adapter_v1::rustc_type_layout_sha256_v1(
                                tcx, layout,
                            ),
                        semantic_layout_identity:
                            crate::rustc_semantic_adapter_v1::rustc_semantic_layout_identity_v1(
                                tcx, target, layout,
                            ),
                    }
                })
                .collect();
            let ordinary =
                crate::production_semantic_types_v1::construct_production_semantic_types_v1(
                    tcx, &producers,
                )
                .unwrap()
                .into_records();
            let tagged = crate::production_semantic_types_v1::construct_production_semantic_types_atomic_v41(
                tcx, &producers).unwrap().into_records();
            let root_identity =
                crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, inputs[index]);
            let root = tagged
                .iter()
                .position(|row| row.identity() == root_identity)
                .unwrap();
            let observed = fe2o3_mir_model::semantic_mir_v1::semantic_atomic_storage_chain_v41(
                &tagged,
                fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1::from_index(root as u32),
            )
            .unwrap();
            for (id, ty) in observed.into_iter().zip(chain) {
                assert_eq!(
                    tagged[id.index() as usize].identity(),
                    crate::rustc_semantic_adapter_v1::rustc_type_identity_v1(tcx, ty)
                );
            }
            for (record, old) in tagged.into_iter().zip(ordinary) {
                assert_eq!(
                    record.rust_type_kind(),
                    if record.identity() == root_identity {
                        kind
                    } else {
                        SemanticRustTypeKindV1::Ordinary
                    }
                );
                assert_eq!(
                    record.with_rust_type_kind(SemanticRustTypeKindV1::Ordinary),
                    old
                );
            }
        }
        for ty in &inputs[2..] {
            assert!(authenticated_shape(tcx, *ty).unwrap().is_none());
        }
        self.done = true;
        Compilation::Stop
    }
}
#[test]
fn atomic_v41_observes_actual_core_chain_and_refuses_local_lookalike() {
    let directory = TestTempDir::create("fe2o3-atomic-nominal-v41");
    let source = directory.path().join("fixture.rs");
    std::fs::write(&source, SOURCE).unwrap();
    let output = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let sysroot = String::from_utf8(output.stdout).unwrap();
    let args = vec![
        "rustc".into(),
        "--crate-name".into(),
        "fe2o3_atomic_nominal_v41".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Zmir-opt-level=0".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = Observe::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(callbacks.done);
}
