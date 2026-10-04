//! Controls invoked only with the actual rustc-derived checked target owner.
//! Copies below are test-only rejected candidates, not source-owner constructors.
use super::*;
fn denial_only_resource_controls(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    abi: &PhysicalEntryPreparedAbiV20,
) {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as WorkBudget;
    let owner_floor = checked.retained_storage();
    let complete_floor = owner_floor.checked_add(abi.retained_storage()).unwrap();
    // These isolated ledgers exercise rejection only. They never create a
    // source owner or a successfully admitted preparation, and the actual
    // caller retains its original cumulative ledger throughout.
    let mut work = WorkBudget::new(WORK);
    let mut missing_owner = Budget::new(&mut work, complete_floor);
    assert!(matches!(
        prepare_physical_entry_abi_v20(checked, roots, &mut missing_owner),
        Err(CompilerDescriptorError::PhysicalEntryResourceV20(
            Resource::Accounting
        ))
    ));
    assert_eq!(missing_owner.storage(), 0);
    assert_eq!(missing_owner.work(), 1);

    let mut work = WorkBudget::new(WORK);
    let mut missing_abi = Budget::new(&mut work, complete_floor);
    missing_abi.reserve_storage(owner_floor).unwrap();
    assert!(matches!(
        validate_prepared_abi_v20(checked, roots, abi, &mut missing_abi),
        Err(CompilerDescriptorError::PhysicalEntryResourceV20(
            Resource::Accounting
        ))
    ));
    assert_eq!(missing_abi.storage(), owner_floor);

    let mut work = WorkBudget::new(WORK);
    let mut short_storage = Budget::new(&mut work, complete_floor - 1);
    short_storage.reserve_storage(owner_floor).unwrap();
    assert!(matches!(
        prepare_physical_entry_abi_v20(checked, roots, &mut short_storage),
        Err(CompilerDescriptorError::PhysicalEntryResourceV20(
            Resource::Storage(_)
        ))
    ));
    assert_eq!(short_storage.storage(), owner_floor);

    let mut work = WorkBudget::new(WORK - 1);
    let mut short_work = Budget::new(&mut work, complete_floor);
    short_work.reserve_storage(complete_floor).unwrap();
    assert!(matches!(
        validate_prepared_abi_v20(checked, roots, abi, &mut short_work),
        Err(CompilerDescriptorError::PhysicalEntryResourceV20(
            Resource::Work(_)
        ))
    ));
    assert_eq!(short_work.storage(), complete_floor);
}

fn copied_abi(abi: &PhysicalEntryPreparedAbiV20) -> PhysicalEntryPreparedAbiV20 {
    PhysicalEntryPreparedAbiV20 {
        canonical: abi.canonical,
        semantic: abi.semantic,
        binding: abi.binding,
        reads: abi.reads.clone(),
        immutable_kernarg_required: abi.immutable_kernarg_required,
        output_disjoint_kernarg_required: abi.output_disjoint_kernarg_required,
        retained: abi.retained,
    }
}
fn exact_failure(result: Result<(), CompilerDescriptorError>, expected: &'static str) {
    assert!(
        matches!(result, Err(CompilerDescriptorError::ProductionDescriptorMismatch(field))
        if field == expected),
        "expected exact descriptor refusal {expected}"
    );
}
/// Returns the number of independently rejected preparation mutations. This
/// does not authenticate the test harness or confer artifact/runtime authority.
pub(crate) fn qualify_actual_owner_abi_controls_v20(
    checked: &Checked,
    roots: &[TypedDescriptorRootV1],
    abi: &PhysicalEntryPreparedAbiV20,
    budget: &mut Budget<'_>,
) -> usize {
    denial_only_resource_controls(checked, roots, abi);
    let floor = budget.storage();
    validate_prepared_abi_v20(checked, roots, abi, budget).unwrap();
    assert!(abi.reads.len() >= 2);
    let mut count = 0;
    for case in 0..12 {
        let mut candidate = copied_abi(abi);
        let expected = match case {
            0 => {
                candidate.reads.clear();
                "physical ABI exact ordered read report"
            }
            1 => {
                candidate.reads.pop();
                "physical ABI exact ordered read report"
            }
            2 => {
                candidate.reads.swap(0, 1);
                "physical ABI exact ordered read report"
            }
            3 => {
                candidate.reads[0].location = Location::new(
                    candidate.reads[0].location.block,
                    candidate.reads[0].location.operation_index + 1,
                );
                "physical ABI exact ordered read report"
            }
            4 => {
                candidate.reads[0].site.occurrence ^= 1;
                "physical ABI exact ordered read report"
            }
            5 => {
                candidate.reads[0].ready_at = Location::new(
                    candidate.reads[0].ready_at.block,
                    candidate.reads[0].ready_at.operation_index + 1,
                );
                "physical ABI exact ordered read report"
            }
            6 => {
                candidate.reads[0].ready_site.occurrence ^= 1;
                "physical ABI exact ordered read report"
            }
            7 => {
                candidate.canonical[0] ^= 1;
                "physical ABI retained source or unresolved conditions"
            }
            8 => {
                candidate.semantic[0] ^= 1;
                "physical ABI retained source or unresolved conditions"
            }
            9 => {
                candidate.binding[0] ^= 1;
                "physical ABI retained source or unresolved conditions"
            }
            10 => {
                candidate.immutable_kernarg_required = false;
                "physical ABI retained source or unresolved conditions"
            }
            11 => {
                candidate.output_disjoint_kernarg_required = false;
                "physical ABI retained source or unresolved conditions"
            }
            _ => unreachable!(),
        };
        exact_failure(
            validate_prepared_abi_v20(checked, roots, &candidate, budget),
            expected,
        );
        assert_eq!(budget.storage(), floor);
        count += 1;
    }
    for case in 0..6 {
        let mut foreign = roots.to_vec();
        let root = &mut foreign[0];
        let mut arguments = root.arguments.as_slice().to_vec();
        let expected = match case {
            0 => {
                let mut binding = root.kernel_binding_bytes();
                binding[0] ^= 1;
                root.kernel_binding = KernelBindingIdV1::from_bytes(binding);
                "physical ABI exact source/canonical/descriptor association"
            }
            1 => {
                arguments[1].offset = 20;
                "physical ABI exact u32 scalar slot"
            }
            2 => {
                arguments[0].access = AccessMode::ReadOnly;
                "physical ABI exact exclusive output slice pair"
            }
            3 => {
                arguments[0].kind = DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32);
                "physical ABI exact exclusive output slice pair"
            }
            4 => {
                root.export_name.push_str("_foreign");
                "physical ABI exact source/canonical/descriptor association"
            }
            5 => {
                root.source_launch = None;
                "physical ABI retained validated source launch"
            }
            _ => unreachable!(),
        };
        root.arguments = TypedArgumentListV1::new(arguments).unwrap();
        exact_failure(
            validate_prepared_abi_v20(checked, &foreign, abi, budget),
            expected,
        );
        assert_eq!(budget.storage(), floor);
        count += 1;
    }
    validate_prepared_abi_v20(checked, roots, abi, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(count, 18);
    count
}
