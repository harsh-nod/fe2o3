use super::*;
use fe2o3_artifacts::Access;
use fe2o3_resource_accounting::ResourceCreditErrorV1;
use std::sync::Barrier;
use std::sync::atomic::{AtomicBool, Ordering};

type Error = GeneratedRuntimeArgumentErrorV1;

mod invocation_tests;

fn limits() -> GeneratedRuntimeArgumentLimitsV1 {
    GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 64)
}

fn decoder_after_input_disposal(
    prepared: GeneratedRuntimeChargedArgumentsV1,
) -> GeneratedRuntimeOutputDecoderV1 {
    let GeneratedRuntimePackedArgumentsV1 {
        packed, decoder, ..
    } = prepared.packed;
    drop(packed);
    decoder
}

fn prepare<T: GeneratedDeviceScalarV1>(
    outputs: Vec<GeneratedRuntimeReadWriteSlice<T>>,
    budget: &GeneratedRuntimeResultBudgetV1,
) -> Result<GeneratedRuntimeChargedArgumentsV1, Error> {
    let plan = tests::plan::<T>(&vec![Access::ReadWrite; outputs.len()], None);
    prepare_charged_with_plan(
        outputs,
        &plan,
        limits(),
        budget,
        |outputs, account| {
            for output in outputs {
                output.account_storage(account)?;
            }
            Ok(())
        },
        |outputs, account| {
            let bindings = outputs
                .into_iter()
                .enumerate()
                .map(|(index, output)| output.bind_argument(&plan, index, account))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], bindings))
        },
    )
}

fn words<T: GeneratedDeviceScalarV1>(values: &[T]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| {
            let (encoded, width) = value.encode_le_bytes_v1();
            encoded[..usize::from(width)].to_vec()
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
        .into_vec()
}

fn result_buffers<T: GeneratedDeviceScalarV1>(
    values: &[&[T]],
) -> Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)> {
    values
        .iter()
        .filter(|values| !values.is_empty())
        .map(|values| (Gfx942RuntimeBufferAccessV1::ReadWrite, words(values)))
        .collect()
}

fn assert_empty(budget: &GeneratedRuntimeResultBudgetV1) {
    let usage = budget.usage();
    assert_eq!(usage.reserved_peak_bytes, 0);
    assert_eq!(usage.unissued_members, 0);
    assert_eq!(usage.retained_members, 0);
    assert_eq!(usage.quarantined_members, 0);
    assert!(!usage.poisoned);
}

#[test]
fn charged_preparation_reuses_seed_and_preserves_exact_packing() {
    let seed = vec![3u32, 7, 11, 19].into_boxed_slice();
    let address = seed.as_ptr();
    let (output, mut observer) = GeneratedRuntimeReadWriteSlice::new_charged(seed);
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
    let prepared = prepare(vec![output], &budget).unwrap();
    assert_eq!(prepared.footprint().result_bytes, 32);
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    assert_eq!(budget.usage().retained_members, 1);
    assert!(observer.try_take().unwrap().is_none());
    let plan = tests::plan::<u32>(&[Access::ReadWrite], None);
    let (legacy, _) = GeneratedRuntimeReadWriteSlice::new(vec![3u32, 7, 11, 19].into_boxed_slice());
    let mut legacy_budget = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits()).unwrap();
    let binding = legacy.bind_argument(&plan, 0, &mut legacy_budget).unwrap();
    let legacy =
        GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], vec![binding])
            .pack(&plan, legacy_budget)
            .unwrap();
    assert_eq!(prepared.packed.buffers(), legacy.buffers());
    assert_eq!(
        prepared.packed.explicit_kernarg(),
        legacy.explicit_kernarg()
    );
    assert_eq!(prepared.packing_observation(), legacy.packing_observation());
    decoder_after_input_disposal(prepared)
        .decode_charged_buffers(result_buffers(&[&[5u32, 9, 13, 21]]))
        .unwrap();
    let result = observer.try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[5, 9, 13, 21]);
    assert_eq!(result.as_slice().as_ptr(), address);
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    drop(result);
    assert_empty(&budget);
}

#[test]
fn full_roster_exhaustion_prevents_binding_every_earlier_seed() {
    for (bytes, members, expected) in [
        (15, 2, ResourceCreditErrorV1::Capacity),
        (16, 1, ResourceCreditErrorV1::RecordCapacity),
    ] {
        let budget = GeneratedRuntimeResultBudgetV1::new(bytes, members).unwrap();
        let before = budget.usage();
        let (first, mut first_observer) =
            GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
        let (second, mut second_observer) =
            GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32].into_boxed_slice());
        let bound = AtomicBool::new(false);
        let plan = tests::plan::<u32>(&[Access::ReadWrite; 2], None);
        let result = prepare_charged_with_plan(
            vec![first, second],
            &plan,
            limits(),
            &budget,
            |outputs, preflight| {
                for output in outputs {
                    output.account_storage(preflight)?;
                }
                Ok(())
            },
            |_, _| {
                bound.store(true, Ordering::SeqCst);
                unreachable!()
            },
        );
        assert!(matches!(result, Err(Error::ResultCredit(error)) if error == expected));
        assert!(!bound.load(Ordering::SeqCst));
        assert_eq!(budget.usage(), before);
        assert!(matches!(
            first_observer.try_take(),
            Err(Error::OutputUnavailable)
        ));
        assert!(matches!(
            second_observer.try_take(),
            Err(Error::OutputUnavailable)
        ));
    }
}

#[test]
fn both_preparation_routes_reject_mixed_custody_before_encoding() {
    let plan = tests::plan::<u32>(&[Access::ReadWrite], None);
    let (output, _) = GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let mut legacy = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits()).unwrap();
    assert!(matches!(
        output.account_storage(&mut legacy),
        Err(Error::BindingMismatch)
    ));
    assert!(matches!(
        output.bind_argument(&plan, 0, &mut legacy),
        Err(Error::BindingMismatch)
    ));
    let budget = GeneratedRuntimeResultBudgetV1::new(16, 2).unwrap();
    let (first, _) = GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let (second, _) = GeneratedRuntimeReadWriteSlice::new(vec![2u32].into_boxed_slice());
    assert!(matches!(
        prepare(vec![first, second], &budget),
        Err(Error::BindingMismatch)
    ));
    assert_empty(&budget);
}

#[test]
fn failed_or_panicking_binding_disposes_prepared_and_unissued_members() {
    for panic in [false, true] {
        let budget = GeneratedRuntimeResultBudgetV1::new(16, 2).unwrap();
        let (first, mut observer) =
            GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
        let (second, _) =
            GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32].into_boxed_slice());
        let plan = tests::plan::<u32>(&[Access::ReadWrite; 2], None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepare_charged_with_plan(
                vec![first, second],
                &plan,
                limits(),
                &budget,
                |outputs, preflight| {
                    for output in outputs {
                        output.account_storage(preflight)?;
                    }
                    Ok(())
                },
                |outputs, binding| {
                    let mut outputs = outputs.into_iter();
                    let _first = outputs.next().unwrap().bind_argument(&plan, 0, binding)?;
                    assert_eq!(budget.usage().retained_members, 1);
                    assert_eq!(budget.usage().unissued_members, 1);
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
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
        assert_empty(&budget);
    }
}

#[test]
fn charged_try_take_does_not_wait_for_a_locked_seed() {
    let budget = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
    let (output, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let decoder = decoder_after_input_disposal(prepare(vec![output], &budget).unwrap());
    let Some(OutputCustody::Charged(custody)) = &decoder.expectations[0].custody else {
        unreachable!()
    };
    let (mut observer, polled, retained) = std::thread::scope(|scope| {
        let (locked_tx, locked_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let (polled_tx, polled_rx) = std::sync::mpsc::channel();
        let holder = scope.spawn(move || {
            custody
                .with_seed::<u32, ()>(|_| {
                    locked_tx.send(()).unwrap();
                    let _ = release_rx.recv();
                    Ok(())
                })
                .unwrap();
        });
        locked_rx.recv().unwrap();
        let poller = scope.spawn(move || {
            polled_tx
                .send(matches!(observer.try_take(), Ok(None)))
                .unwrap();
            observer
        });
        let polled = polled_rx.recv_timeout(std::time::Duration::from_secs(10));
        let retained = budget.usage();
        // Release on timeout or unwind before joining, so blocking regressions cannot hang.
        drop(release_tx);
        holder.join().unwrap();
        (poller.join().unwrap(), polled, retained)
    });
    assert!(
        matches!(polled, Ok(true)),
        "charged result polling waited for the seed lock: {polled:?}"
    );
    assert_eq!(retained.reserved_peak_bytes, 8);
    assert_eq!(retained.retained_members, 1);
    decoder
        .decode_charged_buffers(result_buffers(&[&[9u32]]))
        .unwrap();
    let result = observer.try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[9]);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    drop(result);
    assert_empty(&budget);
}

#[test]
fn poisoned_seed_custody_rejects_decode_and_refunds_after_abandonment() {
    let budget = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
    let (output, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let decoder = decoder_after_input_disposal(prepare(vec![output], &budget).unwrap());
    let Some(OutputCustody::Charged(custody)) = &decoder.expectations[0].custody else {
        unreachable!()
    };
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        custody.with_seed::<u32, ()>(|_| std::panic::panic_any(73u64))
    }));
    assert_eq!(*panic.unwrap_err().downcast::<u64>().unwrap(), 73);
    assert_eq!(budget.usage().reserved_peak_bytes, 8);
    assert!(matches!(observer.try_take(), Err(Error::Custody)));
    assert!(matches!(
        decoder.decode_charged_buffers(result_buffers(&[&[9u32]])),
        Err(Error::Custody)
    ));
    assert!(matches!(observer.try_take(), Err(Error::Custody)));
    assert_empty(&budget);
}

#[test]
fn lower_pack_rejection_disposes_all_bound_outputs_and_credits() {
    let budget = GeneratedRuntimeResultBudgetV1::new(16, 2).unwrap();
    let (first, mut a) = GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let (second, mut b) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32].into_boxed_slice());
    let plan = tests::plan::<u32>(&[Access::ReadWrite; 2], None);
    let result = prepare_charged_with_plan(
        vec![first, second],
        &plan,
        limits(),
        &budget,
        |outputs, preflight| {
            for output in outputs {
                output.account_storage(preflight)?;
            }
            Ok(())
        },
        |outputs, account| {
            let bindings = outputs
                .into_iter()
                .map(|output| output.bind_argument(&plan, 0, account))
                .collect::<Result<Vec<_>, _>>()?;
            assert_eq!(budget.usage().retained_members, 2);
            assert_eq!(budget.usage().unissued_members, 0);
            Ok(GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], bindings))
        },
    );
    assert!(matches!(
        result,
        Err(Error::Binding(GeneratedKfdArgumentError::Pack(
            GeneratedArgumentPackError::DuplicateArgument { argument_index: 0 }
        )))
    ));
    assert!(matches!(a.try_take(), Err(Error::OutputUnavailable)));
    assert!(matches!(b.try_take(), Err(Error::OutputUnavailable)));
    assert_empty(&budget);
}

#[test]
fn late_shape_or_access_or_capacity_error_never_publishes_earlier_outputs() {
    for mutation in 0..4 {
        let budget = GeneratedRuntimeResultBudgetV1::new(16, 2).unwrap();
        let (first, mut a) =
            GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
        let (second, mut b) =
            GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32].into_boxed_slice());
        let prepared = prepare(vec![first, second], &budget).unwrap();
        let mut buffers = result_buffers(&[&[3u32], &[4u32]]);
        match mutation {
            0 => buffers[1].1.pop().map(|_| ()).unwrap(),
            1 => buffers[1].0 = Gfx942RuntimeBufferAccessV1::WriteOnly,
            2 => buffers[1].1.reserve_exact(1),
            _ => {
                buffers.pop();
            }
        }
        assert!(matches!(
            decoder_after_input_disposal(prepared).decode_charged_buffers(buffers),
            Err(Error::BindingMismatch)
        ));
        assert!(matches!(a.try_take(), Err(Error::OutputUnavailable)));
        assert!(matches!(b.try_take(), Err(Error::OutputUnavailable)));
        assert_empty(&budget);
    }
}

#[test]
fn concurrent_observers_cannot_extract_a_partially_decoded_roster() {
    let budget = GeneratedRuntimeResultBudgetV1::new(16, 2).unwrap();
    let (first, mut a) = GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let (second, mut b) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32].into_boxed_slice());
    let prepared = prepare(vec![first, second], &budget).unwrap();
    let decoded_first = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    let start = decoded_first.clone();
    let proceed = resume.clone();
    let worker = std::thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            decoder_after_input_disposal(prepared).decode_charged_with(
                result_buffers(&[&[3u32], &[4u32]]),
                |index| {
                    if index == 0 {
                        start.wait();
                        proceed.wait();
                    }
                    if index == 1 {
                        std::panic::panic_any(73u32);
                    }
                },
            )
        }))
    });
    decoded_first.wait();
    assert!(a.try_take().unwrap().is_none());
    assert!(b.try_take().unwrap().is_none());
    assert_eq!(budget.usage().reserved_peak_bytes, 16);
    resume.wait();
    assert_eq!(
        *worker
            .join()
            .unwrap()
            .err()
            .unwrap()
            .downcast::<u32>()
            .unwrap(),
        73
    );
    assert!(matches!(a.try_take(), Err(Error::OutputUnavailable)));
    assert!(matches!(b.try_take(), Err(Error::OutputUnavailable)));
    assert_empty(&budget);
}

#[test]
fn extracted_result_outlives_joined_producer_and_all_other_owners() {
    let budget = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
    let audit = budget.clone();
    let (output, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let prepared = prepare(vec![output], &budget).unwrap();
    let worker = std::thread::spawn(move || {
        decoder_after_input_disposal(prepared).decode_charged_buffers(result_buffers(&[&[9u32]]))
    });
    worker.join().unwrap().unwrap();
    let result = observer.try_take().unwrap().unwrap();
    drop((observer, budget));
    assert_eq!(audit.usage().reserved_peak_bytes, 8);
    assert_eq!(result.as_slice(), &[9]);
    drop(result);
    assert_empty(&audit);
}

#[test]
fn abandoned_observer_retains_seed_until_producer_disposal() {
    let budget = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
    let (output, observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let prepared = prepare(vec![output], &budget).unwrap();
    drop(observer);
    assert_eq!(budget.usage().reserved_peak_bytes, 8);
    drop(prepared);
    assert_empty(&budget);
}

#[test]
fn empty_typed_output_owns_a_member_and_read_only_returns_have_their_own_charge() {
    let budget = GeneratedRuntimeResultBudgetV1::new(0, 1).unwrap();
    let (output, mut observer) = GeneratedRuntimeReadWriteSlice::<u32>::new_charged(Box::new([]));
    let prepared = prepare(vec![output], &budget).unwrap();
    assert_eq!(budget.usage().retained_members, 1);
    decoder_after_input_disposal(prepared)
        .decode_charged_buffers(vec![])
        .unwrap();
    let result = observer.try_take().unwrap().unwrap();
    assert!(result.is_empty());
    assert_eq!(budget.usage().retained_members, 1);
    drop(result);
    assert_empty(&budget);

    let budget = GeneratedRuntimeResultBudgetV1::new(12, 2).unwrap();
    let source = GeneratedRuntimeReadSlice::new(vec![1u32].into_boxed_slice());
    let (output, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32].into_boxed_slice());
    let plan = tests::plan::<u32>(&[Access::ReadOnly, Access::ReadWrite], None);
    let prepared = prepare_charged_with_plan(
        (source, output),
        &plan,
        limits(),
        &budget,
        |(source, output), account| {
            source.account_storage(account)?;
            output.account_storage(account)
        },
        |(source, output), account| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![
                        source.bind_argument(&plan, 0, account)?,
                        output.bind_argument(&plan, 1, account)?,
                    ],
                ),
            )
        },
    )
    .unwrap();
    assert_eq!(budget.usage().reserved_peak_bytes, 12);
    assert_eq!(prepared.footprint().result_bytes, 12);
    decoder_after_input_disposal(prepared)
        .decode_charged_buffers(vec![
            (Gfx942RuntimeBufferAccessV1::ReadOnly, words(&[1u32])),
            (Gfx942RuntimeBufferAccessV1::ReadWrite, words(&[5u32])),
        ])
        .unwrap();
    assert_eq!(budget.usage().reserved_peak_bytes, 8);
    drop(observer.try_take().unwrap().unwrap());
    assert_empty(&budget);
}

#[test]
fn charged_decoder_cannot_be_used_as_legacy_data_decoder() {
    let budget = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
    let (output, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let prepared = prepare(vec![output], &budget).unwrap();
    assert!(matches!(
        decoder_after_input_disposal(prepared).decode_buffers(result_buffers(&[&[3u32]])),
        Err(Error::BindingMismatch)
    ));
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    assert_empty(&budget);
}

#[test]
fn every_sealed_scalar_decodes_into_the_original_typed_extent() {
    fn round_trip<T: GeneratedDeviceScalarV1>(expected: Vec<T>) {
        let bytes = expected.len() * size_of::<T>();
        let (output, mut observer) =
            GeneratedRuntimeReadWriteSlice::new_charged(expected.clone().into_boxed_slice());
        let budget = GeneratedRuntimeResultBudgetV1::new((bytes * 2) as u64, 1).unwrap();
        let prepared = prepare(vec![output], &budget).unwrap();
        decoder_after_input_disposal(prepared)
            .decode_charged_buffers(result_buffers(&[&expected]))
            .unwrap();
        let result = observer.try_take().unwrap().unwrap();
        assert_eq!(words(result.as_slice()), words(&expected));
        drop(result);
        assert_empty(&budget);
    }
    round_trip(vec![i8::MIN, 0, i8::MAX]);
    round_trip(vec![u8::MIN, u8::MAX]);
    round_trip(vec![i16::MIN, i16::MAX]);
    round_trip(vec![u16::MIN, u16::MAX]);
    round_trip(vec![i32::MIN, i32::MAX]);
    round_trip(vec![u32::MIN, u32::MAX]);
    round_trip(vec![i64::MIN, i64::MAX]);
    round_trip(vec![u64::MIN, u64::MAX]);
    round_trip(vec![
        f32::from_bits(0x7fc00073),
        -0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
    ]);
    round_trip(vec![
        f64::from_bits(0x7ff8000000000073),
        -0.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ]);
}

#[test]
fn output_members_dispose_independently_without_partial_peak_refunds() {
    let budget = GeneratedRuntimeResultBudgetV1::new(24, 2).unwrap();
    let (first, mut a) = GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let (second, mut b) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32, 3].into_boxed_slice());
    decoder_after_input_disposal(prepare(vec![first, second], &budget).unwrap())
        .decode_charged_buffers(result_buffers(&[&[4u32], &[5u32, 6]]))
        .unwrap();
    let first = a.try_take().unwrap().unwrap();
    let second = b.try_take().unwrap().unwrap();
    drop((a, b));
    assert_eq!(budget.usage().reserved_peak_bytes, 24);
    drop(second);
    assert_eq!(budget.usage().reserved_peak_bytes, 8);
    assert_eq!(budget.usage().retained_members, 1);
    drop(first);
    assert_empty(&budget);
}

#[test]
fn changed_accounting_roster_or_private_seed_scalar_rejects() {
    let budget = GeneratedRuntimeResultBudgetV1::new(16, 2).unwrap();
    let plan = tests::plan::<u32>(&[Access::ReadWrite; 2], None);
    let (first, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let (second, _) = GeneratedRuntimeReadWriteSlice::new_charged(vec![2u32].into_boxed_slice());
    let result = prepare_charged_with_plan(
        (first, second),
        &plan,
        limits(),
        &budget,
        |(first, second), account| {
            first.account_storage(account)?;
            second.account_storage(account)
        },
        |(first, second), account| {
            let second = second.bind_argument(&plan, 0, account)?;
            let first = first.bind_argument(&plan, 1, account)?;
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![second, first],
                ),
            )
        },
    );
    assert!(matches!(result, Err(Error::BindingMismatch)));
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    assert_empty(&budget);

    let (output, _) = GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let substituted = GeneratedRuntimeReadWriteSlice {
        values: vec![1f32].into_boxed_slice(),
        custody: output.custody,
    };
    assert!(matches!(
        prepare(vec![substituted], &budget),
        Err(Error::BindingMismatch)
    ));
    assert_empty(&budget);
}

#[test]
fn zero_buffer_preparation_bypasses_the_nonempty_batch_api() {
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, AddressSpace, AliasClass, ArgumentOwnership, Mutability,
        Name, PointerWidth,
    };
    let field = AbiField::new(
        Name::new("value").unwrap(),
        0,
        4,
        4,
        AbiKind::Scalar(u32::ABI_SCALAR_TYPE),
        Mutability::Immutable,
        Access::ByValue,
        AddressSpace::Value,
        u32::scalar_type_identity_v1(PointerWidth::Bits64),
        ArgumentOwnership::ByValue,
        AliasClass::Value,
    )
    .unwrap();
    let abi = AbiLayout::new(4, 4, PointerWidth::Bits64, vec![field.clone()]).unwrap();
    let layout = CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
        4,
        4,
        PointerWidth::Bits64,
        vec![field],
        vec![None],
    )
    .unwrap();
    let plan = crate::generated_argument_plan::validate_argument_packing(
        KernelId::from_bytes([73; 32]),
        &abi,
        &layout,
    )
    .unwrap();
    let budget = GeneratedRuntimeResultBudgetV1::new(0, 1).unwrap();
    let prepared = prepare_charged_with_plan(
        3u32,
        &plan,
        limits(),
        &budget,
        |_, _| Ok(()),
        |value, _| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![plan.scalar(0, value).map_err(Error::Pack)?],
                    vec![],
                ),
            )
        },
    )
    .unwrap();
    assert_eq!(prepared.footprint().result_bytes, 0);
    decoder_after_input_disposal(prepared)
        .decode_charged_buffers(vec![])
        .unwrap();
    assert_empty(&budget);
}

fn prepare_read_only(
    budget: &GeneratedRuntimeResultBudgetV1,
) -> GeneratedRuntimeChargedArgumentsV1 {
    let plan = tests::plan::<u32>(&[Access::ReadOnly], None);
    prepare_charged_with_plan(
        GeneratedRuntimeReadSlice::new(vec![1u32].into_boxed_slice()),
        &plan,
        limits(),
        budget,
        |source, budget| source.account_storage(budget),
        |source, budget| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![source.bind_argument(&plan, 0, budget)?],
                ),
            )
        },
    )
    .unwrap()
}

#[test]
fn equal_capacity_foreign_read_member_cannot_cross_invocation_gates() {
    let first_budget = GeneratedRuntimeResultBudgetV1::new(4, 1).unwrap();
    let second_budget = GeneratedRuntimeResultBudgetV1::new(4, 1).unwrap();
    let mut first = prepare_read_only(&first_budget);
    let mut second = prepare_read_only(&second_budget);
    std::mem::swap(
        &mut first.packed.decoder.expectations[0].read_credit,
        &mut second.packed.decoder.expectations[0].read_credit,
    );
    let returned = vec![(Gfx942RuntimeBufferAccessV1::ReadOnly, words(&[1u32]))];
    assert!(matches!(
        decoder_after_input_disposal(first).decode_charged_buffers(returned),
        Err(Error::BindingMismatch)
    ));
    drop(second);
    assert_empty(&first_budget);
    assert_empty(&second_budget);
}

#[test]
fn public_pack_cannot_shed_the_charged_route() {
    let budget = GeneratedRuntimeResultBudgetV1::new(8, 1).unwrap();
    let plan = tests::plan::<u32>(&[Access::ReadWrite], None);
    let (output, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32].into_boxed_slice());
    let result = prepare_charged_with_plan(
        output,
        &plan,
        limits(),
        &budget,
        |output, account| output.account_storage(account),
        |output, account| {
            let binding = output.bind_argument(&plan, 0, account)?;
            let retained = std::mem::replace(
                account,
                GeneratedRuntimeArgumentBudgetV1::new(&plan, limits())?,
            );
            assert!(matches!(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![binding]
                )
                .pack(&plan, retained),
                Err(Error::BindingMismatch)
            ));
            Err(Error::BindingMismatch)
        },
    );
    assert!(matches!(result, Err(Error::BindingMismatch)));
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    assert_empty(&budget);
}

#[test]
fn charged_write_only_mapped_output_uses_the_existing_binding_rules() {
    let mapping = RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 1 };
    let plan = tests::plan::<u16>(&[Access::WriteOnly], Some(mapping));
    let (output, mut observer) =
        GeneratedRuntimeWriteSlice::new_charged(vec![1u16, 2, 3].into_boxed_slice());
    let budget = GeneratedRuntimeResultBudgetV1::new(12, 1).unwrap();
    let prepared = prepare_charged_with_plan(
        output,
        &plan,
        limits(),
        &budget,
        |output, account| output.account_storage(account),
        |output, account| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![output.bind_mapped_argument(&plan, 0, mapping, account)?],
                ),
            )
        },
    )
    .unwrap();
    decoder_after_input_disposal(prepared)
        .decode_charged_buffers(vec![(
            Gfx942RuntimeBufferAccessV1::WriteOnly,
            words(&[1u16, 8, 9]),
        )])
        .unwrap();
    let result = observer.try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[1, 8, 9]);
    drop(result);
    assert_empty(&budget);
}
