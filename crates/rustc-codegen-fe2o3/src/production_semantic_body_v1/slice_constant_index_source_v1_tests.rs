use super::*;
use crate::collector::CollectedFunctionRole;
use crate::production_rustc_driver_checked_output_source_helpers_v1_tests::amd_source_dependencies_v1;
use crate::production_semantic_fn_abi_v1::construct_production_semantic_fn_abis_v1;
use crate::production_semantic_types_v1::construct_production_semantic_types_v1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, canonical_target_layout_v1,
};
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, RetainedSemanticFunctionProducerV1,
    build_production_semantic_preflight_plan_v1,
};
use crate::semantic_layout_bridge::rustc_semantic_layout_target_v1;
use crate::test_temp_dir::TestTempDir;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use std::path::Path;

const SOURCE: &str = r#"#![no_std]
pub fn prefix<'a>(values: &'a [u64], fallback: &'a u64) -> &'a u64 {
    match values { [head, ..] => head, [] => fallback }
}
pub fn suffix<'a>(values: &'a [u64], fallback: &'a u64) -> &'a u64 {
    match values { [.., last] => last, [] => fallback }
}
pub fn mixed<'a>(values: &'a [u64], fallback: &'a u64) -> (&'a u64, &'a u64, &'a u64, &'a u64) {
    match values {
        [first, second, .., penultimate, last] => (first, second, penultimate, last),
        _ => (fallback, fallback, fallback, fallback),
    }
}
"#;
const ROOTS: &[(&str, &[(u64, u64, bool)])] = &[
    ("prefix", &[(0, 1, false)]),
    ("suffix", &[(1, 1, true)]),
    (
        "mixed",
        &[(0, 4, false), (1, 4, false), (2, 4, true), (1, 4, true)],
    ),
];

struct ConstructionCallbacks {
    profile: ProductionAmdTargetProfileV1,
    completed: bool,
}

impl Callbacks for ConstructionCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(
            tcx.sess.opts.cg.target_cpu.as_deref(),
            Some(self.profile.cpu())
        );
        assert_eq!(tcx.sess.target.arch.as_str(), "amdgcn");
        let roots = ROOTS
            .iter()
            .map(|(name, _)| {
                let definition = tcx
                    .iter_local_def_id()
                    .find(|definition| {
                        tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                            && tcx.item_name(definition.to_def_id()).as_str() == *name
                    })
                    .unwrap();
                Instance::mono(tcx, definition.to_def_id())
            })
            .collect::<Vec<_>>();
        let mut functions = roots
            .iter()
            .map(|instance| RetainedSemanticFunctionProducerV1 {
                identities: canonical_function_identities_v1(tcx, *instance),
                instance: *instance,
                role: CollectedFunctionRole::InternalHelper,
                export_name: None,
                kernel_binding: None,
                generated_host_contract_identity: None,
                frontend_contract: None,
            })
            .collect::<Vec<_>>();
        functions.sort_by_key(|row| row.identities.function());
        let root_ids = (0..functions.len())
            .map(|i| SemanticFunctionIdV1::from_index(i as u32))
            .collect::<Vec<_>>();
        let mut plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(&rustc_semantic_layout_target_v1(tcx).unwrap()),
            functions.into_boxed_slice(),
            root_ids.into_boxed_slice(),
            [0x6d; 32],
            DebugSourceCaptureRequestV2::Disabled,
            None,
        )
        .unwrap();
        assert!(plan.direct_call_producers().is_empty());
        assert!(plan.normalized_intrinsic_producers().is_empty());
        assert!(plan.terminal_producers().is_empty());
        let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
            .unwrap()
            .into_records();
        let abis = construct_production_semantic_fn_abis_v1(
            tcx,
            plan.function_abi_producers(),
            plan.type_producers(),
        )
        .unwrap()
        .into_records();
        let type_bindings = plan
            .type_producers()
            .iter()
            .enumerate()
            .map(|(i, row)| {
                ProductionSemanticTypeBindingV1::new(row.ty, SemanticTypeIdV1::from_index(i as u32))
            })
            .collect::<Vec<_>>();
        let owned = plan
            .function_producers()
            .iter()
            .enumerate()
            .map(|(i, row)| {
                ProductionSemanticCallableOwnerEntryV1::defined(
                    row.instance,
                    SemanticCallableIdV1::from_index(i as u32),
                )
            })
            .collect::<Vec<_>>();
        let mut owner = ProductionSemanticBodyRequestOwnerV1::with_preflight_work(
            plan.take_construction_work().unwrap(),
            types.len(),
            &owned,
        )
        .unwrap();
        let mut total = 0;
        for (index, producer) in plan.function_producers().iter().enumerate() {
            let instance = producer.instance;
            let planned = &plan.body_producers()[index];
            let raw = tcx.instance_mir(instance.def);
            let locals = planned
                .locals
                .iter()
                .map(|row| {
                    ProductionSemanticLocalBindingV1::new(
                        row.rustc_local,
                        planned.raw_to_semantic_locals[row.rustc_local as usize],
                        row.identity,
                        row.source.provenance,
                    )
                })
                .collect::<Vec<_>>();
            let blocks = planned
                .blocks
                .iter()
                .map(|row| {
                    ProductionSemanticBlockBindingV1::new(
                        row.rustc_block,
                        planned.raw_to_semantic_blocks[row.rustc_block as usize],
                        row.identity,
                        row.source.provenance,
                        row.statements
                            .iter()
                            .map(|source| source.provenance)
                            .collect(),
                        row.terminator.provenance,
                    )
                })
                .collect::<Vec<_>>();
            let identities = producer.identities;
            let produced = construct_production_semantic_body_v1(
                ProductionSemanticBodyInputV1 {
                    context_entry: None,
                    tcx,
                    instance,
                    body: raw,
                    function: SemanticFunctionIdV1::from_index(index as u32),
                    identities: ProductionSemanticFunctionIdentitiesV1::new(
                        identities.function(),
                        identities.item_definition(),
                        identities.monomorphization(),
                        identities.generic_type_arguments(),
                        identities.const_generic_arguments(),
                    ),
                    role: SemanticFunctionRoleV1::InternalHelper,
                    export: ProductionSemanticFunctionExportV1::None,
                    source: planned.source.provenance,
                    abi: abis[index].clone(),
                    type_bindings: &type_bindings,
                    local_bindings: &locals,
                    block_bindings: &blocks,
                    entry: planned.entry,
                    direct_calls: &[],
                    terminal_expansions: &[],
                    normalized_intrinsics: &[],
                },
                &mut owner,
            )
            .unwrap();
            let mut observed = Vec::new();
            for (raw_block, block) in raw.basic_blocks.iter_enumerated() {
                let semantic_block = &produced.blocks()
                    [planned.raw_to_semantic_blocks[raw_block.index()].index() as usize];
                for (ordinal, statement) in block.statements.iter().enumerate() {
                    let StatementKind::Assign(assignment) = &statement.kind else {
                        continue;
                    };
                    let Rvalue::Ref(_, borrow, place) = &assignment.1 else {
                        continue;
                    };
                    if !place
                        .projection
                        .iter()
                        .any(|p| matches!(p, ProjectionElem::ConstantIndex { .. }))
                    {
                        continue;
                    }
                    assert!(matches!(borrow, BorrowKind::Shared));
                    let emitted = &semantic_block.statements()[ordinal];
                    let SemanticStatementKindV1::Assign(assignment) = emitted.kind() else {
                        panic!("borrow assignment changed");
                    };
                    let SemanticRvalueKindV1::Borrow {
                        kind,
                        place: actual,
                    } = assignment.value().kind()
                    else {
                        panic!("indexed borrow changed");
                    };
                    assert_eq!(*kind, SemanticBorrowKindV1::Shared);
                    assert_eq!(
                        actual.local(),
                        planned.raw_to_semantic_locals[place.local.index()]
                    );
                    assert_eq!(actual.projections().len(), place.projection.len());
                    let frozen = planned
                        .blocks
                        .iter()
                        .find(|row| row.rustc_block as usize == raw_block.index())
                        .unwrap();
                    assert_eq!(emitted.source(), frozen.statements[ordinal].provenance);
                    let mut derived = PlaceTy::from_ty(raw.local_decls[place.local].ty);
                    for (raw_projection, actual_projection) in
                        place.projection.iter().zip(actual.projections())
                    {
                        let expected = match raw_projection {
                            ProjectionElem::Deref => SemanticProjectionKindV1::Dereference,
                            ProjectionElem::ConstantIndex {
                                offset,
                                min_length,
                                from_end,
                            } => {
                                assert!(matches!(derived.ty.kind(), TyKind::Slice(..)));
                                observed.push((offset, min_length, from_end));
                                SemanticProjectionKindV1::ConstantIndex {
                                    offset,
                                    minimum_length: min_length,
                                    from_end,
                                }
                            }
                            other => panic!("unexpected fixture projection: {other:?}"),
                        };
                        assert_eq!(actual_projection.kind(), expected);
                        derived = derived.projection_ty(tcx, raw_projection);
                        let ty = type_bindings
                            .iter()
                            .find(|row| row.rustc_type == derived.ty)
                            .unwrap()
                            .semantic_type;
                        assert_eq!(actual_projection.result_type(), ty);
                    }
                    assert_eq!(
                        actual.ty(),
                        actual.projections().last().unwrap().result_type()
                    );
                }
            }
            let root_index = roots.iter().position(|root| *root == instance).unwrap();
            let mut expected = ROOTS[root_index].1.to_vec();
            observed.sort();
            expected.sort();
            assert_eq!(
                observed, expected,
                "{} {:?}",
                ROOTS[root_index].0, self.profile
            );
            total += observed.len();
        }
        assert_eq!(total, 6);
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and actual AMD frontend callbacks; no GPU execution"]
fn genuine_slice_constant_indices_preserve_retained_mir() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let scratch = TestTempDir::create("fe2o3-slice-constant-index-source");
        let target = scratch.path().join("target");
        let source = scratch.path().join("fixture.rs");
        std::fs::write(&source, SOURCE).unwrap();
        let dependencies = amd_source_dependencies_v1(&workspace, &target, profile);
        for opt in [0, 3] {
            assert_eq!(std::fs::read_to_string(&source).unwrap(), SOURCE);
            let args = vec![
                dependencies.rustc.to_str().unwrap().to_owned(),
                "--crate-name=fe2o3_slice_constant_index_source".into(),
                "--crate-type=lib".into(),
                "--edition=2024".into(),
                "--target=amdgcn-amd-amdhsa".into(),
                format!("-Ctarget-cpu={}", profile.cpu()),
                "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32".into(),
                format!("-Copt-level={opt}"),
                format!("-Zmir-opt-level={}", if opt == 0 { 0 } else { 2 }),
                "-Zinline-mir=no".into(),
                "-Zalways-encode-mir".into(),
                "-Zunstable-options".into(),
                "-Cpanic=abort".into(),
                "--emit=metadata".into(),
                "--sysroot".into(),
                dependencies.sysroot.trim().into(),
                format!("--extern=noprelude:core={}", dependencies.core.display()),
                format!(
                    "--extern=noprelude:compiler_builtins={}",
                    dependencies.builtins.display()
                ),
                format!(
                    "-Ldependency={}",
                    dependencies.device.parent().unwrap().display()
                ),
                format!("-Ldependency={}", target.join("release/deps").display()),
                "-o".into(),
                scratch
                    .path()
                    .join(format!("fixture-opt{opt}.rmeta"))
                    .display()
                    .to_string(),
                source.display().to_string(),
            ];
            let mut callbacks = ConstructionCallbacks {
                profile,
                completed: false,
            };
            rustc_driver::run_compiler(&args, &mut callbacks);
            assert!(
                callbacks.completed,
                "{} opt{opt}: callback did not finish",
                profile.cpu()
            );
            assert_eq!(std::fs::read_to_string(&source).unwrap(), SOURCE);
        }
    }
}
