//! Real AMD source and exact ABI substitutions for the two typed terminals.

use super::*;
use crate::collector::production_importer_v1::typed_matrix_terminal_v1 as typed;
use crate::production_target_v1::RetainedProductionTargetV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1;

pub(super) const SOURCE: &str = r#"
#![no_std]
#![allow(deprecated)]
use fe2o3_device::{kernel, KernelContext, KernelResult, StrictIeee,
    KernelCapabilityBrand, CurrentTarget, RegisteredLaunch, UnbrandedCapability,
    WaveLane, Wave64, Wave32, F32AccumulatorFragment, Bf16F32M16N16K16,
    MfmaAccumulatorRowMajor, SubgroupBrand, InitialEpoch, NextEpoch};
pub enum Root {}
pub enum Foreign {}
type Brand = KernelCapabilityBrand<'static, Root, CurrentTarget, RegisteredLaunch>;
type Other = KernelCapabilityBrand<'static, Foreign, CurrentTarget, RegisteredLaunch>;
type Group = SubgroupBrand<'static, Wave64, Brand, InitialEpoch>;
type NextGroup = SubgroupBrand<'static, Wave64, Brand, NextEpoch<InitialEpoch>>;
type Acc<B> = F32AccumulatorFragment<'static, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, B>;

pub fn current() -> WaveLane<Wave64> { WaveLane::current() }
pub fn current_scoped() -> WaveLane<Wave64, Brand> { loop {} }
pub fn current_wave32() -> WaveLane<Wave32> { WaveLane::current() }
pub mod counterfeit { pub struct UnbrandedCapability; }
pub fn current_counterfeit() -> WaveLane<Wave64, counterfeit::UnbrandedCapability> { loop {} }
pub fn project_unbranded(a: Acc<UnbrandedCapability>) -> [f32; 4] { a.into_values() }
pub fn project_root(a: Acc<Brand>) -> [f32; 4] { a.into_values() }
pub fn project_foreign(a: Acc<Other>) -> [f32; 4] { a.into_values() }
pub fn project_group(a: Acc<Group>) -> [f32; 4] { a.into_values() }
pub fn project_next(a: Acc<NextGroup>) -> [f32; 4] { a.into_values() }
pub fn borrowed(_: &Acc<Brand>) -> [f32; 4] { loop {} }
pub fn wrong_profile(_: F32AccumulatorFragment<'static, Root>) -> [f32; 4] { loop {} }
pub fn wrong_distribution(_: F32AccumulatorFragment<'static, Bf16F32M16N16K16, Root>) -> [f32; 4] { loop {} }
pub fn wrong_width(_: F32AccumulatorFragment<'static, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave32>) -> [f32; 4] { loop {} }
pub fn wrong_output(_: Acc<Brand>) -> [f32; 3] { loop {} }
pub struct CounterfeitAccumulator([f32; 4]);
pub fn counterfeit(_: CounterfeitAccumulator) -> [f32; 4] { loop {} }

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn typed_matrix_projection(context: KernelContext<'_>) -> KernelResult {
    let _compatibility = WaveLane::<Wave64>::current();
    let policy = context.numerical_policy::<StrictIeee>();
    let lane = context.subgroup_lane::<Wave64>();
    let matrix = context.matrix();
    let matrix = matrix.with_numerical_policy(&policy);
    let accumulator = matrix.bf16_zero_accumulator(&lane);
    let _values = accumulator.into_values();
    Ok(())
}
"#;

fn local_instance<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture {name}"));
    Instance::mono(tcx, definition.to_def_id())
}

pub(super) fn check(tcx: TyCtxt<'_>, cpu: &str) {
    use ProductionTerminalExpansionV1::{
        F32MatrixAccumulatorIntoValues as Project, WaveLaneCurrent as Current,
    };
    let mut projection_identities = BTreeSet::new();
    for (name, expansion, positive) in [
        ("current", Current, true),
        ("current_scoped", Current, false),
        ("current_wave32", Current, false),
        ("current_counterfeit", Current, false),
        ("project_unbranded", Project, true),
        ("project_root", Project, true),
        ("project_foreign", Project, true),
        ("project_group", Project, true),
        ("project_next", Project, true),
        ("borrowed", Project, false),
        ("wrong_profile", Project, false),
        ("wrong_distribution", Project, false),
        ("wrong_width", Project, false),
        ("wrong_output", Project, false),
        ("counterfeit", Project, false),
    ] {
        let instance = local_instance(tcx, name);
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        );
        assert_eq!(
            typed::source_matches(tcx, expansion, signature.inputs(), signature.output()),
            positive,
            "{name}"
        );
        if positive {
            let expected = if expansion == Current {
                TrustedDeviceItem::WaveLaneCurrent
            } else {
                assert!(
                    projection_identities
                        .insert(rustc_type_identity_v1(tcx, signature.inputs()[0]))
                );
                TrustedDeviceItem::F32AccumulatorFragmentIntoValues
            };
            assert!(
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
                        trusted_device_items::classify(tcx, *definition) == Some(expected)
                    }),
                "{name}: positive must call the real authenticated terminal"
            );
        }
    }
    assert_eq!(
        projection_identities.len(),
        5,
        "projection never normalizes brands or epochs"
    );

    let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
    let partitions = tcx.collect_and_partition_mono_items(());
    let closure = crate::collector::collect_authenticated_kernel_closure_v1(
        tcx,
        partitions.codegen_units,
        false,
        target,
    )
    .expect("collect the actual registered typed matrix source");
    let observed = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
        .unwrap()
        .authenticate_import_session(tcx)
        .unwrap();
    let inventory =
        build_identity_inventory_v1(tcx, &observed, &closure.collection, &closure.roots).unwrap();
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
        canonical_target_layout_v1(observed.rustc_layout()),
        inventory.functions,
        inventory.roots,
        inventory.sha256,
        &closure_types,
        DebugSourceCaptureRequestV2::Disabled,
    )
    .unwrap();
    let imported =
        construct_production_semantic_mir_v1(tcx, closure, DebugSourceCaptureRequestV2::Disabled)
            .expect("import both typed terminal ABIs from real source");
    assert_eq!(imported.rustc_target.contract().cpu(), cpu);
    let mir = &imported.semantic_mir;
    typed::validate_carriage(tcx, &plan, mir).unwrap();

    let mut found = BTreeSet::new();
    for (index, terminal) in plan.terminal_producers().iter().enumerate() {
        if !typed::handles(terminal.expansion) {
            continue;
        }
        let callable = plan.function_producers().len() + index;
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = &mir.callables()[callable]
        else {
            panic!("missing terminal");
        };
        assert!(
            mir.functions()
                .iter()
                .flat_map(|function| function.blocks())
                .any(|block| {
                    matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
                if call.callee().index() as usize == callable)
                }),
            "the terminal record must be consumed by an actual source call"
        );
        assert_eq!(
            typed::operation(
                tcx,
                terminal.instance,
                terminal.expansion,
                binding.abi(),
                mir.types()
            )
            .unwrap(),
            *operation
        );
        let (id, substitutions, fake) = match operation {
            SemanticCompilerIntrinsicOperationV1::WaveLaneCurrent { lane, wave_width } => {
                assert_eq!(*wave_width, 64);
                assert!(found.insert("current"));
                (
                    *lane,
                    vec!["current_scoped", "current_wave32", "current_counterfeit"],
                    "current",
                )
            }
            SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorIntoValues {
                fragment,
                values,
            } => {
                assert!(found.insert("projection"));
                assert_eq!(binding.abi().source_input_types(), &[*fragment]);
                assert_eq!(binding.abi().source_output_type(), *values);
                let borrowed = binding
                    .abi()
                    .clone()
                    .with_source_argument_ownership(vec![
                        SemanticSourceArgumentOwnershipV1::SharedBorrow,
                    ])
                    .unwrap();
                assert!(
                    typed::operation(
                        tcx,
                        terminal.instance,
                        terminal.expansion,
                        &borrowed,
                        mir.types()
                    )
                    .is_err()
                );
                (
                    *fragment,
                    vec![
                        "project_unbranded",
                        "project_root",
                        "project_foreign",
                        "project_group",
                        "project_next",
                    ],
                    "project_root",
                )
            }
            _ => panic!("unexpected typed operation"),
        };
        assert!(
            typed::operation(
                tcx,
                local_instance(tcx, fake),
                terminal.expansion,
                binding.abi(),
                mir.types()
            )
            .is_err(),
            "a same-signature caller cannot replace the authenticated terminal"
        );
        for name in substitutions {
            let instance = local_instance(tcx, name);
            let signature = tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(instance.def_id())
                    .instantiate(tcx, instance.args),
            );
            let replacement = if terminal.expansion == Current {
                signature.output()
            } else {
                signature.inputs()[0]
            };
            let mut types = mir.types().to_vec();
            let old = &types[id.index() as usize];
            let identity = rustc_type_identity_v1(tcx, replacement);
            assert_ne!(identity, old.identity());
            types[id.index() as usize] = SemanticTypeDeclV1::new(
                identity,
                old.layout_identity(),
                old.layout().clone(),
                old.shape().clone(),
            )
            .with_rustc_abi_properties(old.abi_properties())
            .with_rust_type_kind(old.rust_type_kind());
            assert!(
                typed::operation(
                    tcx,
                    terminal.instance,
                    terminal.expansion,
                    binding.abi(),
                    &types
                )
                .is_err(),
                "exact source type substitution {name}"
            );
            let changed = InertSemanticMirRequestV1::new_with_callables(
                mir.target(),
                types,
                mir.allocations().to_vec(),
                mir.statics().to_vec(),
                mir.vtables().to_vec(),
                mir.functions().to_vec(),
                mir.callables().to_vec(),
                mir.roots().to_vec(),
            )
            .unwrap()
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap();
            assert!(
                typed::validate_carriage(tcx, &plan, &changed).is_err(),
                "inert admission is not source authority: {name}"
            );
        }
    }
    assert_eq!(found, BTreeSet::from(["current", "projection"]));
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn typed_matrix_terminal_abis_gfx942() {
    run_probe(
        "gfx942",
        "typed_terminal_tests::typed_matrix_terminal_abis_gfx942",
        ProbeStage::TypedTerminals,
    );
}

#[test]
#[ignore = "requires existing actual AMD metadata; never builds dependencies"]
fn typed_matrix_terminal_abis_gfx950() {
    run_probe(
        "gfx950",
        "typed_terminal_tests::typed_matrix_terminal_abis_gfx950",
        ProbeStage::TypedTerminals,
    );
}
