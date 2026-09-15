//! Actual AMD terminal ABI replay and independent whole-import checkpoints.
//! Neither checkpoint claims ranked memory refinement or executable authority.

use super::*;
use crate::collector::production_importer_v1::typed_matrix_terminal_v1 as typed;
use crate::production_target_v1::RetainedProductionTargetV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1;

pub(super) const SOURCE: &str = r#"
#![no_std]
#![allow(deprecated)]
use fe2o3_device::{kernel, KernelContext, KernelResult, Global, ReadOnly, StrictIeee,
    DeviceMatrix, MatrixCapability, KernelCapabilityBrand, CurrentTarget, RegisteredLaunch,
    UnbrandedCapability, Bf16MfmaAFragment, Bf16MfmaBFragment, Bf16MfmaFragment,
    F32AccumulatorFragment, Bf16F32M16N16K16, MfmaOperandA, MfmaOperandB,
    MfmaRegisterTile16x16, MfmaAccumulatorRowMajor, Wave64, Wave32,
    SubgroupBrand, InitialEpoch, NextEpoch};
pub enum Root {}
pub enum Foreign {}
type Brand = KernelCapabilityBrand<'static, Root, CurrentTarget, RegisteredLaunch>;
type Other = KernelCapabilityBrand<'static, Foreign, CurrentTarget, RegisteredLaunch>;
type Group = SubgroupBrand<'static, Wave64, Brand, InitialEpoch>;
type NextGroup = SubgroupBrand<'static, Wave64, Brand, NextEpoch<InitialEpoch>>;
type A<Brand> = Bf16MfmaAFragment<'static, Brand>;
type B<Brand> = Bf16MfmaBFragment<'static, Brand>;
type C<Brand> = F32AccumulatorFragment<'static, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, Brand>;
type U = UnbrandedCapability;
pub mod counterfeit {
    pub struct UnbrandedCapability;
    pub struct DeviceMatrix;
    pub struct Fragment([u16; 4]);
    pub struct Accumulator([f32; 4]);
}
pub fn legacy(m: &DeviceMatrix, a: A<U>, b: B<U>, c: C<U>) -> C<U> {
    m.multiply_accumulate(a, b, c)
}
pub fn scoped(m: &MatrixCapability<Brand>, a: A<Brand>, b: B<Brand>, c: C<Brand>) -> C<Brand> {
    m.multiply_accumulate(a, b, c)
}
pub fn mutable_context(_: &mut DeviceMatrix) {}
pub fn raw_context(_: *const DeviceMatrix) {}
pub fn value_context(_: DeviceMatrix) {}
pub fn fake_context(_: &counterfeit::DeviceMatrix) {}
pub fn a_root(_: A<Brand>) {}
pub fn a_foreign(_: A<Other>) {}
pub fn a_group(_: A<Group>) {}
pub fn a_next(_: A<NextGroup>) {}
pub fn a_fake_brand(_: A<counterfeit::UnbrandedCapability>) {}
pub fn a_borrow(_: &A<U>) {}
pub fn a_fake(_: counterfeit::Fragment) {}
pub fn a_role(_: B<U>) {}
pub fn a_profile(_: Bf16MfmaFragment<'static, MfmaOperandA, Root, MfmaRegisterTile16x16, Wave64>) {}
pub fn a_distribution(_: Bf16MfmaFragment<'static, MfmaOperandA, Bf16F32M16N16K16, Root, Wave64>) {}
pub fn a_width(_: Bf16MfmaFragment<'static, MfmaOperandA, Bf16F32M16N16K16, MfmaRegisterTile16x16, Wave32>) {}
pub fn b_root(_: B<Brand>) {}
pub fn b_fake_brand(_: B<counterfeit::UnbrandedCapability>) {}
pub fn b_borrow(_: &B<U>) {}
pub fn b_role(_: A<U>) {}
pub fn b_profile(_: Bf16MfmaFragment<'static, MfmaOperandB, Root, MfmaRegisterTile16x16, Wave64>) {}
pub fn b_distribution(_: Bf16MfmaFragment<'static, MfmaOperandB, Bf16F32M16N16K16, Root, Wave64>) {}
pub fn b_width(_: Bf16MfmaFragment<'static, MfmaOperandB, Bf16F32M16N16K16, MfmaRegisterTile16x16, Wave32>) {}
pub fn c_root(_: C<Brand>) {}
pub fn c_foreign(_: C<Other>) {}
pub fn c_group(_: C<Group>) {}
pub fn c_next(_: C<NextGroup>) {}
pub fn c_fake_brand(_: C<counterfeit::UnbrandedCapability>) {}
pub fn c_borrow(_: &C<U>) {}
pub fn c_fake(_: counterfeit::Accumulator) {}
pub fn c_profile(_: F32AccumulatorFragment<'static, Root>) {}
pub fn c_distribution(_: F32AccumulatorFragment<'static, Bf16F32M16N16K16, Root>) {}
pub fn c_width(_: F32AccumulatorFragment<'static, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave32>) {}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn global_bf16_multiply(context: KernelContext<'_>, a: Global<'_, u16, ReadOnly>,
    b: Global<'_, u16, ReadOnly>, n: u32) -> KernelResult {
    let policy = context.numerical_policy::<StrictIeee>();
    let lane = context.subgroup_lane::<Wave64>();
    let matrix = context.matrix();
    let matrix = matrix.with_numerical_policy(&policy);
    let a_view = matrix.bf16_a_global_row_major(&a, 0, n as usize, n as usize, n as usize)?;
    let b_view = matrix.bf16_b_global_row_major(&b, 0, n as usize, n as usize, n as usize)?;
    let a_fragment = a_view.load_m16k16(&lane, 0, 0);
    let b_fragment = b_view.load_k16n16(&lane, 0, 0);
    let accumulator = matrix.bf16_zero_accumulator(&lane);
    let accumulator = matrix.multiply_accumulate(a_fragment, b_fragment, accumulator);
    let _values = accumulator.into_values();
    Ok(())
}
"#;

const SUBSTITUTIONS: &[(&str, usize)] = &[
    ("mutable_context", 0),
    ("raw_context", 0),
    ("value_context", 0),
    ("fake_context", 0),
    ("a_root", 1),
    ("a_foreign", 1),
    ("a_group", 1),
    ("a_next", 1),
    ("a_fake_brand", 1),
    ("a_borrow", 1),
    ("a_fake", 1),
    ("a_role", 1),
    ("a_profile", 1),
    ("a_distribution", 1),
    ("a_width", 1),
    ("b_root", 2),
    ("b_fake_brand", 2),
    ("b_borrow", 2),
    ("b_role", 2),
    ("b_profile", 2),
    ("b_distribution", 2),
    ("b_width", 2),
    ("c_root", 3),
    ("c_foreign", 3),
    ("c_group", 3),
    ("c_next", 3),
    ("c_fake_brand", 3),
    ("c_borrow", 3),
    ("c_fake", 3),
    ("c_profile", 3),
    ("c_distribution", 3),
    ("c_width", 3),
];

fn local_instance<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture {name}"));
    Instance::mono(tcx, definition.to_def_id())
}

fn signature<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> rustc_middle::ty::FnSig<'tcx> {
    let instance = local_instance(tcx, name);
    tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    )
}

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>, cpu: &str, full_import: bool) {
    let expansion = ProductionTerminalExpansionV1::MatrixMultiplyAccumulate;
    let actual = signature(tcx, "legacy");
    assert!(typed::source_matches(
        tcx,
        expansion,
        actual.inputs(),
        actual.output()
    ));
    let scoped = signature(tcx, "scoped");
    assert!(!typed::source_matches(
        tcx,
        expansion,
        scoped.inputs(),
        scoped.output()
    ));
    for &(name, index) in SUBSTITUTIONS {
        let replacement = signature(tcx, name).inputs()[0];
        let mut inputs = actual.inputs().to_vec();
        assert_ne!(inputs[index], replacement, "{name}");
        inputs[index] = replacement;
        assert!(
            !typed::source_matches(tcx, expansion, &inputs, actual.output()),
            "{name}"
        );
        if index == 3 {
            // Equal input/output replacements must still fail the exact brand/profile check.
            assert!(
                !typed::source_matches(tcx, expansion, &inputs, replacement),
                "{name}"
            );
            assert!(
                !typed::source_matches(tcx, expansion, actual.inputs(), replacement),
                "output {name}"
            );
        }
    }
    for count in 0..4 {
        assert!(!typed::source_matches(
            tcx,
            expansion,
            &actual.inputs()[..count],
            actual.output()
        ));
    }
    let mut extra = actual.inputs().to_vec();
    extra.push(actual.inputs()[3]);
    assert!(!typed::source_matches(
        tcx,
        expansion,
        &extra,
        actual.output()
    ));

    let calls_real_terminal = |instance: Instance<'tcx>| {
        tcx.instance_mir(instance.def)
            .basic_blocks
            .iter()
            .any(|block| {
                let TerminatorKind::Call {
                    func: Operand::Constant(callee),
                    ..
                } = &block.terminator().kind
                else {
                    return false;
                };
                let TyKind::FnDef(definition, _) = callee.const_.ty().kind() else {
                    return false;
                };
                trusted_device_items::classify(tcx, *definition)
                    == Some(TrustedDeviceItem::DeviceMatrixMultiplyAccumulate)
            })
    };
    assert!(calls_real_terminal(local_instance(tcx, "legacy")));
    assert!(
        !calls_real_terminal(local_instance(tcx, "scoped")),
        "scoped adaptation must remain a defined call"
    );

    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
    let partitions = tcx.collect_and_partition_mono_items(());
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .expect("collect actual Global loads, checked constructors, scoped adapter and legacy MMA");
    let observed_target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    let mut observed_closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap(),
    )
    .unwrap();
    let collected_contexts =
        collect_authenticated_kernel_contexts_v1(tcx, &mut observed_closure.collection).unwrap();
    let inventory = build_identity_inventory_v1(
        tcx,
        &observed_target,
        &observed_closure.collection,
        &observed_closure.roots,
    )
    .unwrap();
    let contexts = bind_authenticated_kernel_contexts_v1(
        collected_contexts,
        &inventory.functions,
        &inventory.roots,
        &AuthenticatedRustcIdentityInventoryV3 {
            sha256: inventory.sha256,
            canonical_transcript: inventory.canonical_transcript.clone(),
        },
        &observed_target,
    )
    .unwrap();
    let closure_types = observed_closure
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
    .unwrap();
    assert!(
        plan.function_producers()
            .iter()
            .any(|function| calls_real_terminal(function.instance)),
        "the registered source closure must actually consume the legacy terminal"
    );
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers())
        .unwrap()
        .into_records();
    let abi_producers = plan
        .terminal_producers()
        .iter()
        .map(|terminal| terminal.abi.clone())
        .collect::<Vec<_>>();
    let abis = construct_production_semantic_fn_abis_v1(tcx, &abi_producers, plan.type_producers())
        .unwrap()
        .into_records();
    let mut found = 0;
    for (index, terminal) in plan.terminal_producers().iter().enumerate() {
        if terminal.expansion != expansion {
            continue;
        }
        found += 1;
        let abi = &abis[index];
        let root =
            capability_memory_root_for_terminal_v1(tcx, &plan, &contexts, index as u32, expansion)
                .unwrap();
        assert!(
            root.is_none(),
            "legacy ABI matching cannot mint a scoped root contract"
        );
        let operation = terminal_operation_v1(
            tcx,
            terminal.instance,
            expansion,
            abi,
            &types,
            root,
            terminal.identities.function(),
            &contexts,
        )
        .expect("the actual production dispatch must accept the exact legacy ABI");
        let SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate {
            context,
            lhs_fragment,
            rhs_fragment,
            accumulator_fragment,
            lhs,
            rhs,
            accumulator,
        } = operation
        else {
            panic!("wrong operation");
        };
        assert_eq!(
            context,
            pointer_pointee_v1(&types, abi.source_input_types()[0]).unwrap()
        );
        assert_eq!(
            &abi.source_input_types()[1..],
            &[lhs_fragment, rhs_fragment, accumulator_fragment]
        );
        assert_eq!(abi.source_output_type(), accumulator_fragment);
        assert_eq!(lhs.role, SemanticMfmaOperandRoleV1::A);
        assert_eq!(rhs.role, SemanticMfmaOperandRoleV1::B);
        assert_eq!(lhs.profile, SemanticMfmaProfileV1::Bf16F32M16N16K16);
        assert_eq!(rhs.profile, lhs.profile);
        assert_eq!(accumulator.profile, lhs.profile);
        assert_eq!(
            (lhs.wave_width, rhs.wave_width, accumulator.wave_width),
            (64, 64, 64)
        );
        assert_eq!(
            lhs.register_distribution,
            SemanticMfmaRegisterDistributionV1::Tile16x16
        );
        assert_eq!(rhs.register_distribution, lhs.register_distribution);
        assert_eq!(
            accumulator.distribution,
            SemanticMfmaAccumulatorDistributionV1::RowMajor
        );
        assert!(
            typed::operation(tcx, local_instance(tcx, "legacy"), expansion, abi, &types).is_err(),
            "a same-signature source wrapper cannot impersonate the authenticated terminal"
        );
        for argument in 0..4 {
            let mut ownership = abi.source_argument_ownership().to_vec();
            ownership[argument] = if argument == 0 {
                SemanticSourceArgumentOwnershipV1::ByValue
            } else {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            };
            let changed = abi
                .clone()
                .with_source_argument_ownership(ownership)
                .unwrap();
            assert!(
                typed::operation(tcx, terminal.instance, expansion, &changed, &types).is_err(),
                "ownership {argument}"
            );
        }
        for &(name, argument) in SUBSTITUTIONS {
            let id = abi.source_input_types()[argument].index() as usize;
            let mut changed = types.clone();
            let original = &types[id];
            let identity = rustc_type_identity_v1(tcx, signature(tcx, name).inputs()[0]);
            assert_ne!(identity, original.identity(), "{name}");
            changed[id] = SemanticTypeDeclV1::new(
                identity,
                original.layout_identity(),
                original.layout().clone(),
                original.shape().clone(),
            )
            .with_rustc_abi_properties(original.abi_properties())
            .with_rust_type_kind(original.rust_type_kind());
            assert!(
                typed::operation(tcx, terminal.instance, expansion, abi, &changed).is_err(),
                "unchanged physical layout cannot authorize nominal substitution {name}"
            );
        }
    }
    assert_eq!(found, 1);
    if full_import {
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("complete actual-source import is a separate mandatory checkpoint");
        assert_eq!(imported.rustc_target.contract().cpu(), cpu);
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let mir = &imported.semantic_mir;
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        typed::validate_carriage(tcx, &plan, &decoded).unwrap();
        for (index, terminal) in plan.terminal_producers().iter().enumerate() {
            if terminal.expansion != expansion {
                continue;
            }
            let callable = plan.function_producers().len() + index;
            assert!(
                decoded
                    .functions()
                    .iter()
                    .flat_map(|function| function.blocks())
                    .any(|block| {
                        matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                    if call.callee().index() as usize == callable && call.arguments().len() == 4)
                    })
            );
        }
    }
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn legacy_multiply_terminal_abi_gfx942() {
    run_probe(
        "gfx942",
        "legacy_multiply_tests::legacy_multiply_terminal_abi_gfx942",
        ProbeStage::LegacyMultiply,
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn legacy_multiply_terminal_abi_gfx950() {
    run_probe(
        "gfx950",
        "legacy_multiply_tests::legacy_multiply_terminal_abi_gfx950",
        ProbeStage::LegacyMultiply,
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn legacy_multiply_full_import_gfx942() {
    run_probe(
        "gfx942",
        "legacy_multiply_tests::legacy_multiply_full_import_gfx942",
        ProbeStage::LegacyMultiplyImport,
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn legacy_multiply_full_import_gfx950() {
    run_probe(
        "gfx950",
        "legacy_multiply_tests::legacy_multiply_full_import_gfx950",
        ProbeStage::LegacyMultiplyImport,
    );
}
