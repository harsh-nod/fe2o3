//! These controls accept only the actual rustc-derived checked target.
//! Test-only candidate copies are rejected ABI projections, not source owners.
use super::*;
fn denial_only_resource_controls(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    abi: &PhysicalGlobalCopyPreparedAbiV21,
) {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as WorkBudget;
    let owner = checked.retained_storage();
    let complete = owner.checked_add(abi.retained_storage()).unwrap();
    // Fresh ledgers below are failure-only. No admitted owner is constructed
    // from them; the actual source owner remains held by its original ledger.
    let mut work = WorkBudget::new(WORK);
    let mut missing_owner = Budget::new(&mut work, complete);
    assert!(matches!(
        prepare_physical_global_copy_abi_v21(checked, roots, &mut missing_owner),
        Err(CompilerDescriptorError::PhysicalGlobalCopyResourceV21(
            Resource::Accounting
        ))
    ));
    assert_eq!((missing_owner.storage(), missing_owner.work()), (0, 1));
    let mut work = WorkBudget::new(WORK);
    let mut missing_abi = Budget::new(&mut work, complete);
    missing_abi.reserve_storage(owner).unwrap();
    assert!(matches!(
        validate_prepared_abi_v21(checked, roots, abi, &mut missing_abi),
        Err(CompilerDescriptorError::PhysicalGlobalCopyResourceV21(
            Resource::Accounting
        ))
    ));
    assert_eq!((missing_abi.storage(), missing_abi.work()), (owner, 1));
    let mut work = WorkBudget::new(WORK);
    let mut short_storage = Budget::new(&mut work, complete - 1);
    short_storage.reserve_storage(owner).unwrap();
    assert!(matches!(
        prepare_physical_global_copy_abi_v21(checked, roots, &mut short_storage),
        Err(CompilerDescriptorError::PhysicalGlobalCopyResourceV21(
            Resource::Storage(_)
        ))
    ));
    assert_eq!(short_storage.storage(), owner);
    let mut work = WorkBudget::new(WORK - 1);
    let mut short_work = Budget::new(&mut work, complete);
    short_work.reserve_storage(complete).unwrap();
    assert!(matches!(
        validate_prepared_abi_v21(checked, roots, abi, &mut short_work),
        Err(CompilerDescriptorError::PhysicalGlobalCopyResourceV21(
            Resource::Work(_)
        ))
    ));
    assert_eq!(short_work.storage(), complete);
}
fn copied(abi: &PhysicalGlobalCopyPreparedAbiV21) -> PhysicalGlobalCopyPreparedAbiV21 {
    PhysicalGlobalCopyPreparedAbiV21 {
        canonical: abi.canonical,
        semantic: abi.semantic,
        binding: abi.binding,
        reads: abi.reads,
        input_read: abi.input_read,
        output_store: abi.output_store,
        conditions: abi.conditions,
        retained: abi.retained,
    }
}
fn exact_failure(result: Result<(), CompilerDescriptorError>, expected: &'static str) {
    assert!(
        matches!(result,Err(CompilerDescriptorError::ProductionDescriptorMismatch(field)) if field==expected),
        "expected exact descriptor refusal {expected}"
    );
}
pub(crate) fn qualify_actual_owner_abi_controls_v21(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    abi: &PhysicalGlobalCopyPreparedAbiV21,
    budget: &mut Budget<'_>,
) -> usize {
    denial_only_resource_controls(checked, roots, abi);
    let floor = budget.storage();
    let before = budget.work();
    validate_prepared_abi_v21(checked, roots, abi, budget).unwrap();
    assert_eq!(budget.work() - before, WORK);
    // Exact retained-capacity preparation succeeds under the original ledger;
    // the temporary receipt is scoped and storage is restored without reset.
    {
        let second = prepare_physical_global_copy_abi_v21(checked, roots, budget).unwrap();
        assert_eq!(
            second.retained_storage(),
            std::mem::size_of::<PhysicalGlobalCopyPreparedAbiV21>()
        );
        assert_eq!(second.reads, abi.reads);
        assert_eq!(second.conditions, abi.conditions);
    }
    assert_eq!(budget.storage(), floor);
    let mut count = 0;
    for case in 0..35 {
        let mut candidate = copied(abi);
        let expected = match case {
            0 => {
                candidate.reads = [None; 4];
                "global-copy ABI exact ordered read report"
            }
            1 => {
                candidate.reads[3] = None;
                "global-copy ABI exact ordered read report"
            }
            2 => {
                candidate.reads.swap(0, 1);
                "global-copy ABI exact ordered read report"
            }
            3 => {
                candidate.reads[0]
                    .as_mut()
                    .unwrap()
                    .location
                    .operation_index += 1;
                "global-copy ABI exact ordered read report"
            }
            4 => {
                candidate.reads[0].as_mut().unwrap().site.occurrence ^= 1;
                "global-copy ABI exact ordered read report"
            }
            5 => {
                candidate.reads[0]
                    .as_mut()
                    .unwrap()
                    .ready_at
                    .operation_index += 1;
                "global-copy ABI exact ordered read report"
            }
            6 => {
                candidate.reads[0].as_mut().unwrap().ready_site.occurrence ^= 1;
                "global-copy ABI exact ordered read report"
            }
            7 => {
                candidate.reads[0].as_mut().unwrap().slot = Slot::OutputPointer;
                "global-copy ABI exact ordered read report"
            }
            8 => {
                candidate.reads[0].as_mut().unwrap().results.swap(0, 1);
                "global-copy ABI exact ordered read report"
            }
            9 => {
                candidate.canonical[0] ^= 1;
                "global-copy ABI retained source identity"
            }
            10 => {
                candidate.semantic[0] ^= 1;
                "global-copy ABI retained source identity"
            }
            11 => {
                candidate.binding[0] ^= 1;
                "global-copy ABI retained source identity"
            }
            12 => {
                candidate.conditions.minimum_input_bytes = 256;
                "global-copy ABI retained unresolved conditions"
            }
            13 => {
                candidate.conditions.minimum_output_bytes = 256;
                "global-copy ABI retained unresolved conditions"
            }
            14 => {
                candidate.conditions.input_readable = false;
                "global-copy ABI retained unresolved conditions"
            }
            15 => {
                candidate.conditions.input_initialized = false;
                "global-copy ABI retained unresolved conditions"
            }
            16 => {
                candidate.conditions.output_writable = false;
                "global-copy ABI retained unresolved conditions"
            }
            17 => {
                candidate.conditions.input_output_disjoint = false;
                "global-copy ABI retained unresolved conditions"
            }
            18 => {
                candidate.conditions.kernarg_immutable = false;
                "global-copy ABI retained unresolved conditions"
            }
            19 => {
                candidate.conditions.kernarg_live = false;
                "global-copy ABI retained unresolved conditions"
            }
            20 => {
                candidate.conditions.kernarg_readable = false;
                "global-copy ABI retained unresolved conditions"
            }
            21 => {
                candidate.conditions.kernarg_bytes = 24;
                "global-copy ABI retained unresolved conditions"
            }
            22 => {
                candidate.conditions.kernarg_alignment = 4;
                "global-copy ABI retained unresolved conditions"
            }
            23 => {
                candidate.conditions.kernarg_disjoint_output = candidate.conditions.input;
                "global-copy ABI retained unresolved conditions"
            }
            24 => {
                candidate.input_read.access.allocation = candidate.conditions.output;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            25 => {
                candidate.input_read.access.site.occurrence ^= 1;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            26 => {
                candidate.input_read.ready_at.operation_index += 1;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            27 => {
                candidate.input_read.ready_site.occurrence ^= 1;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            28 => {
                candidate.input_read.access.exec = candidate.output_store.access.exec;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            29 => {
                candidate.output_store.value = candidate.output_store.length[0];
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            30 => {
                candidate.output_store.access.index.swap(0, 1);
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            31 => {
                candidate.output_store.mask_site.occurrence ^= 1;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            32 => {
                candidate.output_store.comparison_site.occurrence ^= 1;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            33 => {
                candidate.output_store.ready_site.occurrence ^= 1;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            34 => {
                candidate.output_store.restore_site.occurrence ^= 1;
                "global-copy ABI exact global read/write readiness and SSA report"
            }
            _ => unreachable!(),
        };
        exact_failure(
            validate_prepared_abi_v21(checked, roots, &candidate, budget),
            expected,
        );
        assert_eq!(budget.storage(), floor);
        count += 1;
    }
    for case in 0..9 {
        let mut foreign = roots.to_vec();
        let root = &mut foreign[0];
        let mut arguments = root.arguments.as_slice().to_vec();
        let expected = match case {
            0 => {
                let mut binding = root.kernel_binding_bytes();
                binding[0] ^= 1;
                root.kernel_binding = KernelBindingIdV1::from_bytes(binding);
                "global-copy ABI exact source/canonical/descriptor association"
            }
            1 => {
                arguments[0].offset = 8;
                "global-copy ABI exact shared readonly input slice pair"
            }
            2 => {
                arguments[1].offset = 24;
                "global-copy ABI exact exclusive output slice pair"
            }
            3 => {
                arguments[0].access = AccessMode::ReadWrite;
                "global-copy ABI exact shared readonly input slice pair"
            }
            4 => {
                arguments[1].access = AccessMode::ReadOnly;
                "global-copy ABI exact exclusive output slice pair"
            }
            5 => {
                arguments[1].kind = DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32);
                "global-copy ABI exact exclusive output slice pair"
            }
            6 => {
                root.export_name.push_str("_foreign");
                "global-copy ABI exact source/canonical/descriptor association"
            }
            7 => {
                root.source_launch = None;
                "global-copy ABI retained validated source launch"
            }
            8 => {
                root.explicit_argument_bytes = 40;
                "global-copy ABI exact source/canonical/descriptor association"
            }
            _ => unreachable!(),
        };
        root.arguments = TypedArgumentListV1::new(arguments).unwrap();
        exact_failure(
            validate_prepared_abi_v21(checked, &foreign, abi, budget),
            expected,
        );
        assert_eq!(budget.storage(), floor);
        count += 1;
    }
    validate_prepared_abi_v21(checked, roots, abi, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(count, 44);
    count
}
