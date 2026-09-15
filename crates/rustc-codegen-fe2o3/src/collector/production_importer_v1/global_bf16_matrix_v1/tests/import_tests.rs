//! Actual registered AMD source through canonical import and replayed SSA.
//! These checks are not a ranked memory-refinement or launch receipt.

use super::*;
use crate::production_target_v1::RetainedProductionTargetV1;
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, build_production_semantic_preflight_plan_v1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticTerminatorKindV1,
};

mod layout_identity;

pub(super) const SOURCE: &str = r#"
#![no_std]
#![allow(deprecated)]
use fe2o3_device::{Global, KernelContext, KernelResult, ReadOnly, StrictIeee,
    SubgroupWidth64, kernel};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn global_bf16_reads(context: KernelContext<'_>, a: Global<'_, u16, ReadOnly>,
    b: Global<'_, u16, ReadOnly>, n: u32) -> KernelResult {
    let policy = context.numerical_policy::<StrictIeee>();
    let lane = context.subgroup_lane::<SubgroupWidth64>();
    let matrix = context.matrix();
    let matrix = matrix.with_numerical_policy(&policy);
    let a_view = matrix.bf16_a_global_row_major(&a, 0, n as usize, n as usize, n as usize)?;
    let b_view = matrix.bf16_b_global_row_major(&b, 0, n as usize, n as usize, n as usize)?;
    let _a = a_view.load_m16k16(&lane, 0, 0);
    let _b = b_view.load_k16n16(&lane, 0, 0);
    Ok(())
}
"#;

pub(super) fn check(tcx: TyCtxt<'_>, cpu: &str, check_ssa: bool) {
    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .expect("use the actual AMD session, not a relabelled host target");
    let partitions = tcx.collect_and_partition_mono_items(());
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .expect("collect the registered kernel, actual Global bind and checked matrix constructors");
    let observed_target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    let inventory =
        build_identity_inventory_v1(tcx, &observed_target, &closure.collection, &closure.roots)
            .unwrap();
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
        canonical_target_layout_v1(observed_target.rustc_layout()),
        inventory.functions,
        inventory.roots,
        inventory.sha256,
        &closure_types,
        DebugSourceCaptureRequestV2::Disabled,
    )
    .expect("reobserve the actual source plan for substitution checks");
    assert_root_matrix_route(tcx, &plan);
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .unwrap_or_else(|error| {
                for (index, terminal) in plan.terminal_producers().iter().enumerate() {
                    if !matches!(
                        terminal.expansion,
                        ProductionTerminalExpansionV1::GlobalBf16MatrixALoadZeroFilled
                            | ProductionTerminalExpansionV1::GlobalBf16MatrixBLoadZeroFilled
                    ) {
                        continue;
                    }
                    eprintln!(
                        "BF16 import terminal {index}: {:?} {} inputs={:?} output={:?}",
                        terminal.expansion,
                        tcx.def_path_str(terminal.instance.def_id()),
                        terminal.abi.source_inputs,
                        terminal.abi.source_output,
                    );
                }
                panic!(
                    "retain the full Global BF16 source contract through production import: {error}"
                );
            });
    assert_eq!(imported.rustc_target.contract().cpu(), cpu);
    assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
    assert_eq!(
        imported.rustc_preflight_plan.canonical_transcript(),
        plan.canonical_transcript()
    );
    let mir = &imported.semantic_mir;
    layout_identity::check(&plan, mir);
    assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V23);
    assert_eq!(mir.roots().len(), 1);
    let matrix_records = mir
        .functions()
        .iter()
        .filter_map(|function| match function.defined_capability_contract() {
            Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record)) => Some(*record),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [matrix_record] = matrix_records.as_slice() else {
        panic!("the BF16 fixture must retain its single original Context.matrix getter")
    };
    let getter = &mir.functions()[matrix_record.function().index() as usize];
    assert_eq!(matrix_record.source_identity(), getter.identity());
    assert_eq!(matrix_record.abi_identity(), getter.abi().identity());
    assert_eq!(matrix_record.provenance().root(), mir.roots()[0]);
    mir.require_complete_external_entries().unwrap();
    assert!(
        AdmittedInertSemanticMirV1::decode_exact_v22_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .is_err(),
        "the combined document's MatrixDerive tag7 cannot be decoded as V22"
    );
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v23_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
    assert_eq!(decoded.functions(), mir.functions());
    assert_eq!(decoded.callables(), mir.callables());
    assert_eq!(
        decoded.functions()[matrix_record.function().index() as usize].defined_capability_contract(),
        Some(&SemanticDefinedCapabilityContractV1::KernelMatrixDerive(*matrix_record)),
        "V23 roundtrip must retain the complete original getter/bridge/Current record"
    );
    crate::collector::production_importer_v1::validate_execution_terminal_carriage_v1(
        tcx,
        &plan,
        &imported.kernel_contexts,
        &decoded,
    )
    .expect("decoded MatrixDerive and terminal records must replay against original source");
    validate_carriage(tcx, &plan, &imported.kernel_contexts, &decoded)
        .expect("decoded contracts must match actual retained terminal signatures and roots");

    let mut roles = BTreeSet::new();
    for (index, callable) in mir.callables().iter().enumerate() {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract },
            ..
        } = callable
        else {
            continue;
        };
        assert!(roles.insert(contract.operand().role));
        assert_eq!(contract.source_identity(), binding.identity());
        assert_eq!(contract.provenance().root(), mir.roots()[0]);
        assert_eq!(
            contract.memory(),
            SemanticCapabilityMemoryContractV1::global_read_only()
        );
        assert!(
            mir.functions()
                .iter()
                .flat_map(|function| function.blocks())
                .any(|block| {
                    matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                if call.callee().index() as usize == index && call.arguments().len() == 4)
                }),
            "the imported read contract must be consumed by an actual retained call"
        );

        // Equal physical layouts must not permit replacement of either nominal
        // identity, or swapping A/B roles, after inert admission succeeds.
        for mutation in 0..3 {
            let replacement = SemanticTypeIdentityV1::from_sha256([0xe9; 32]);
            assert_ne!(replacement, contract.matrix_brand());
            assert_ne!(replacement, contract.global_brand());
            let mut operand = contract.operand();
            if mutation == 2 {
                operand.role = match operand.role {
                    SemanticMfmaOperandRoleV1::A => SemanticMfmaOperandRoleV1::B,
                    SemanticMfmaOperandRoleV1::B => SemanticMfmaOperandRoleV1::A,
                };
            }
            let changed = SemanticGlobalBf16MatrixLoadV1::new(
                contract.types(),
                operand,
                if mutation == 0 {
                    replacement
                } else {
                    contract.matrix_brand()
                },
                if mutation == 1 {
                    replacement
                } else {
                    contract.global_brand()
                },
                contract.provenance(),
                contract.source_identity(),
            )
            .unwrap();
            let mut callables = mir.callables().to_vec();
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut callables[index]
            else {
                unreachable!();
            };
            *operation =
                SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { contract: changed };
            let changed = InertSemanticMirRequestV1::new_with_callables(
                mir.target(),
                mir.types().to_vec(),
                mir.allocations().to_vec(),
                mir.statics().to_vec(),
                mir.vtables().to_vec(),
                mir.functions().to_vec(),
                callables,
                mir.roots().to_vec(),
            )
            .unwrap()
            .admit_exact_v23(SemanticMirLimitsV1::default())
            .unwrap();
            assert!(
                validate_carriage(tcx, &plan, &imported.kernel_contexts, &changed).is_err(),
                "inert facts cannot authorize source substitution {mutation}"
            );
        }
    }
    assert_eq!(
        roles,
        BTreeSet::from([SemanticMfmaOperandRoleV1::A, SemanticMfmaOperandRoleV1::B])
    );
    let binds = mir.functions().iter().flat_map(|function| function.blocks()).filter(|block| {
        matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
            if matches!(mir.callables()[call.callee().index() as usize],
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly { .. }, ..
                }))
    }).count();
    assert_eq!(
        binds, 2,
        "reads cannot create their own Global allocation bindings"
    );

    if check_ssa {
        use fe2o3_pliron::{
            ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
            ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
        };
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            decoded,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let owner = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap_or_else(|error| {
            if let fe2o3_pliron::ProductionSemanticSsaErrorV1::ExpandedExecution {
                source_block: Some((instance_id, function_id, block_id)),
                source_local,
                ..
            } = &error
                && let Some(function) = mir.functions().get(function_id.index() as usize)
            {
                let matches = plan.function_producers().iter().filter(|producer| {
                    producer.identities.function() == function.identity()
                }).collect::<Vec<_>>();
                if let [producer] = matches.as_slice() {
                    let instance = producer.instance;
                    eprintln!(
                        "BF16 SSA rejected source: instance={} function={} identity={} source={} rustc_instance={instance:?} block={} local={source_local:?}",
                        instance_id.index(), function_id.index(),
                        crate::encode_hex(function.identity().as_bytes()),
                        tcx.def_path_str(instance.def_id()), block_id.index(),
                    );
                    for abi in plan.function_abi_producers().iter().filter(|abi| {
                        abi.function == *function_id && abi.identity == function.abi().identity()
                    }) {
                        eprintln!(
                            "BF16 SSA retained ABI: extern={:?} inputs={:?} output={:?}",
                            abi.extern_abi, abi.source_inputs, abi.source_output,
                        );
                    }
                } else {
                    eprintln!("BF16 SSA source producer identity match count={}", matches.len());
                }
                if let Some(local) = source_local
                    && local.function() == *function_id
                    && local.instance() == *instance_id
                    && let Some(declaration) = function.locals().get(local.local().index() as usize)
                {
                    eprintln!(
                        "BF16 SSA source local: identity={:?} role={:?} type={:?} shape={:?}",
                        declaration.identity(), declaration.role(), declaration.ty(),
                        mir.types().get(declaration.ty().index() as usize).map(|ty| ty.shape()),
                    );
                    if let Some(ty) = mir.types().get(declaration.ty().index() as usize) {
                        let producers = plan.type_producers().iter().filter(|producer| {
                            producer.identity == ty.identity()
                        }).collect::<Vec<_>>();
                        if let [producer] = producers.as_slice() {
                            eprintln!("BF16 SSA retained local Rust type: {:?}", producer.ty);
                        }
                    }
                }
                if let Some(block) = function.blocks().get(block_id.index() as usize) {
                    for (index, statement) in block.statements().iter().take(8).enumerate() {
                        eprintln!("BF16 SSA source statement={index}: {:?}", statement.kind());
                    }
                    eprintln!("BF16 SSA source terminator: {:?}", block.terminator().kind());
                }
            }
            panic!("retain actual captured shared Global and checked Result payload transport: {error:?}")
        });
        owner.verify_replay().unwrap();
        let view = owner.execution_view_for_root(mir.roots()[0]).unwrap();
        let reads = view.body().blocks().iter().filter(|block| {
            matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                if matches!(mir.callables()[call.callee().index() as usize],
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::GlobalBf16MatrixLoad { .. }, ..
                    }))
        }).count();
        assert_eq!(
            reads, 2,
            "expansion must retain both real terminal occurrences"
        );
    }
}

fn assert_root_matrix_route<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
) {
    let mut binds = 0;
    for (index, function) in plan.function_producers().iter().enumerate() {
        let instance = function.instance;
        if trusted_device_items::classify(tcx, instance.def_id())
            != Some(TrustedDeviceItem::PolicyMatrixBind)
        {
            continue;
        }
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        );
        eprintln!(
            "BF16 root matrix bind: function={index} source={} signature={signature:?}",
            tcx.def_path_str(instance.def_id())
        );
        let [matrix_reference, policy_reference] = signature.inputs() else {
            panic!("the root matrix bind retains exactly its two source references");
        };
        let matrix = rust_shared_reference_v1(*matrix_reference).unwrap();
        let matrix_args = rust_exact_reviewed_adt_arguments_v1(
            tcx,
            matrix,
            "fe2o3_device::matrix::MatrixCapability",
        )
        .unwrap()
        .types()
        .collect::<Vec<_>>();
        let [brand] = matrix_args.as_slice() else {
            panic!("exact matrix brand arity");
        };
        assert!(
            rust_kernel_brand_v1(tcx, *brand).is_some(),
            "KernelContext::matrix has a root brand, not an invented subgroup epoch"
        );
        let policy = rust_shared_reference_v1(*policy_reference).unwrap();
        let policy_args = rust_trusted_adt_type_arguments_v1(
            tcx,
            policy,
            TrustedDeviceItem::NumericalPolicyCapability,
        )
        .unwrap();
        let [global_brand, mode] = policy_args.as_slice() else {
            panic!("exact policy brand arity");
        };
        assert_eq!(brand, global_brand);
        assert!(rust_is_exact_trusted_marker_v1(
            tcx,
            *mode,
            TrustedDeviceItem::StrictIeeeNumericalPolicy
        ));
        let bound_args = rust_trusted_adt_type_arguments_v1(
            tcx,
            signature.output(),
            TrustedDeviceItem::PolicyMatrixCapability,
        )
        .unwrap();
        assert_eq!(bound_args, [*brand, *global_brand, *mode]);
        binds += 1;
    }
    assert_eq!(
        binds, 1,
        "retain the original root matrix/policy constructor; do not replace the fixture route"
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn global_bf16_full_import_gfx942() {
    run_probe(
        "gfx942",
        "import_tests::global_bf16_full_import_gfx942",
        ProbeStage::Import,
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn global_bf16_full_import_gfx950() {
    run_probe(
        "gfx950",
        "import_tests::global_bf16_full_import_gfx950",
        ProbeStage::Import,
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn global_bf16_source_ssa_gfx942() {
    run_probe(
        "gfx942",
        "import_tests::global_bf16_source_ssa_gfx942",
        ProbeStage::Ssa,
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn global_bf16_source_ssa_gfx950() {
    run_probe(
        "gfx950",
        "import_tests::global_bf16_source_ssa_gfx950",
        ProbeStage::Ssa,
    );
}
