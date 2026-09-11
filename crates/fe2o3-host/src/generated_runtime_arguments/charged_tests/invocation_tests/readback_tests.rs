use super::*;

fn projected(
    budget: &GeneratedRuntimeResultBudgetV1,
) -> (
    GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
    GeneratedRuntimeChargedResultV1<u32>,
) {
    let (parts, observer) = input_parts(budget);
    let hsaco = synthetic_cov6::preparation_module();
    (
        parts
            .storage
            .prepare(&hsaco, "vecadd")
            .unwrap()
            .project_persistent(&hsaco)
            .unwrap(),
        observer,
    )
}

#[test]
fn readback_uses_original_account_and_preserves_prepared_source() {
    let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
    let shared = budget.clone();
    let unrelated = GeneratedRuntimeResultBudgetV1::new(4096, 16).unwrap();
    let (mut storage, mut observer) = projected(&budget);
    let pointer = storage.prepared().buffers()[0].bytes().as_ptr();
    let bytes = storage.prepared().buffers()[0].bytes().to_vec();
    let staged = storage.prepare_readback().unwrap();
    assert_eq!(shared.usage().reserved_peak_bytes, 48);
    assert_eq!(shared.usage().retained_members, 2);
    assert_eq!(staged.buffers()[0].1, vec![0; 16]);
    assert_empty(&unrelated);
    assert!(storage.readback.is_none());
    storage.install_readback(staged);
    assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), pointer);
    assert_eq!(storage.prepared().buffers()[0].bytes(), bytes);
    assert!(observer.try_take().unwrap().is_none());
    let usage = budget.usage();
    assert!(matches!(
        storage.prepare_readback(),
        Err(Error::StaleOrAliasedOutput)
    ));
    assert_eq!(budget.usage(), usage);
    drop(storage);
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn readback_byte_and_record_exhaustion_preserve_original_custody() {
    for (bytes, members, expected) in [
        (47, 2, ResourceCreditErrorV1::Capacity),
        (48, 1, ResourceCreditErrorV1::RecordCapacity),
    ] {
        let budget = GeneratedRuntimeResultBudgetV1::new(bytes, members).unwrap();
        let (storage, mut observer) = projected(&budget);
        let usage = budget.usage();
        assert!(
            matches!(storage.prepare_readback(), Err(Error::ResultCredit(error)) if error == expected)
        );
        assert_eq!(budget.usage(), usage);
        assert!(storage.readback.is_none());
        assert!(observer.try_take().unwrap().is_none());
        drop(storage);
        assert_empty(&budget);
    }
}

#[test]
fn readback_staged_rejection_refunds_only_disposed_overlap_and_allows_retry() {
    let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
    let (mut storage, mut observer) = projected(&budget);
    let original = budget.usage();
    drop(storage.prepare_readback().unwrap());
    assert_eq!(budget.usage(), original);
    assert!(observer.try_take().unwrap().is_none());
    let staged = storage.prepare_readback().unwrap();
    storage.install_readback(staged);
    drop(observer);
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn readback_installed_and_staged_unwind_dispose_all_host_storage() {
    for install in [false, true] {
        let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
        let (mut storage, mut observer) = projected(&budget);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let staged = storage.prepare_readback().unwrap();
            if install {
                storage.install_readback(staged);
            }
            let _storage = storage;
            panic!("after reservation");
        }));
        assert!(result.is_err());
        assert_empty(&budget);
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    }
}

fn mixed(
    budget: &GeneratedRuntimeResultBudgetV1,
) -> (
    GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
    GeneratedRuntimeChargedResultV1<u32>,
) {
    mixed_with_read_length(budget, 4)
}

#[test]
fn generated_storage_mutable_view_preserves_charged_full_roster_and_readback() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, observer) = mixed(&budget);
    let original = storage
        .prepared()
        .buffers()
        .iter()
        .map(|buffer| (buffer.bytes().as_ptr(), buffer.bytes().to_vec()))
        .collect::<Vec<_>>();
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    assert_eq!(storage.prepared_mut().buffers().len(), 2);
    assert_eq!(
        storage.prepared().buffer_access(1),
        Some(Gfx942RuntimeBufferAccessV1::ReadOnly)
    );
    let readback = storage.prepare_readback().unwrap();
    let destinations = readback
        .buffers()
        .iter()
        .map(|(_, bytes)| bytes.as_ptr())
        .collect::<Vec<_>>();
    storage.install_readback(readback);
    for (buffer, (pointer, bytes)) in storage.prepared_mut().buffers().iter().zip(original) {
        assert_eq!(buffer.bytes().as_ptr(), pointer);
        assert_eq!(buffer.bytes(), bytes);
    }
    assert_eq!(
        storage
            .readback
            .as_ref()
            .unwrap()
            .buffers()
            .iter()
            .map(|(_, bytes)| bytes.as_ptr())
            .collect::<Vec<_>>(),
        destinations
    );
    assert_eq!(budget.usage().reserved_peak_bytes, 80);
    drop(observer);
    assert_eq!(budget.usage().reserved_peak_bytes, 80);
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn generated_storage_with_readback_disposes_payload_before_refund_on_drop_and_unwind() {
    for unwind in [false, true] {
        let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
        let (mut storage, mut observer) = mixed(&budget);
        storage.install_readback(storage.prepare_readback().unwrap());
        let disposed = Arc::new(AtomicBool::new(false));
        let witnessed = GeneratedRuntimeStorageV1 {
            payload: DisposalWitness {
                payload: Some(storage.payload),
                budget: budget.clone(),
                disposed: Arc::clone(&disposed),
                expected_bytes: 80,
            },
            readback: storage.readback,
            decoder: storage.decoder,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _storage = witnessed;
            if unwind {
                panic!("after complete charged readback reservation");
            }
        }));
        assert_eq!(result.is_err(), unwind);
        assert!(disposed.load(Ordering::SeqCst));
        assert_empty(&budget);
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    }
}

fn mixed_with_read_length(
    budget: &GeneratedRuntimeResultBudgetV1,
    elements: usize,
) -> (
    GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
    GeneratedRuntimeChargedResultV1<u32>,
) {
    let plan = tests::plan::<u32>(&[Access::ReadWrite, Access::ReadOnly], None);
    let (output, observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(vec![1u32; 4].into_boxed_slice());
    let input = GeneratedRuntimeReadSlice::new(vec![2u32; elements].into_boxed_slice());
    let packed = prepare_charged_with_plan(
        (output, input),
        &plan,
        limits(),
        budget,
        |(output, input), account| {
            output.account_storage(account)?;
            input.account_storage(account)
        },
        |(output, input), account| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![
                        output.bind_argument(&plan, 0, account)?,
                        input.bind_argument(&plan, 1, account)?,
                    ],
                ),
            )
        },
    )
    .unwrap();
    // The structural fixture uses only buffer 0. Keep every original encoded
    // buffer and the real charged decoder; this is not compiler authority.
    let GeneratedRuntimePackedArgumentsV1 {
        packed, decoder, ..
    } = packed.packed;
    let mut explicit = vec![0; 16];
    explicit[8..].copy_from_slice(&4u64.to_le_bytes());
    let storage = GeneratedRuntimeStorageV1 {
        payload: Gfx942RuntimeDispatchInputsV1::new(
            explicit,
            packed.buffers,
            vec![fe2o3_kfd::Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4)],
            geometry(),
            0,
            1000,
        ),
        readback: None,
        decoder,
    };
    let hsaco = synthetic_cov6::preparation_module();
    (
        storage
            .prepare(&hsaco, "vecadd")
            .unwrap()
            .project_persistent(&hsaco)
            .unwrap(),
        observer,
    )
}

#[test]
fn readback_zero_length_expectation_does_not_add_a_native_buffer() {
    let budget = GeneratedRuntimeResultBudgetV1::new(48, 3).unwrap();
    let (storage, mut observer) = mixed_with_read_length(&budget, 0);
    assert_eq!(storage.decoder.expectations.len(), 2);
    assert_eq!(storage.prepared().buffers().len(), 1);
    let staged = storage.prepare_readback().unwrap();
    assert_eq!(staged.buffers().len(), 1);
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    assert!(observer.try_take().unwrap().is_none());
    drop(staged);
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn readback_includes_unused_read_only_buffers_and_exact_access() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (storage, mut observer) = mixed(&budget);
    assert_eq!(storage.prepared().pointer_fixups().len(), 1);
    assert_eq!(storage.prepared().buffers().len(), 2);
    let staged = storage.prepare_readback().unwrap();
    assert_eq!(staged.buffers().len(), 2);
    assert_eq!(
        staged.buffers()[0].0,
        Gfx942RuntimeBufferAccessV1::ReadWrite
    );
    assert_eq!(staged.buffers()[1].0, Gfx942RuntimeBufferAccessV1::ReadOnly);
    assert_eq!(budget.usage().reserved_peak_bytes, 80);
    drop(staged);
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    assert!(observer.try_take().unwrap().is_none());
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn readback_late_allocation_failure_and_panic_restore_full_account() {
    for fail_at in 0..2 {
        for panic in [false, true] {
            let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
            let (storage, mut observer) = mixed(&budget);
            let before = budget.usage();
            let mut ordinal = 0;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                storage.prepare_readback_with(|length| {
                    assert_eq!(budget.usage().reserved_peak_bytes, 80);
                    assert_eq!(budget.usage().unissued_members, 1);
                    if ordinal == fail_at {
                        assert!(!panic, "injected destination allocation panic");
                        return Err(Error::Allocation);
                    }
                    ordinal += 1;
                    Ok(vec![0; length])
                })
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(matches!(result, Ok(Err(Error::Allocation))));
            }
            assert_eq!(budget.usage(), before);
            assert!(storage.readback.is_none());
            assert!(observer.try_take().unwrap().is_none());
            drop(storage);
            assert_empty(&budget);
        }
    }
}

#[test]
fn readback_rejects_bad_destination_lengths_and_excess_capacity() {
    for shape in 0..3 {
        let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
        let (storage, _observer) = projected(&budget);
        let before = budget.usage();
        let result = storage.prepare_readback_with(|length| {
            Ok(match shape {
                0 => vec![0; length - 1],
                1 => vec![0; length + 1],
                _ => {
                    let mut bytes = Vec::with_capacity(length + 1);
                    bytes.resize(length, 0);
                    bytes
                }
            })
        });
        assert!(matches!(result, Err(Error::BindingMismatch)));
        assert_eq!(budget.usage(), before);
    }
}

#[test]
fn readback_rejects_decoder_length_access_count_and_gate_substitution() {
    for field in 0..4 {
        let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
        let (mut storage, _observer) = projected(&budget);
        let before = budget.usage();
        match field {
            0 => storage.decoder.expectations[0].byte_len += 1,
            1 => storage.decoder.expectations[0].access = Gfx942RuntimeBufferAccessV1::ReadOnly,
            2 => storage.decoder.expectations[0].byte_len = 0,
            _ => {
                let other = GeneratedRuntimeResultBudgetV1::new(32, 1).unwrap();
                let (other_storage, _) = projected(&other);
                storage.decoder.result_gate = other_storage.decoder.result_gate.clone();
            }
        }
        assert!(storage.prepare_readback().is_err());
        assert_eq!(budget.usage(), before);
    }
}
