//! Actual registered AMD source -> replayed canonical MIR -> checked SSA.
//! This is not a physical LDS/barrier/KIR or launch qualification test.
use super::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

include!("gfx950_transpose/runtime_closure_assertion.rs");
include!("gfx950_transpose/operand_diagnostic.rs");
include!("gfx950_transpose/owned_source_unit.rs");
#[path = "gfx950_transpose/custody_diagnostic.rs"]
pub(super) mod custody_diagnostic;

pub(super) fn source(fp8: bool) -> String {
    let source = r#"
#![no_std]
use fe2o3_device::{Global, ReadOnly, KernelContext, StrictIeee, SubgroupWidth64, Gfx950Fp4E2M1, kernel};
#[kernel(typed, launch(required = [64,1,1], max = [64,1,1], max_grid = [1,1,1]))]
pub fn transpose_source(mut context: KernelContext<'_>, input: Global<'_, u8, ReadOnly>, read_enabled: u32) {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let partition = subgroup.gfx950_wave16(workgroup.epoch());
        let tile = partition.transpose_tile::<Gfx950Fp4E2M1>();
        let staged = subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let bound = matrix.with_numerical_policy(&policy);
            let matrix = bound.gfx950();
            let Ok(view) = matrix.fp4_a_global_row_major(&input, 0, 16, 128, 128) else {
                fe2o3_device::trap();
            };
            tile.stage_k_transposed(&view, 0, 0)
        });
        let (workgroup, published) = staged.publish(workgroup);
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), move |_matrix, lane| {
            if read_enabled != 0 {
                let _fragment = published.read_mfma_fragment(lane);
            }
        });
    });
}
"#;
    if fp8 {
        source
            .replace("Gfx950Fp4E2M1", "Gfx950Fp8E4M3")
            .replace("fp4_a_global_row_major", "fp8_a_global_row_major")
    } else {
        source.into()
    }
}

fn with_callables(
    mir: &AdmittedInertSemanticMirV1,
    callables: Vec<SemanticCallableDeclV1>,
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        mir.functions().to_vec(),
        callables,
        mir.roots().to_vec(),
    )?
    .admit_exact_v24(SemanticMirLimitsV1::default())
}

fn transpose(callable: &SemanticCallableDeclV1) -> Option<SemanticExecutionCapabilityContractV1> {
    match callable {
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } if matches!(
            contract.operation(),
            SemanticExecutionCapabilityOperationV1::Gfx950Transpose(_)
        ) =>
        {
            Some(*contract)
        }
        _ => None,
    }
}

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
    fp8: bool,
) {
    check_mode(tcx, closure, fp8, false);
}

fn check_mode<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
    fp8: bool,
    source_unit: bool,
) {
    assert_runtime_read_closure(&closure);
    use crate::production_target_v1::RetainedProductionTargetV1;
    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    let inventory =
        build_identity_inventory_v1(tcx, &target, &closure.collection, &closure.roots).unwrap();
    let closure_types = closure
        .collection
        .functions
        .iter()
        .filter_map(|function| function.closure_plan.as_ref())
        .flat_map(|plan| plan.authenticated_closure_type_identities())
        .map(SemanticTypeIdentityV1::from_sha256)
        .collect::<BTreeSet<_>>();
    let plan = build_production_semantic_preflight_plan_v1(
        tcx,
        canonical_target_layout_v1(target.rustc_layout()),
        inventory.functions,
        inventory.roots,
        inventory.sha256,
        &closure_types,
        DebugSourceCaptureRequestV2::Disabled,
    )
    .expect("retain the live source plan, including the complete closure roster");
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .expect("complete authenticated source-shaped transpose transaction import");
    assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
    assert_eq!(
        imported.rustc_preflight_plan.canonical_transcript(),
        plan.canonical_transcript()
    );
    let mir = &imported.semantic_mir;
    assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V25);
    assert_eq!(mir.transpose_owned_flows().len(), 1, "complete live source footer");
    let [root] = mir.roots() else {
        panic!("one actual registered source root")
    };
    let records = mir
        .callables()
        .iter()
        .filter_map(transpose)
        .collect::<Vec<_>>();
    assert_eq!(
        records.len(),
        4,
        "issue, stage, publish and read must all be real called terminals"
    );
    assert!(
        !mir.callables().iter().any(|callable| matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeCurrent { .. }
                    | SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage { .. }
                    | SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish { .. }
                    | SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeRead { .. },
                ..
            }
        )),
        "no fallback to the historical singleton contract"
    );
    let expected_format = if fp8 {
        SemanticGfx950LdsTransposeFormatV1::Fp8E4M3
    } else {
        SemanticGfx950LdsTransposeFormatV1::Fp4E2M1
    };
    for record in &records {
        let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t) = record.operation() else {
            unreachable!()
        };
        assert_eq!(t.format(), expected_format);
        assert_eq!(record.provenance().root(), *root);
        assert_eq!(record.obligations().bits(), t.obligations());
        assert_eq!(record.epoch_after().is_some(), t.advances_epoch());
    }
    assert!(AdmittedInertSemanticMirV1::decode_exact_v24_canonical(
        mir.canonical_encoding(), SemanticMirLimitsV1::default(),
    ).is_err(), "live V25 custody must not be accepted as historical V24");
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v25_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
    assert_eq!(decoded.callables(), mir.callables());
    assert_eq!(decoded.functions(), mir.functions());
    assert_eq!(decoded.transpose_owned_flows(), mir.transpose_owned_flows());
    validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
        .unwrap();
    let current = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(current.canonical_encoding(), mir.canonical_encoding());

    // A self-consistent nominal replacement may be inert, but is not original
    // source custody. Rebuild every phase so rejection cannot be a roster typo.
    let mut changed = mir.callables().to_vec();
    let mut changed_count = 0;
    for callable in &mut changed {
        let Some(record) = transpose(callable) else {
            continue;
        };
        let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t) = record.operation() else {
            unreachable!()
        };
        let after = matches!(
            t.operation(),
            SemanticGfx950TransposeOperationV1::Read { .. }
        );
        let changed = SemanticGfx950TransposeContractV1::new(
            t.operation(),
            t.format(),
            SemanticTypeIdentityV1::from_sha256([if after { 91 } else { 90 }; 32]),
            t.advances_epoch()
                .then_some(SemanticTypeIdentityV1::from_sha256([91; 32])),
        )
        .unwrap();
        let changed = SemanticExecutionCapabilityContractV1::new(
            SemanticExecutionCapabilityOperationV1::Gfx950Transpose(changed),
            record.signature(),
            record.provenance(),
            record.workgroup_brand().unwrap(),
            record.epoch_before().unwrap(),
            record.epoch_after(),
            record.source_identity(),
        )
        .unwrap();
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable else {
            unreachable!()
        };
        *operation =
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract: changed };
        changed_count += 1;
    }
    assert_eq!(changed_count, 4);
    let changed =
        with_callables(mir, changed).expect("consistent inert nominal substitution fixture");
    let error =
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &changed)
            .expect_err("all four changed nominal records still lack the original source receipt")
            .to_string();
    assert!(
        error.contains("execution terminal complete source contract carriage"),
        "{error}"
    );

    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .expect("real transpose call results and reference/move flows must construct SSA");
    owner.verify_replay().unwrap();
    if source_unit {
        crate::collector::production_importer_v1::transpose_owned_source_v1::tests::check(
            tcx, &plan, &imported.kernel_contexts, &owner,
        );
        return;
    }
    let view = owner.execution_view_for_root(*root).unwrap();
    let retained_source_plan = &plan;
    let plan = owner.execution_plan_for_root(*root).unwrap();
    let mut actual = BTreeSet::new();
    for (block_index, block) in view.body().blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(record) = transpose(&mir.callables()[call.callee().index() as usize]) else {
            continue;
        };
        assert!(actual.insert(record.source_identity()));
        assert!(
            call.arguments()
                .iter()
                .map(SemanticOperandV1::ty)
                .eq(record.signature().arguments())
        );
        let destination = call.destination().unwrap();
        assert_eq!(destination.place().ty(), record.signature().output());
        assert!(destination.place().projections().is_empty());
        let local = destination.place().local();
        assert!(
            !plan
                .implicit_entry_variables()
                .iter()
                .any(|v| v.get() == local.index())
        );
        assert!(
            !plan
                .frame_initializations()
                .iter()
                .any(|row| row.local() == local)
        );
        let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(block_index as u32), 0);
        assert!(
            plan.plan()
                .edge_definitions(edge)
                .unwrap()
                .iter()
                .any(|v| v.variable().get() == local.index()),
            "transpose result must be defined by this call-return edge, not by its zero-sized type"
        );
        let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t) = record.operation() else {
            unreachable!()
        };
        if !matches!(
            t.operation(),
            SemanticGfx950TransposeOperationV1::Issue { .. }
        ) {
            assert!(
                matches!(call.arguments()[0], SemanticOperandV1::Move(_)),
                "{}",
                custody_diagnostic::failure(tcx, retained_source_plan, &owner, *root, block_index, t.operation(), 0)
            );
        }
        if matches!(
            t.operation(),
            SemanticGfx950TransposeOperationV1::Publish { .. }
        ) {
            assert!(
                matches!(call.arguments()[1], SemanticOperandV1::Move(_)),
                "{}",
                custody_diagnostic::failure(tcx, retained_source_plan, &owner, *root, block_index, t.operation(), 1)
            );
        }
    }
    assert_eq!(actual.len(), 4);
}

#[test]
#[ignore = "requires cached actual AMD metadata; no Cargo, KIR or launch proof"]
fn transpose_fp4_source_ssa_gfx950_v24() {
    run_registered_source(
        "gfx950",
        "gfx950_transpose::transpose_fp4_source_ssa_gfx950_v24",
        true,
        false,
        SourceCase::TransposeFp4,
    );
}

#[test]
#[ignore = "requires cached actual AMD metadata; no Cargo, KIR or launch proof"]
fn transpose_fp8_source_ssa_gfx950_v24() {
    run_registered_source(
        "gfx950",
        "gfx950_transpose::transpose_fp8_source_ssa_gfx950_v24",
        true,
        false,
        SourceCase::TransposeFp8,
    );
}
