//! Registered source controls for the shared V24 boundary. No KIR qualification.
use super::*;
use crate::collector::production_importer_v1::reusable_lds_v1;
use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[derive(Clone, Copy, Debug)]
pub(super) struct Case {
    fp8: bool,
    reusable: bool,
    transpose: bool,
}

pub(super) fn source(case: Case) -> String {
    let mut source = String::from(
        r#"
#![no_std]
use fe2o3_device::{Global, ReadOnly, KernelContext, StrictIeee, SubgroupWidth64, Gfx950Fp4E2M1, kernel};
#[kernel(typed, launch(required = [64,1,1], max = [64,1,1], max_grid = [1,1,1]))]
pub fn combined_v24(mut context: KernelContext<'_>, input: Global<'_, u8, ReadOnly>, enabled: u32) {
    if enabled == 0 { return; }
"#,
    );
    if case.transpose {
        source.push_str("    let policy = context.numerical_policy::<StrictIeee>();\n");
    }
    source.push_str("    context.with_workgroup(|workgroup| {\n");
    if case.reusable {
        source.push_str(
            "        let _reusable = workgroup.allocate_lds::<f32, 64>().into_reusable();\n",
        );
    }
    if case.transpose {
        source.push_str(
            r#"
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
            if enabled > 1 {
                let _fragment = published.read_mfma_fragment(lane);
            }
        });
"#,
        );
    }
    source.push_str("    });\n}\n");
    if case.fp8 {
        source = source
            .replace("Gfx950Fp4E2M1", "Gfx950Fp8E4M3")
            .replace("fp4_a_global_row_major", "fp8_a_global_row_major");
    }
    source
}

fn request(mir: &AdmittedInertSemanticMirV1) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        mir.functions().to_vec(),
        mir.callables().to_vec(),
        mir.roots().to_vec(),
    )
    .unwrap()
}

fn transpose(callable: &SemanticCallableDeclV1) -> Option<SemanticExecutionCapabilityContractV1> {
    match callable {
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } if matches!(
            contract.operation(),
            SemanticExecutionCapabilityOperationV1::Gfx950Transpose(_)
        ) =>
        {
            assert_eq!(binding.identity(), contract.source_identity());
            Some(*contract)
        }
        _ => None,
    }
}

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    closure: crate::collector::AuthenticatedCollectedKernelClosureV1<'tcx>,
    case: Case,
) {
    if case.transpose {
        super::gfx950_transpose::assert_runtime_read_closure(&closure);
    }
    use crate::production_target_v1::RetainedProductionTargetV1;
    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    assert_eq!(target.contract().cpu(), "gfx950");
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
    .expect("second live observation includes the original closure instances");
    crate::rustc_semantic_plan_v1::lds_type_dependencies_v1::assert_live_roster(tcx, &plan);
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .unwrap_or_else(|error| panic!("{case:?}: registered combined source import: {error}"));
    assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
    assert_eq!(
        imported.rustc_preflight_plan.canonical_transcript(),
        plan.canonical_transcript()
    );
    let mir = &imported.semantic_mir;
    mir.require_complete_external_entries().unwrap();
    let [root] = mir.roots() else {
        panic!("one registered root")
    };
    let root = *root;
    let converters = mir
        .functions()
        .iter()
        .filter_map(|function| match function.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record)) => {
                Some(*record)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let transposes = mir
        .callables()
        .iter()
        .filter_map(transpose)
        .collect::<Vec<_>>();
    assert_eq!(converters.len(), usize::from(case.reusable), "{case:?}");
    assert_eq!(
        transposes.len(),
        if case.transpose { 4 } else { 0 },
        "{case:?}"
    );
    assert_eq!(mir.callables().iter().filter(|callable| matches!(callable,
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
        } if matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::LdsAllocate { .. })
    )).count(), usize::from(case.reusable), "transpose is not a fabricated LdsAllocate");
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
        "no legacy singleton transpose substitution"
    );
    for record in &converters {
        assert_eq!(record.provenance().root(), root);
        assert_eq!(
            (
                record.elements(),
                record.element_size(),
                record.element_align()
            ),
            (64, 4, 4)
        );
        assert_ne!(record.types().input, record.types().output);
        if case.transpose {
            let issuer = transposes
                .iter()
                .find(|contract| {
                    matches!(contract.operation(),
                        SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t)
                        if matches!(t.operation(), SemanticGfx950TransposeOperationV1::Issue { .. })
                    )
                })
                .expect("the same original Workgroup also issues the transpose tile");
            assert_eq!(record.provenance(), issuer.provenance());
            assert_eq!(Some(record.brand()), issuer.workgroup_brand());
            assert_eq!(Some(record.epoch()), issuer.epoch_before());
        }
    }
    for record in &transposes {
        let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t) = record.operation() else {
            unreachable!()
        };
        assert_eq!(record.provenance().root(), root);
        assert_eq!(
            t.format(),
            if case.fp8 {
                SemanticGfx950LdsTransposeFormatV1::Fp8E4M3
            } else {
                SemanticGfx950LdsTransposeFormatV1::Fp4E2M1
            }
        );
        assert_eq!(record.obligations().bits(), t.obligations());
        assert_eq!(record.epoch_after().is_some(), t.advances_epoch());
    }

    // Controls remove source operations before collection, never their receipts
    // after import. Either retained family must independently force V24.
    let current = request(mir)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    assert_eq!(current.wire_version(), mir.wire_version());
    assert_eq!(current.canonical_encoding(), mir.canonical_encoding());
    if case.reusable || case.transpose {
        assert_eq!(
            mir.wire_version(),
            SemanticMirWireVersionV1::V24,
            "{case:?}"
        );
        assert!(
            matches!(
                request(mir).admit_exact_v23(SemanticMirLimitsV1::default()),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    required: SemanticMirWireVersionV1::V24,
                    ..
                })
            ),
            "{case:?}: either family must require V24"
        );
    } else {
        assert!(mir.wire_version() < SemanticMirWireVersionV1::V24);
        request(mir)
            .admit_exact_v23(SemanticMirLimitsV1::default())
            .unwrap();
    }
    let exact = request(mir)
        .admit_exact_v24(SemanticMirLimitsV1::default())
        .unwrap();
    for decoded in [
        AdmittedInertSemanticMirV1::decode_exact_v24_canonical(
            exact.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        ),
        AdmittedInertSemanticMirV1::decode_current_production_canonical(
            exact.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        ),
    ] {
        let decoded = decoded.unwrap();
        assert_eq!(decoded.canonical_encoding(), exact.canonical_encoding());
        assert_eq!(decoded.functions(), mir.functions());
        assert_eq!(decoded.callables(), mir.callables());
        assert_eq!(decoded.types(), mir.types());
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        reusable_lds_v1::validate_carriage(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
    }
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
        .unwrap();
    reusable_lds_v1::validate_carriage(tcx, &plan, &imported.kernel_contexts, &decoded).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(decoded, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap_or_else(|error| {
        panic!("{case:?}: original source custody must construct SSA: {error:?}")
    });
    owner.verify_replay().unwrap();
    let retained_source_plan = &plan;
    let plan = owner.execution_plan_for_root(root).unwrap();
    let receipts = plan.defined_reusable_lds_results();
    assert_eq!(receipts.len(), converters.len());
    for (receipt, record) in receipts.iter().zip(&converters) {
        assert_eq!(receipt.record(), *record);
        assert_ne!(receipt.allocation(), receipt.parameter());
        assert_ne!(receipt.parameter(), receipt.return_local());
    }
    let view = owner.execution_view_for_root(root).unwrap();
    let mut calls = BTreeSet::new();
    for (block_index, block) in view.body().blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(record) = transpose(&mir.callables()[call.callee().index() as usize]) else {
            continue;
        };
        assert!(calls.insert(record.source_identity()));
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
            "result must be defined by its actual normal call-return edge"
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
                super::gfx950_transpose::custody_diagnostic::failure(tcx, retained_source_plan, &owner, root, block_index, t.operation(), 0)
            );
        }
        if matches!(
            t.operation(),
            SemanticGfx950TransposeOperationV1::Publish { .. }
        ) {
            assert!(
                matches!(call.arguments()[1], SemanticOperandV1::Move(_)),
                "{}",
                super::gfx950_transpose::custody_diagnostic::failure(tcx, retained_source_plan, &owner, root, block_index, t.operation(), 1)
            );
        }
    }
    assert_eq!(calls.len(), transposes.len());
}

macro_rules! registered_case {
    ($name:ident, $fp8:expr, $reusable:expr, $transpose:expr) => {
        #[test]
        #[ignore = "requires cached authenticated AMD metadata; no Cargo or KIR qualification"]
        fn $name() {
            run_registered_source(
                "gfx950",
                concat!("combined_v24::", stringify!($name)),
                true,
                false,
                SourceCase::CombinedV24(Case {
                    fp8: $fp8,
                    reusable: $reusable,
                    transpose: $transpose,
                }),
            );
        }
    };
}
registered_case!(combined_v24_fp4_both, false, true, true);
registered_case!(combined_v24_fp8_both, true, true, true);
registered_case!(combined_v24_fp4_without_reusable, false, false, true);
registered_case!(combined_v24_fp8_without_reusable, true, false, true);
registered_case!(combined_v24_without_transpose, false, true, false);
registered_case!(combined_v24_without_either, false, false, false);
