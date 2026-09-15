//! Live AMD core/user-generic source, canonical import, actual SSA and replay.
use crate::collector::collect_authenticated_kernel_closure_v1;
use crate::production_target_v1::RetainedProductionTargetV1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, rustc_block_identity_v1, rustc_mir_body_sha256_v1,
};
use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
use crate::trusted_device_items::authenticate_reviewed_safe_core_option_zip_helper_v1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaErrorV1,
    ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1, SemanticPartialMoveViolationV1,
    plan_semantic_function_ssa_with_module_v1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::{
    mir::{Operand, TerminatorKind},
    ty::{Instance, TyCtxt, TyKind, TypingEnv},
};
use rustc_session::config::Input;
use rustc_span::FileName;

#[path = "import_tests/harness.rs"]
mod harness;
#[path = "import_tests/mutations.rs"]
mod mutations;

#[derive(Clone, Copy, Debug)]
enum Case {
    Core942,
    Core950,
    NonCopy,
    UserEnum,
    Niche,
    Mutations,
}

impl Case {
    fn profile(self) -> &'static str {
        if matches!(self, Self::Core950) {
            "gfx950"
        } else {
            "gfx942"
        }
    }
    fn source(self) -> String {
        let helper = match self {
            Self::Core942 | Self::Core950 | Self::Mutations => {
                "pub fn probe(a: usize, b: usize) { let _result = Some(a).zip(Some(b)); }"
            }
            Self::NonCopy => {
                "pub fn probe(a: usize, b: usize) { let _result = Some(NonCopy(a)).zip(Some(NonCopy(b))); }"
            }
            Self::UserEnum => {
                "pub fn probe(a: usize, b: usize) { let _result = combine(Parcel::Present(NonCopy(a)), Parcel::Present(NonCopy(b))); }"
            }
            Self::Niche => {
                "pub fn probe(a: &usize, b: &usize) { let _result = Some(a).zip(Some(b)); }"
            }
        };
        let call = if matches!(self, Self::Niche) {
            "probe(&lhs, &rhs)"
        } else {
            "probe(lhs, rhs)"
        };
        format!(
            r#"#![no_std]
use fe2o3_device::{{KernelContext, kernel}};
pub struct NonCopy(pub usize);
pub enum Parcel<T> {{ Missing, Present(T) }}
pub fn combine<T>(a: Parcel<T>, b: Parcel<T>) -> Parcel<(T, T)> {{
    match (a, b) {{
        (Parcel::Present(x), Parcel::Present(y)) => Parcel::Present((x, y)),
        _ => Parcel::Missing,
    }}
}}
{helper}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn discriminant_import_source(context: KernelContext<'_>, lhs: usize, rhs: usize) {{
    let _ = context;
    {call};
}}
"#
        )
    }

    fn test_name(self) -> &'static str {
        match self {
            Self::Core942 => "option_zip_actual_amd_discriminant_ssa_gfx942",
            Self::Core950 => "option_zip_actual_amd_discriminant_ssa_gfx950",
            Self::NonCopy => "option_zip_actual_amd_noncopy_discriminant_ssa",
            Self::UserEnum => "user_enum_actual_amd_discriminant_ssa",
            Self::Niche => "option_zip_actual_amd_niche_payload_remains_rejected",
            Self::Mutations => "option_zip_actual_amd_source_mutations_preserve_move_gates",
        }
    }
}

fn callee<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Instance<'tcx> {
    let Operand::Constant(constant) = operand else {
        panic!("constant callee")
    };
    let TyKind::FnDef(definition, args) = constant.const_.ty().kind() else {
        panic!("direct call")
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
        .unwrap()
        .expect("resolved direct call")
}

/// Locate the retained CFG pattern, not a rustc/canonical block or local index.
/// This is test evidence selection only, never an acceptance predicate.
fn cleanup_site(function: &SemanticFunctionDeclV1) -> (usize, usize, SemanticPlaceV1) {
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                continue;
            };
            if place.projections().is_empty() {
                continue;
            }
            let follows_payload_move = function.blocks().iter().any(|predecessor| {
                if !matches!(predecessor.terminator().kind(), SemanticTerminatorKindV1::Goto(edge)
                    if edge.target().index() as usize == block_index)
                {
                    return false;
                }
                predecessor.statements().iter().any(|statement| {
                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                        return false;
                    };
                    let mut found = false;
                    assignment
                        .value()
                        .kind()
                        .try_visit_operands::<()>(|operand| {
                            if let SemanticOperandV1::Move(moved) = operand {
                                found |= moved.local() == place.local()
                                    && moved.projections().len() > place.projections().len()
                                    && moved.projections().starts_with(place.projections())
                                    && moved.projections().iter().any(|projection| {
                                        matches!(
                                            projection.kind(),
                                            SemanticProjectionKindV1::Downcast(_)
                                        )
                                    });
                            }
                            Ok(())
                        })
                        .unwrap();
                    found
                })
            });
            if follows_payload_move {
                return (block_index, statement_index, place.clone());
            }
        }
    }
    panic!("original generic source must retain a discriminant read reached after a payload move")
}

struct Probe {
    case: Case,
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("discriminant_import_source.rs".into()),
            input: self.case.source(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
        let probe = Instance::mono(
            tcx,
            tcx.iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == "probe"
                })
                .unwrap()
                .to_def_id(),
        );
        let original = tcx
            .instance_mir(probe.def)
            .basic_blocks
            .iter()
            .find_map(|block| {
                let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                    return None;
                };
                let instance = callee(tcx, func);
                let selected = if matches!(self.case, Case::UserEnum) {
                    instance.def_id().is_local()
                        && tcx.item_name(instance.def_id()).as_str() == "combine"
                } else {
                    authenticate_reviewed_safe_core_option_zip_helper_v1(tcx, instance)
                };
                selected.then_some(instance)
            })
            .expect("actual core zip or source-defined generic enum helper");
        let original_body = tcx.instance_mir(original.def);
        let original_hash = rustc_mir_body_sha256_v1(tcx, original);
        let selected = crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, original);
        assert!(!selected.is_source_expansion());
        assert!(std::ptr::eq(selected.body(), original_body));
        assert!(crate::production_semantic_terminal_v1::classify(tcx, original.def_id()).is_none());

        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure =
            collect_authenticated_kernel_closure_v1(tcx, partitions.codegen_units, false, target)
                .expect("actual source collection, without terminal substitution");
        assert_eq!(
            closure
                .exact_codegen_function_symbols_v1()
                .filter(|(instance, _, _)| *instance == original)
                .count(),
            1
        );
        let imported = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("actual complete canonical source import");
        let mir = &imported.semantic_mir;
        mir.require_complete_external_entries().unwrap();
        let identity = canonical_function_identities_v1(tcx, original).function();
        let function_index = mir
            .functions()
            .iter()
            .position(|function| function.identity() == identity)
            .unwrap();
        let function_id = SemanticFunctionIdV1::from_index(function_index as u32);
        let function = &mir.functions()[function_index];
        assert_eq!(function.blocks().len(), original_body.basic_blocks.len());
        for raw in 0..original_body.basic_blocks.len() {
            assert_eq!(
                function
                    .blocks()
                    .iter()
                    .filter(|block| block.identity()
                        == rustc_block_identity_v1(identity, original_hash, raw as u32))
                    .count(),
                1
            );
        }
        assert_eq!(
            mir.callables()
                .iter()
                .filter(|callable| matches!(callable,
            SemanticCallableDeclV1::Defined { function } if *function == function_id))
                .count(),
            1
        );
        let (block, statement, place) = cleanup_site(function);
        let SemanticRustcVariantsV1::Multiple(layout) =
            mir.types()[place.ty().index() as usize].layout().variants()
        else {
            panic!("exact retained enum layout")
        };
        let isolated = plan_semantic_function_ssa_with_module_v1(
            function_id,
            function,
            mir.types(),
            mir.callables(),
            ProductionSemanticSsaLimitsV1::default(),
        );
        if matches!(self.case, Case::Niche) {
            assert!(matches!(
                layout.encoding(),
                SemanticEnumEncodingV1::Niche(_)
            ));
            assert!(
                matches!(isolated, Err(ProductionSemanticSsaErrorV1::PartialMove {
                function, block: found_block, statement: Some(found_statement), local,
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            }) if function == function_id && found_block as usize == block
                && found_statement as usize == statement && local == place.local().index())
            );
        } else {
            assert!(
                matches!(layout.encoding(), SemanticEnumEncodingV1::Direct(tag)
                if matches!(tag.tag(), SemanticBackendScalarV1::Initialized { .. }))
            );
            assert!(
                isolated
                    .expect("original direct-tag helper must reach SSA")
                    .partial_move_certificate()
                    .projected_moves()
                    >= 2
            );
            if matches!(self.case, Case::Mutations) {
                mutations::check(mir, function_id, block, statement, &place);
            }
            let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
                mir.canonical_encoding(),
                SemanticMirLimitsV1::default(),
            )
            .unwrap();
            assert_eq!(decoded.functions(), mir.functions());
            assert_eq!(decoded.callables(), mir.callables());
            let owner = ProductionSemanticSsaOwnerV1::try_new(
                ProductionSemanticMirOwnerV1::try_new(
                    decoded,
                    ProductionSemanticMirLimitsV1::default(),
                )
                .unwrap(),
                ProductionSemanticSsaLimitsV1::default(),
            )
            .expect("source AND checked call-expanded SSA, not collection-only qualification");
            owner.verify_replay().unwrap();
            assert_eq!(
                owner.source_semantic().canonical_encoding(),
                mir.canonical_encoding()
            );
            assert_eq!(
                owner.source_semantic().functions()[function_index],
                *function
            );
            assert_eq!(
                owner
                    .plan_for_function(function_id)
                    .unwrap()
                    .function_identity(),
                identity
            );
        }
        assert_eq!(rustc_mir_body_sha256_v1(tcx, original), original_hash);
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires existing AMDGPU device/core metadata; no build fallback"]
fn option_zip_actual_amd_discriminant_ssa_gfx942() {
    harness::run(Case::Core942);
}
#[test]
#[ignore = "requires existing AMDGPU device/core metadata; no build fallback"]
fn option_zip_actual_amd_discriminant_ssa_gfx950() {
    harness::run(Case::Core950);
}
#[test]
#[ignore = "requires existing AMDGPU device/core metadata; no build fallback"]
fn option_zip_actual_amd_noncopy_discriminant_ssa() {
    harness::run(Case::NonCopy);
}
#[test]
#[ignore = "requires existing AMDGPU device/core metadata; no build fallback"]
fn user_enum_actual_amd_discriminant_ssa() {
    harness::run(Case::UserEnum);
}
#[test]
#[ignore = "requires existing AMDGPU device/core metadata; no build fallback"]
fn option_zip_actual_amd_niche_payload_remains_rejected() {
    harness::run(Case::Niche);
}
#[test]
#[ignore = "requires existing AMDGPU device/core metadata; no build fallback"]
fn option_zip_actual_amd_source_mutations_preserve_move_gates() {
    harness::run(Case::Mutations);
}
