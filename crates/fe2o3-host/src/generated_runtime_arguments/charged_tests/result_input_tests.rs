use super::*;

pub(super) fn completed<T: GeneratedDeviceScalarV1>(
    values: Box<[T]>,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> ChargedTypedResultV1<T> {
    let buffers = result_buffers(&[&values]);
    let (output, mut observer) = GeneratedRuntimeReadWriteSlice::new_charged(values);
    decoder_after_input_disposal(prepare(vec![output], budget).unwrap())
        .decode_charged_buffers(buffers)
        .unwrap();
    observer.try_take().unwrap().unwrap()
}

fn prepare_read<T: GeneratedDeviceScalarV1>(
    input: GeneratedRuntimeReadSlice<T>,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> Result<GeneratedRuntimeChargedArgumentsV1, Error> {
    let plan = tests::plan::<T>(&[Access::ReadOnly], None);
    prepare_charged_with_plan(
        input,
        &plan,
        limits(),
        budget,
        |input, account| input.account_storage(account),
        |input, account| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![input.bind_argument(&plan, 0, account)?],
                ),
            )
        },
    )
}

#[test]
fn completed_input_conversion_preserves_storage_type_bits_and_credit_across_move() {
    fn check<T: GeneratedDeviceScalarV1>(values: Box<[T]>) {
        let bytes = words(&values);
        let budget = GeneratedRuntimeResultBudgetV1::new((bytes.len() * 2) as u64, 1).unwrap();
        let pointer = values.as_ptr() as usize;
        let result = completed(values, &budget);
        let before = budget.usage();
        let input = GeneratedRuntimeReadSlice::from_charged_result(result);
        assert_eq!(input.values.as_slice().as_ptr() as usize, pointer);
        assert_eq!(words(input.values.as_slice()), bytes);
        assert_eq!(input.len() * size_of::<T>(), bytes.len());
        assert_eq!(input.is_empty(), bytes.is_empty());
        assert_eq!(budget.usage(), before);
        let audit = budget.clone();
        std::thread::spawn(move || {
            assert_eq!(input.values.as_slice().as_ptr() as usize, pointer);
            assert_eq!(audit.usage(), before);
            drop(input);
            assert_empty(&audit);
        })
        .join()
        .unwrap();
        assert_empty(&budget);
    }
    check(vec![0u8, u8::MAX].into_boxed_slice());
    check(vec![i16::MIN, -1, i16::MAX].into_boxed_slice());
    check(vec![0u32, u32::MAX].into_boxed_slice());
    check(vec![i64::MIN, i64::MAX].into_boxed_slice());
    check(vec![f32::from_bits(0x7fc01234), -0.0, f32::INFINITY].into_boxed_slice());
    check(vec![f64::from_bits(0xfff8000000001234), -0.0].into_boxed_slice());
    check::<u32>(Box::new([]));
}

#[test]
fn completed_input_shared_budget_reserves_overlap_before_encoding() {
    let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
    let result = completed(vec![3u32, 7, 11, 19].into_boxed_slice(), &budget);
    let pointer = result.as_slice().as_ptr();
    let plan = tests::plan::<u32>(&[Access::ReadOnly], None);
    let prepared = prepare_charged_with_plan(
        GeneratedRuntimeReadSlice::from_charged_result(result),
        &plan,
        limits(),
        &budget,
        |input, account| {
            assert_eq!(input.values.as_slice().as_ptr(), pointer);
            assert_eq!(budget.usage().reserved_peak_bytes, 32);
            input.account_storage(account)
        },
        |input, account| {
            assert_eq!(input.values.as_slice().as_ptr(), pointer);
            assert_eq!(budget.usage().reserved_peak_bytes, 48);
            assert_eq!(budget.usage().retained_members, 1);
            assert_eq!(budget.usage().unissued_members, 1);
            let binding = input.bind_argument(&plan, 0, account)?;
            assert_eq!(budget.usage().reserved_peak_bytes, 16);
            assert_eq!(budget.usage().retained_members, 1);
            assert_eq!(budget.usage().unissued_members, 0);
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![binding],
                ),
            )
        },
    )
    .unwrap();
    let reference_budget = GeneratedRuntimeResultBudgetV1::new(16, 1).unwrap();
    let reference = prepare_read(
        GeneratedRuntimeReadSlice::new(vec![3u32, 7, 11, 19].into_boxed_slice()),
        &reference_budget,
    )
    .unwrap();
    assert_eq!(prepared.packed.buffers(), reference.packed.buffers());
    assert_eq!(
        prepared.packed.explicit_kernarg(),
        reference.packed.explicit_kernarg()
    );
    assert_eq!(
        prepared.packing_observation(),
        reference.packing_observation()
    );
    assert_eq!(prepared.footprint(), reference.footprint());
    assert_eq!(prepared.footprint().output_observers, 0);
    drop(reference);
    assert_empty(&reference_budget);
    decoder_after_input_disposal(prepared)
        .decode_charged_buffers(vec![(
            Gfx942RuntimeBufferAccessV1::ReadOnly,
            words(&[3u32, 7, 11, 19]),
        )])
        .unwrap();
    assert_empty(&budget);
}

#[test]
fn completed_input_overlap_exhaustion_rejects_before_binding() {
    for (bytes, members, expected) in [
        (47, 2, ResourceCreditErrorV1::Capacity),
        (48, 1, ResourceCreditErrorV1::RecordCapacity),
    ] {
        let budget = GeneratedRuntimeResultBudgetV1::new(bytes, members).unwrap();
        let result = completed(vec![1u32; 4].into_boxed_slice(), &budget);
        let before = budget.usage();
        let plan = tests::plan::<u32>(&[Access::ReadOnly], None);
        let result = prepare_charged_with_plan(
            GeneratedRuntimeReadSlice::from_charged_result(result),
            &plan,
            limits(),
            &budget,
            |input, account| {
                input.account_storage(account)?;
                assert_eq!(budget.usage(), before);
                Ok(())
            },
            |_, _| panic!("insufficient overlap must reject before binding"),
        );
        assert!(matches!(result, Err(Error::ResultCredit(error)) if error == expected));
        assert_empty(&budget);
    }
}

#[test]
fn completed_input_rejection_and_unwind_dispose_both_accounts() {
    for after_encoding in [false, true] {
        for panic in [false, true] {
            let source = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
            let next = GeneratedRuntimeResultBudgetV1::new(4, 1).unwrap();
            let input = GeneratedRuntimeReadSlice::from_charged_result(completed(
                vec![9u32].into_boxed_slice(),
                &source,
            ));
            let plan = tests::plan::<u32>(&[Access::ReadOnly], None);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                prepare_charged_with_plan(
                    input,
                    &plan,
                    limits(),
                    &next,
                    |input, account| input.account_storage(account),
                    |input, account| {
                        assert_eq!(source.usage().reserved_peak_bytes, 8);
                        assert_eq!(next.usage().reserved_peak_bytes, 4);
                        let _binding = if after_encoding {
                            let binding = input.bind_argument(&plan, 0, account)?;
                            assert_empty(&source);
                            Some(binding)
                        } else {
                            // Retain the input through the injected return or unwind.
                            let _input = input;
                            if panic {
                                std::panic::panic_any(73u32);
                            }
                            return Err(Error::Allocation);
                        };
                        if panic {
                            std::panic::panic_any(73u32);
                        }
                        Err(Error::Allocation)
                    },
                )
            }));
            if panic {
                assert_eq!(*result.err().unwrap().downcast::<u32>().unwrap(), 73);
            } else {
                assert!(matches!(result.unwrap(), Err(Error::Allocation)));
            }
            assert_empty(&source);
            assert_empty(&next);
        }
    }
}

#[test]
fn completed_input_payload_and_binding_mismatches_dispose_without_readiness() {
    for failure in 0..3 {
        let source = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
        let next = GeneratedRuntimeResultBudgetV1::new(4, 1).unwrap();
        let input = GeneratedRuntimeReadSlice::from_charged_result(completed(
            vec![9u32].into_boxed_slice(),
            &source,
        ));
        let plan = if failure == 2 {
            tests::plan::<u16>(&[Access::ReadOnly], None)
        } else {
            tests::plan::<u32>(&[Access::ReadOnly], None)
        };
        let bounds = if failure == 0 {
            GeneratedRuntimeArgumentLimitsV1::new(plan.kernarg_size() as usize * 2, 4096, 64)
        } else {
            limits()
        };
        let result = prepare_charged_with_plan(
            input,
            &plan,
            bounds,
            &next,
            |input, account| input.account_storage(account),
            |input, account| {
                assert_ne!(failure, 0, "payload limit is checked before binding");
                let index = usize::from(failure == 1);
                input.bind_argument(&plan, index, account)?;
                panic!("invalid binding accepted")
            },
        );
        if failure == 0 {
            assert!(matches!(result, Err(Error::PayloadLimit)));
        } else {
            assert!(matches!(result, Err(Error::Pack(_))));
        }
        assert_empty(&source);
        assert_empty(&next);
    }
}

#[test]
fn completed_input_empty_result_still_requires_a_successor_member() {
    for members in [1, 2] {
        let budget = GeneratedRuntimeResultBudgetV1::new(0, members).unwrap();
        let input =
            GeneratedRuntimeReadSlice::from_charged_result(completed::<u32>(Box::new([]), &budget));
        assert_eq!(budget.usage().retained_members, 1);
        let result = prepare_read(input, &budget);
        if members == 1 {
            assert!(matches!(
                result,
                Err(Error::ResultCredit(ResourceCreditErrorV1::RecordCapacity))
            ));
        } else {
            let prepared = result.unwrap();
            assert!(prepared.packed.buffers().is_empty());
            assert_eq!(budget.usage().retained_members, 1);
            decoder_after_input_disposal(prepared)
                .decode_charged_buffers(vec![])
                .unwrap();
        }
        assert_empty(&budget);
    }
}

#[test]
fn completed_input_legacy_packing_preserves_the_existing_uncharged_route() {
    let source = GeneratedRuntimeResultBudgetV1::new(16, 1).unwrap();
    let input = GeneratedRuntimeReadSlice::from_charged_result(completed(
        vec![3u32, 7].into_boxed_slice(),
        &source,
    ));
    let plan = tests::plan::<u32>(&[Access::ReadOnly], None);
    let mut budget = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits()).unwrap();
    input.account_storage(&mut budget).unwrap();
    assert_eq!(source.usage().reserved_peak_bytes, 16);
    let mut budget = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits()).unwrap();
    let binding = input.bind_argument(&plan, 0, &mut budget).unwrap();
    assert_empty(&source);
    let packed =
        GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], vec![binding])
            .pack(&plan, budget)
            .unwrap();
    let next = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
    let reference = prepare_read(
        GeneratedRuntimeReadSlice::new(vec![3u32, 7].into_boxed_slice()),
        &next,
    )
    .unwrap();
    assert_eq!(packed.buffers(), reference.packed.buffers());
    assert_eq!(
        packed.explicit_kernarg(),
        reference.packed.explicit_kernarg()
    );
    drop(packed);
    drop(reference);
    assert_empty(&next);
}

#[test]
fn completed_input_rejects_output_custody_and_mapping_before_accounting() {
    for case in 0..4 {
        let access = match case {
            0 => Gfx942RuntimeBufferAccessV1::WriteOnly,
            1 => Gfx942RuntimeBufferAccessV1::ReadWrite,
            _ => Gfx942RuntimeBufferAccessV1::ReadOnly,
        };
        let custody =
            (case == 2).then(|| OutputCustody::Charged(ChargedOutputCustodyV1::new::<u32>(1).0));
        let mapping = (case == 3).then_some(RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 1 });
        let source = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
        let values =
            OwnedRuntimeSliceV1::Completed(completed(vec![7u32].into_boxed_slice(), &source));
        let plan = tests::plan::<u32>(&[Access::ReadWrite], None);
        let mut budget = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits()).unwrap();
        let before = budget.footprint();
        assert!(matches!(
            bind_owned_slice(values, custody, &plan, 0, access, mapping, &mut budget),
            Err(Error::BindingMismatch)
        ));
        assert_eq!(budget.footprint(), before);
        assert_empty(&source);
    }
}
