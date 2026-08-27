use dialect_kernel::{
    AccessKindAttr, AtomicOrderingAttr, AtomicScopeAttr, DIALECT_NAME, IndexConstantOp,
    MemorySpaceAttr, RankedAccessOp, RankedViewOp, RankedViewType, ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    KernelCheckPassKindV1, KernelCheckStatusV1, MAX_PLIRON_ATOMIC_TARGET_CAPABILITIES_V1,
    MAX_PLIRON_SYSTEM_COHERENT_ALLOCATIONS_V1, PlironAtomicLegalityFindingV1,
    PlironAtomicTargetCapabilityV1, PlironAtomicTargetContextV1,
    require_pliron_atomic_legality_with_target_before_lowering_v1,
    run_pliron_atomic_legality_check_v1, run_pliron_atomic_legality_check_with_target_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{ops::FuncOp, types::FunctionType},
    context::{Context, Ptr},
    dialect::DialectName,
    op::Op,
    operation::Operation,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn function(context: &mut Context, name: &str) -> FuncOp {
    FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, vec![], vec![]),
    )
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn view(context: &mut Context, memory_space: MemorySpaceAttr) -> RankedViewOp {
    let view_type = RankedViewType::new(context, 32, true, vec![1]).unwrap();
    RankedViewOp::new_in_space_with_allocation_contract(
        context,
        view_type,
        vec![],
        memory_space,
        1,
        1,
    )
    .unwrap()
}

fn target(
    memory_space: MemorySpaceAttr,
    max_scope: AtomicScopeAttr,
) -> PlironAtomicTargetContextV1 {
    PlironAtomicTargetContextV1::new([PlironAtomicTargetCapabilityV1::new(
        32,
        memory_space,
        max_scope,
    )
    .unwrap()])
    .unwrap()
}

fn kernel_with_access(
    context: &mut Context,
    memory_space: MemorySpaceAttr,
    kind: AccessKindAttr,
    ordering: AtomicOrderingAttr,
    scope: AtomicScopeAttr,
) -> FuncOp {
    let function = function(context, "atomic_legality");
    let entry = function.get_entry_block(context);
    let memory = view(context, memory_space);
    let zero = IndexConstantOp::new(context, 0);
    let access = RankedAccessOp::new_atomic(
        context,
        kind,
        ordering,
        scope,
        memory.result(context),
        vec![zero.result(context)],
    )
    .unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &memory);
    append(context, entry, &zero);
    append(context, entry, &access);
    append(context, entry, &ret);
    function
}

#[test]
fn valid_atomic_contract_is_clean_only_with_a_matching_target_capability() {
    let context = &mut setup();
    let function = kernel_with_access(
        context,
        MemorySpaceAttr::Global,
        AccessKindAttr::AtomicReadModifyWrite,
        AtomicOrderingAttr::AcquireRelease,
        AtomicScopeAttr::Device,
    );

    let unbound = run_pliron_atomic_legality_check_v1(context, &function);
    assert_eq!(unbound.pass(), KernelCheckPassKindV1::AtomicLegality);
    assert_eq!(unbound.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        unbound.findings(),
        [PlironAtomicLegalityFindingV1::TargetCapabilityUnavailable { .. }]
    ));

    let target = target(MemorySpaceAttr::Global, AtomicScopeAttr::Device);
    let report = run_pliron_atomic_legality_check_with_target_v1(context, &function, &target);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert!(report.findings().is_empty());
    assert!(!report.grants_compiler_refinement_authority());
    assert!(!report.grants_artifact_or_launch_authority());
    assert!(!target.grants_compiler_refinement_authority());
    assert!(!target.grants_artifact_or_launch_authority());
    require_pliron_atomic_legality_with_target_before_lowering_v1(context, &function, &target)
        .unwrap();
}

#[test]
fn load_release_and_store_acquire_are_rejected_at_the_exact_operation() {
    for (kind, ordering) in [
        (AccessKindAttr::AtomicRead, AtomicOrderingAttr::Release),
        (AccessKindAttr::AtomicWrite, AtomicOrderingAttr::Acquire),
    ] {
        let context = &mut setup();
        let function = kernel_with_access(
            context,
            MemorySpaceAttr::Global,
            kind,
            ordering,
            AtomicScopeAttr::Device,
        );
        let report = run_pliron_atomic_legality_check_with_target_v1(
            context,
            &function,
            &target(MemorySpaceAttr::Global, AtomicScopeAttr::Device),
        );
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(matches!(
            report.findings(),
            [PlironAtomicLegalityFindingV1::InvalidOrdering {
                block: 0,
                operation: 2,
                ..
            }]
        ));
    }
}

#[test]
fn workgroup_memory_rejects_device_scope_and_private_memory_rejects_atomics() {
    for (memory_space, scope) in [
        (MemorySpaceAttr::Workgroup, AtomicScopeAttr::Device),
        (MemorySpaceAttr::Private, AtomicScopeAttr::Workgroup),
    ] {
        let context = &mut setup();
        let function = kernel_with_access(
            context,
            memory_space,
            AccessKindAttr::AtomicWrite,
            AtomicOrderingAttr::Release,
            scope,
        );
        let report = run_pliron_atomic_legality_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(matches!(
            report.findings(),
            [PlironAtomicLegalityFindingV1::InvalidScope {
                block: 0,
                operation: 2,
                ..
            }]
        ));
    }
}

#[test]
fn missing_contract_is_rejected_instead_of_being_treated_as_race_compatible() {
    let context = &mut setup();
    let function = function(context, "missing_atomic_contract");
    let entry = function.get_entry_block(context);
    let memory = view(context, MemorySpaceAttr::Global);
    let zero = IndexConstantOp::new(context, 0);
    let raw = Operation::new(
        context,
        RankedAccessOp::get_concrete_op_info(),
        vec![],
        vec![memory.result(context), zero.result(context)],
        vec![],
        0,
    );
    let access = RankedAccessOp::from_operation(raw);
    access.set_attr_kernel_access_kind(context, AccessKindAttr::AtomicWrite);
    let ret = ReturnOp::new(context);
    append(context, entry, &memory);
    append(context, entry, &zero);
    append(context, entry, &access);
    append(context, entry, &ret);

    let report = run_pliron_atomic_legality_check_v1(context, &function);
    assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
    assert!(matches!(
        report.findings(),
        [PlironAtomicLegalityFindingV1::MissingContract {
            ordering_missing: true,
            scope_missing: true,
            ..
        }]
    ));
}

#[test]
fn system_scope_stays_incomplete_without_authenticated_allocation_coherence() {
    let context = &mut setup();
    let function = kernel_with_access(
        context,
        MemorySpaceAttr::Global,
        AccessKindAttr::AtomicRead,
        AtomicOrderingAttr::Acquire,
        AtomicScopeAttr::System,
    );
    let target = target(MemorySpaceAttr::Global, AtomicScopeAttr::System);
    let report = run_pliron_atomic_legality_check_with_target_v1(context, &function, &target);
    assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
    assert!(matches!(
        report.findings(),
        [PlironAtomicLegalityFindingV1::SystemCoherenceUnproven {
            block: 0,
            operation: 2,
        }]
    ));
    let diagnostic =
        require_pliron_atomic_legality_with_target_before_lowering_v1(context, &function, &target)
            .unwrap_err()
            .to_string();
    assert!(diagnostic.contains("error[FE2O3-ATOMIC-002]"));
    assert!(diagnostic.contains("authenticated coherent-allocation provenance"));

    let coherent = target
        .with_system_coherent_allocations([1])
        .expect("bounded coherent allocation");
    let report = run_pliron_atomic_legality_check_with_target_v1(context, &function, &coherent);
    assert_eq!(report.status(), KernelCheckStatusV1::Clean);
    assert!(report.findings().is_empty());
}

#[test]
fn target_capability_context_is_bounded_and_rejects_invalid_claims() {
    assert!(
        PlironAtomicTargetCapabilityV1::new(
            32,
            MemorySpaceAttr::Private,
            AtomicScopeAttr::Workgroup,
        )
        .is_err()
    );
    assert!(
        PlironAtomicTargetCapabilityV1::new(
            32,
            MemorySpaceAttr::Workgroup,
            AtomicScopeAttr::Device,
        )
        .is_err()
    );
    let valid =
        PlironAtomicTargetCapabilityV1::new(32, MemorySpaceAttr::Global, AtomicScopeAttr::Device)
            .unwrap();
    assert!(
        PlironAtomicTargetContextV1::new(std::iter::repeat_n(
            valid,
            MAX_PLIRON_ATOMIC_TARGET_CAPABILITIES_V1 + 1
        ))
        .is_err()
    );
    assert!(
        PlironAtomicTargetContextV1::new([valid])
            .unwrap()
            .with_system_coherent_allocations([0])
            .is_err()
    );
    assert!(
        PlironAtomicTargetContextV1::new([valid])
            .unwrap()
            .with_system_coherent_allocations(
                1..=u64::try_from(MAX_PLIRON_SYSTEM_COHERENT_ALLOCATIONS_V1 + 1).unwrap(),
            )
            .is_err()
    );
}
