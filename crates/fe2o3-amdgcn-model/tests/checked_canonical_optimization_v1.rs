//! Consumer-layer integration: the real binder owns N -> B; kernel-opt checks
//! only the supplied admitted V12 B -> O. No backend or replay wire is invoked.

use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_amdgcn_model::bind_production_target_v1;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1, CanonicalKirTransitionErrorV1, check_canonical_kir_transition_v1,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, Kernel, KernelId, LaunchDomain,
    LaunchExtent, Module, Operation, OperationKind, ScalarType, Signature, TargetCapability,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12 as Owner, WaveWidth,
    WorkgroupSize, gfx942_xnack_minus_target_capability, gfx950_xnack_minus_target_capability,
};
use fe2o3_kernel_opt::{
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2, optimize_checked_canonical_kernel_ir_v1,
};

const WORK: usize = 1_000_000_000_000;
const STORAGE: usize = 1_000_000_000;
const PREFIX: usize = 29;
const PRIOR_WORK: usize = 7;

fn neutral(changed: bool) -> Module {
    let mut block = BasicBlock::new(BlockId(if changed { 40 } else { 0 }));
    if changed {
        for (id, value) in [(17, 7), (93, 99)] {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(value)),
            ));
        }
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("checked-target-adapter");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn bound_roundtrip(changed: bool) {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let source = neutral(changed);
        let source_before = source.clone(); // Test oracle, not compiler scratch.
        let bound = bind_production_target_v1(&source, profile).unwrap();
        assert_eq!(source, source_before);
        assert_eq!(bound.profile(), profile);
        assert_eq!(bound.kernel_ids(), &[KernelId::new("kernel")]);
        let target = match profile {
            Profile::Gfx942 => gfx942_xnack_minus_target_capability(),
            Profile::Gfx950 => gfx950_xnack_minus_target_capability(),
        };
        let wave = TargetCapability::WaveWidth(WaveWidth::Wave64);
        for capabilities in [
            &bound.module().required_capabilities,
            &bound.module().kernels[0].required_capabilities,
            &bound.module().functions[0].required_capabilities,
        ] {
            assert!(capabilities.contains(&target));
            assert!(capabilities.contains(&wave));
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR_WORK).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let (input, input_storage) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        let input_retained = input_storage.retained_storage();
        budget.reserve_storage(input_retained).unwrap();
        assert_eq!(input.module(), bound.module());
        let checked = optimize_checked_canonical_kernel_ir_v1(&input, &mut budget).unwrap();
        assert_eq!(budget.storage(), PREFIX + input_retained);
        let retained = checked.storage().retained_storage();
        budget.reserve_storage(retained).unwrap();
        assert_eq!(
            checked.native_input_audit_bytes(),
            input.canonical().canonical_bytes()
        );
        let output = checked.owner().module();
        assert_eq!(output.id, input.module().id);
        assert_eq!(output.kernels, input.module().kernels);
        assert_eq!(
            output.required_capabilities,
            input.module().required_capabilities
        );
        assert_eq!(output.functions[0].id, input.module().functions[0].id);
        assert_eq!(output.functions[0].role, input.module().functions[0].role);
        assert_eq!(
            output.functions[0].signature,
            input.module().functions[0].signature
        );
        assert_eq!(
            output.functions[0].required_capabilities,
            input.module().functions[0].required_capabilities
        );
        let actual = checked
            .report()
            .passes()
            .iter()
            .map(|pass| pass.pass().name())
            .collect::<Vec<_>>();
        let expected = KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
            .iter()
            .map(|pass| pass.name())
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert_eq!(
            checked.report().passes().iter().any(|pass| pass.changed()),
            changed
        );
        assert!(checked.map().matches_execution(checked.report()));
        assert!(!checked.grants_authority());
        if changed {
            let original = &input.module().functions[0].body.as_ref().unwrap().blocks[0];
            assert_eq!(original.id, BlockId(40));
            assert_eq!(original.operations[0].results[0].id, ValueId(17));
            assert_eq!(original.operations[1].results[0].id, ValueId(93));
            assert_eq!(original.operations.len(), 2);
            assert!(
                output.functions[0].body.as_ref().unwrap().blocks[0]
                    .operations
                    .is_empty()
            );
            assert_ne!(
                checked.owner().canonical().canonical_bytes(),
                input.canonical().canonical_bytes()
            );
        } else {
            assert_eq!(output, input.module());
            assert_eq!(
                checked.owner().canonical().canonical_bytes(),
                input.canonical().canonical_bytes()
            );
        }
        drop(checked);
        budget.release_storage(retained).unwrap();
        drop(input);
        budget.release_storage(input_retained).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        assert!(budget.work() > PRIOR_WORK);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn real_binder_gfx942_and_gfx950_keep_exact_metadata_through_changed_non_dense_execution() {
    bound_roundtrip(true);
}

#[test]
fn real_binder_gfx942_and_gfx950_keep_exact_metadata_through_noop_execution() {
    bound_roundtrip(false);
}

#[test]
fn real_checked_rows_do_not_admit_foreign_target_or_kernel_metadata() {
    let source = neutral(false);
    let bound = bind_production_target_v1(&source, Profile::Gfx942).unwrap();
    let foreign = bind_production_target_v1(&source, Profile::Gfx950).unwrap();
    let mut renamed = bound.module().clone();
    renamed.kernels[0].id = KernelId::new("other_kernel");
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX).unwrap();
    let (input, input_storage) =
        Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let checked = optimize_checked_canonical_kernel_ir_v1(&input, &mut budget).unwrap();
    let checked_storage = checked.storage().retained_storage();
    budget.reserve_storage(checked_storage).unwrap();
    // This is an independent hostile-candidate checker test over the real
    // adapter result, not a public adapter hook for supplying another output.
    for (candidate, expected) in [
        (foreign.module(), "module metadata"),
        (&renamed, "kernel declaration"),
    ] {
        let (other, other_storage) =
            Owner::from_module_ref_with_verification_budget_v12(candidate, &mut budget).unwrap();
        budget
            .reserve_storage(other_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        {
            let (input_inventory, a) =
                CanonicalKirInventoryV1::derive(&input, &mut budget).unwrap();
            budget.reserve_storage(a.retained_storage()).unwrap();
            let (output_inventory, b) =
                CanonicalKirInventoryV1::derive(&other, &mut budget).unwrap();
            budget.reserve_storage(b.retained_storage()).unwrap();
            let before = budget.storage();
            assert!(
                matches!(check_canonical_kir_transition_v1(&input_inventory, &output_inventory, checked.occurrences().candidate(), &mut budget), Err(CanonicalKirTransitionErrorV1::Rule(rule)) if rule == expected)
            );
            assert_eq!(budget.storage(), before);
        }
        budget.release_storage(budget.storage() - floor).unwrap();
        drop(other);
        budget
            .release_storage(other_storage.retained_storage())
            .unwrap();
    }
    assert_eq!(checked.owner().module(), input.module());
    assert!(!checked.grants_authority());
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(input);
    budget
        .release_storage(input_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), PREFIX);
}
