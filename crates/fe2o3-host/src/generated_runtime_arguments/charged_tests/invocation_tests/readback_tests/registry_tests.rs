//! Storage/credit controls only; no native completion is fabricated.

use super::*;
use crate::generated_runtime_arguments::{
    GeneratedRegistryRepeatFrameV1, GeneratedRegistryStorageV1,
};

fn original(
    budget: &GeneratedRuntimeResultBudgetV1,
) -> (
    GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
    GeneratedRuntimeChargedResultV1<u32>,
) {
    let plan = tests::plan::<u32>(&[Access::WriteOnly], None);
    let (output, result) =
        GeneratedRuntimeWriteSlice::new_charged(vec![1_u32; 4].into_boxed_slice());
    let packed = prepare_charged_with_plan(
        output,
        &plan,
        limits(),
        budget,
        |output, account| output.account_storage(account),
        |output, account| {
            Ok(
                GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(
                    vec![],
                    vec![output.bind_argument(&plan, 0, account)?],
                ),
            )
        },
    )
    .unwrap();
    let hsaco = synthetic_cov6::preparation_module();
    let storage = packed
        .into_runtime_inputs(geometry(), 0, 1000)
        .storage
        .prepare(&hsaco, "vecadd")
        .unwrap()
        .project_persistent(&hsaco)
        .unwrap();
    (storage, result)
}

fn stage(storage: &mut GeneratedRegistryStorageV1) {
    let readback = storage.prepare_readback().unwrap();
    storage.install_readback(readback);
    let (_, buffers) = storage.borrow_readback().unwrap();
    buffers[0].1.copy_from_slice(&words(&[7_u32, 11, 13, 17]));
}

#[test]
fn registry16_data_only_results_release_no_original_source_debit() {
    let budget = GeneratedRuntimeResultBudgetV1::new(16 * 64, 16 * 3).unwrap();
    let mut originals = core::array::from_fn::<_, 16, _>(|_| {
        let (source, observer) = original(&budget);
        let pointer = source.prepared().buffers()[0].bytes().as_ptr();
        let mut storage = GeneratedRegistryStorageV1::new(source).unwrap();
        stage(&mut storage);
        (storage, observer, pointer)
    });
    assert_eq!(budget.usage().reserved_peak_bytes, 16 * 64);
    assert_eq!(budget.usage().retained_members, 48);
    for (index, (storage, observer, pointer)) in originals.iter_mut().enumerate() {
        storage.decode_retaining_source().unwrap();
        let result = observer.try_take().unwrap().unwrap();
        assert_eq!(result.as_slice(), &[7, 11, 13, 17]);
        drop(result);
        assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), *pointer);
        assert_eq!(
            budget.usage().reserved_peak_bytes,
            16 * 64 - (index as u64 + 1) * 48
        );
    }
    assert_eq!(budget.usage().reserved_peak_bytes, 16 * 16);
    assert_eq!(budget.usage().retained_members, 16);
    drop(originals);
    assert_empty(&budget);
}

#[test]
fn registry_decode_retains_original_source_and_its_credit_after_result_disposal() {
    let budget = GeneratedRuntimeResultBudgetV1::new(64, 3).unwrap();
    let (original, observer) = original(&budget);
    let pointer = original.prepared().buffers()[0].bytes().as_ptr();
    let bytes = original.prepared().buffers()[0].bytes().to_vec();
    let mut storage = GeneratedRegistryStorageV1::new(original).unwrap();
    stage(&mut storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 64);
    assert_eq!(budget.usage().retained_members, 3);
    let observer = Mutex::new(observer);
    storage
        .decode_retaining_source_with(|_| {
            assert!(observer.lock().unwrap().try_take().unwrap().is_none());
        })
        .unwrap();
    assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), pointer);
    assert_eq!(storage.prepared().buffers()[0].bytes(), bytes);
    assert!(storage.prepared_mut().is_none());
    assert!(storage.decode_retaining_source().is_err());
    assert!(storage.prepare_readback().is_err());
    let result = observer.into_inner().unwrap().try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[7, 11, 13, 17]);
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    drop(result);
    assert_eq!(budget.usage().reserved_peak_bytes, 16);
    assert_eq!(budget.usage().retained_members, 1);
    assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), pointer);
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn registry_result_can_retain_original_credit_after_source_disposal() {
    let budget = GeneratedRuntimeResultBudgetV1::new(64, 3).unwrap();
    let (original, mut observer) = original(&budget);
    let mut storage = GeneratedRegistryStorageV1::new(original).unwrap();
    stage(&mut storage);
    storage.decode_retaining_source().unwrap();
    let result = observer.try_take().unwrap().unwrap();
    drop(storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    assert_eq!(result.as_slice(), &[7, 11, 13, 17]);
    drop(result);
    assert_empty(&budget);
}

#[test]
fn registry_decode_panic_keeps_original_payload_and_retained_source_charge() {
    let budget = GeneratedRuntimeResultBudgetV1::new(64, 3).unwrap();
    let (original, mut observer) = original(&budget);
    let pointer = original.prepared().buffers()[0].bytes().as_ptr();
    let mut storage = GeneratedRegistryStorageV1::new(original).unwrap();
    stage(&mut storage);
    let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        storage.decode_retaining_source_with(|_| panic!("injected original decoder unwind"))
    }));
    assert!(failed.is_err());
    assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), pointer);
    assert!(storage.prepared_mut().is_none());
    assert!(storage.decode_retaining_source().is_err());
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    assert_eq!(budget.usage().reserved_peak_bytes, 16);
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn registry_source_reservation_refuses_byte_or_record_pressure_without_leak() {
    for (bytes, records) in [(47, 2), (64, 1)] {
        let budget = GeneratedRuntimeResultBudgetV1::new(bytes, records).unwrap();
        let (original, mut observer) = original(&budget);
        assert_eq!(budget.usage().reserved_peak_bytes, 32);
        assert!(GeneratedRegistryStorageV1::new(original).is_err());
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
        assert_empty(&budget);
    }
}

#[test]
fn registry_decoder_refusal_keeps_original_readwrite_storage_and_all_charges() {
    let budget = GeneratedRuntimeResultBudgetV1::new(64, 3).unwrap();
    let (original, mut observer) = projected(&budget);
    let pointer = original.prepared().buffers()[0].bytes().as_ptr();
    let mut storage = GeneratedRegistryStorageV1::new(original).unwrap();
    stage(&mut storage);
    let before = budget.usage();
    assert!(matches!(
        storage.decode_retaining_source(),
        Err(Error::BindingMismatch)
    ));
    assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), pointer);
    assert_eq!(budget.usage(), before);
    assert!(observer.try_take().unwrap().is_none());
    drop(storage);
    assert_empty(&budget);
}

#[test]
fn registry_repeat2_data_frames_keep_old_result_and_original_source_credits_separate() {
    let budget = GeneratedRuntimeResultBudgetV1::new(112, 5).unwrap();
    let (original, mut first) = original(&budget);
    assert!(
        GeneratedRegistryRepeatFrameV1::prepare(&original, &budget).is_err(),
        "ordinary inert fixture cannot become native repeat authority"
    );
    let pointer = original.prepared().buffers()[0].bytes().as_ptr();
    let (mut second, mut second_result) =
        GeneratedRegistryRepeatFrameV1::prepare_data_only_for_test(&original, &budget).unwrap();
    let mut storage = GeneratedRegistryStorageV1::new(original).unwrap();
    stage(&mut storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 112);
    assert_eq!(budget.usage().retained_members, 5);
    storage.decode_retaining_source().unwrap();
    let first = first.try_take().unwrap().unwrap();
    assert!(second_result.try_take().unwrap().is_none());
    second.borrow_readback(storage.prepared()).unwrap()[0]
        .1
        .copy_from_slice(&words(&[23_u32, 29, 31, 37]));
    second.decode(storage.prepared()).unwrap();
    let second_result = second_result.try_take().unwrap().unwrap();
    assert_eq!(first.as_slice(), &[7, 11, 13, 17]);
    assert_eq!(second_result.as_slice(), &[23, 29, 31, 37]);
    assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), pointer);
    assert!(second.decode(storage.prepared()).is_err());
    assert!(second.domain().is_err());
    assert!(storage.decode_retaining_source().is_err());
    assert_eq!(budget.usage().reserved_peak_bytes, 80);
    drop(second_result);
    drop(second);
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    drop(storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    assert_eq!(first.as_slice(), &[7, 11, 13, 17]);
    drop(first);
    assert_empty(&budget);
}

#[test]
fn registry_repeat2_second_decoder_panic_retains_original_source_and_old_result() {
    let budget = GeneratedRuntimeResultBudgetV1::new(112, 5).unwrap();
    let (original, mut first) = original(&budget);
    let pointer = original.prepared().buffers()[0].bytes().as_ptr();
    let (mut second, mut observer) =
        GeneratedRegistryRepeatFrameV1::prepare_data_only_for_test(&original, &budget).unwrap();
    let mut storage = GeneratedRegistryStorageV1::new(original).unwrap();
    stage(&mut storage);
    storage.decode_retaining_source().unwrap();
    let first = first.try_take().unwrap().unwrap();
    second.borrow_readback(storage.prepared()).unwrap()[0]
        .1
        .copy_from_slice(&words(&[23_u32, 29, 31, 37]));
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            second.decode_with(storage.prepared(), |_| panic!("second decoder unwind"))
        }))
        .is_err()
    );
    assert_eq!(storage.prepared().buffers()[0].bytes().as_ptr(), pointer);
    assert_eq!(first.as_slice(), &[7, 11, 13, 17]);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    drop(second);
    drop(storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    drop(first);
    assert_empty(&budget);
}

#[test]
fn registry_repeat2_second_frame_pressure_preserves_original_pending_result() {
    for (bytes, records) in [(79, 3), (80, 2)] {
        let budget = GeneratedRuntimeResultBudgetV1::new(bytes, records).unwrap();
        let (original, mut observer) = original(&budget);
        let before = budget.usage();
        assert!(
            GeneratedRegistryRepeatFrameV1::prepare_data_only_for_test(&original, &budget).is_err()
        );
        assert_eq!(
            budget.usage().reserved_peak_bytes,
            before.reserved_peak_bytes
        );
        assert_eq!(budget.usage().retained_members, before.retained_members);
        assert!(observer.try_take().unwrap().is_none());
        drop(original);
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
        assert_empty(&budget);
    }
}
