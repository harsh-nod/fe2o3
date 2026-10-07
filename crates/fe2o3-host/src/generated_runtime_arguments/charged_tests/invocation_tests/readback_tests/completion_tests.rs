use super::*;

fn stage_original(storage: &mut GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>) {
    let mut readback = storage.prepare_readback().unwrap();
    for ((_, bytes), source) in readback
        .buffers_mut()
        .iter_mut()
        .zip(storage.prepared().buffers())
    {
        bytes.copy_from_slice(source.bytes());
    }
    storage.install_readback(readback);
}

#[test]
fn reserved_readback_completion_preserves_owner_through_decode() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, observer) = mixed(&budget);
    assert_eq!(budget.usage().reserved_peak_bytes, 48);
    stage_original(&mut storage);
    storage.readback.as_mut().unwrap().buffers_mut()[0]
        .1
        .copy_from_slice(&words(&[9u32, 13, 17, 21]));
    storage.validate_reserved_readback().unwrap();
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
    let observer = Mutex::new(observer);
    witnessed
        .decode_validated_readback_with(|index| {
            assert_eq!(index, 0);
            assert!(disposed.load(Ordering::SeqCst));
            assert_eq!(budget.usage().reserved_peak_bytes, 80);
            assert_eq!(budget.usage().retained_members, 3);
            assert!(observer.lock().unwrap().try_take().unwrap().is_none());
        })
        .unwrap();
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    assert_eq!(budget.usage().retained_members, 1);
    let result = observer.into_inner().unwrap().try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[9, 13, 17, 21]);
    drop(result);
    assert_empty(&budget);
}

#[test]
fn reserved_readback_foreign_gate_rejects_with_shared_or_separate_budgets() {
    for shared in [false, true] {
        let first_budget = GeneratedRuntimeResultBudgetV1::new(160, 6).unwrap();
        let second_budget = if shared {
            first_budget.clone()
        } else {
            GeneratedRuntimeResultBudgetV1::new(160, 6).unwrap()
        };
        let (mut first, mut first_observer) = mixed(&first_budget);
        let (mut second, mut second_observer) = mixed(&second_budget);
        stage_original(&mut second);
        let foreign = second.readback.take().unwrap();
        for ((access, bytes), (ordinal, source)) in foreign
            .buffers()
            .iter()
            .zip(first.prepared().buffers().iter().enumerate())
        {
            assert_eq!(Some(*access), first.prepared().buffer_access(ordinal));
            assert_eq!(bytes.as_slice(), source.bytes());
        }
        first.install_readback(foreign);
        assert!(matches!(
            first.decode_reserved_readback_with(|_| panic!("foreign decode")),
            Err(Error::BindingMismatch)
        ));
        assert!(matches!(
            first_observer.try_take(),
            Err(Error::OutputUnavailable)
        ));
        assert_eq!(second_budget.usage().reserved_peak_bytes, 48);
        if !shared {
            assert_empty(&first_budget);
        }
        assert!(second_observer.try_take().unwrap().is_none());
        stage_original(&mut second);
        second.decode_reserved_readback().unwrap();
        let result = second_observer.try_take().unwrap().unwrap();
        assert_eq!(result.as_slice(), &[1; 4]);
        drop(result);
        assert_empty(&first_budget);
        assert_empty(&second_budget);
    }
}

#[test]
fn reserved_readback_unused_read_only_mutation_rejects_all_outputs() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, mut observer) = mixed(&budget);
    stage_original(&mut storage);
    assert_eq!(storage.prepared().pointer_fixups().len(), 1);
    storage.readback.as_mut().unwrap().buffers_mut()[1].1[15] ^= 1;
    assert!(matches!(
        storage.decode_reserved_readback_with(|_| panic!("late mutation decoded")),
        Err(Error::BindingMismatch)
    ));
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn reserved_readback_decoder_panic_disposes_overlap_before_seed_refund() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, observer) = mixed(&budget);
    stage_original(&mut storage);
    storage.validate_reserved_readback().unwrap();
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
    let observer = Mutex::new(observer);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        witnessed.decode_validated_readback_with(|_| {
            assert!(disposed.load(Ordering::SeqCst));
            assert_eq!(budget.usage().reserved_peak_bytes, 80);
            assert!(observer.lock().unwrap().try_take().unwrap().is_none());
            panic!("reserved readback decoder panic");
        })
    }));
    assert_eq!(
        result.unwrap_err().downcast_ref::<&str>(),
        Some(&"reserved readback decoder panic")
    );
    assert_empty(&budget);
    assert!(matches!(
        observer.into_inner().unwrap().try_take(),
        Err(Error::OutputUnavailable)
    ));
}

#[test]
fn reserved_readback_dropped_observer_retains_credits_until_completion() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, observer) = mixed(&budget);
    stage_original(&mut storage);
    drop(observer);
    assert_eq!(budget.usage().reserved_peak_bytes, 80);
    storage
        .decode_reserved_readback_with(|_| {
            assert_eq!(budget.usage().reserved_peak_bytes, 80);
        })
        .unwrap();
    assert_empty(&budget);
}

#[test]
fn reserved_readback_missing_owner_never_decodes() {
    let budget = GeneratedRuntimeResultBudgetV1::new(48, 2).unwrap();
    let (storage, mut observer) = projected(&budget);
    assert!(matches!(
        storage.decode_reserved_readback_with(|_| panic!("missing owner decoded")),
        Err(Error::BindingMismatch)
    ));
    assert_empty(&budget);
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
}

#[test]
fn reserved_readback_late_shape_and_decoder_mutations_reject_before_delivery() {
    for mutation in 0..7 {
        let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
        let (mut storage, mut observer) = mixed(&budget);
        stage_original(&mut storage);
        match mutation {
            0 => {
                storage.readback.as_mut().unwrap().buffers_mut().pop();
            }
            1 => {
                storage.readback.as_mut().unwrap().buffers_mut()[1].1.pop();
            }
            2 => storage.readback.as_mut().unwrap().buffers_mut()[1]
                .1
                .reserve_exact(1),
            3 => {
                storage.readback.as_mut().unwrap().buffers_mut()[1].0 =
                    Gfx942RuntimeBufferAccessV1::ReadWrite
            }
            4 => storage.decoder.expectations[1].byte_len += 1,
            5 => storage.decoder.expectations[1].access = Gfx942RuntimeBufferAccessV1::ReadWrite,
            _ => storage.decoder.result_gate = None,
        }
        assert!(matches!(
            storage.decode_reserved_readback_with(|_| panic!("malformed owner decoded")),
            Err(Error::BindingMismatch)
        ));
        assert_empty(&budget);
        assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    }
}

#[test]
fn reserved_readback_settlement_failure_never_commits_ready_outputs() {
    let budget = GeneratedRuntimeResultBudgetV1::new(80, 3).unwrap();
    let (mut storage, mut observer) = mixed(&budget);
    stage_original(&mut storage);
    // Model a malformed adapter owner without shedding the live buffers' debit.
    storage
        .readback
        .as_mut()
        .unwrap()
        .quarantine_credit_for_test();
    let decoded = AtomicBool::new(false);
    assert!(matches!(
        storage.decode_reserved_readback_with(|_| {
            decoded.store(true, Ordering::SeqCst);
        }),
        Err(Error::ResultCredit(ResourceCreditErrorV1::Invariant))
    ));
    assert!(decoded.load(Ordering::SeqCst));
    assert!(matches!(observer.try_take(), Err(Error::OutputUnavailable)));
    let usage = budget.usage();
    assert_eq!(usage.reserved_peak_bytes, 32);
    assert_eq!(usage.quarantined_members, 1);
    assert_eq!(usage.retained_members, 0);
    assert_eq!(usage.unissued_members, 0);
}

#[test]
fn reserved_readback_read_only_only_completion_releases_all_members() {
    let plan = tests::plan::<u32>(&[Access::ReadOnly], None);
    let input = GeneratedRuntimeReadSlice::new(vec![1u32, 2, 3, 4].into_boxed_slice());
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 2).unwrap();
    let packed = prepare_charged_with_plan(
        input,
        &plan,
        limits(),
        &budget,
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
    .unwrap();
    let hsaco = synthetic_cov6::preparation_module();
    let mut storage = packed
        .into_runtime_inputs(geometry(), 0, 1000)
        .storage
        .prepare(&hsaco, "vecadd")
        .unwrap()
        .project_persistent(&hsaco)
        .unwrap();
    stage_original(&mut storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    storage
        .decode_reserved_readback_with(|_| panic!("read-only output callback"))
        .unwrap();
    assert_empty(&budget);
}

#[test]
fn reserved_readback_zero_output_does_not_shift_physical_ordinals() {
    let plan = tests::plan::<u32>(&[Access::ReadWrite, Access::ReadOnly], None);
    let (output, mut observer) =
        GeneratedRuntimeReadWriteSlice::new_charged(Vec::<u32>::new().into_boxed_slice());
    let input = GeneratedRuntimeReadSlice::new(vec![2u32; 4].into_boxed_slice());
    let budget = GeneratedRuntimeResultBudgetV1::new(32, 3).unwrap();
    let packed = prepare_charged_with_plan(
        (output, input),
        &plan,
        limits(),
        &budget,
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
    let GeneratedRuntimePackedArgumentsV1 {
        packed, decoder, ..
    } = packed.packed;
    // Structural-only fixture: the first nonempty buffer is input ordinal 1.
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
    let mut storage = storage
        .prepare(&hsaco, "vecadd")
        .unwrap()
        .project_persistent(&hsaco)
        .unwrap();
    assert_eq!(storage.decoder.expectations.len(), 2);
    assert_eq!(storage.prepared().buffers().len(), 1);
    assert_eq!(budget.usage().reserved_peak_bytes, 16);
    assert_eq!(budget.usage().retained_members, 2);
    stage_original(&mut storage);
    assert_eq!(budget.usage().reserved_peak_bytes, 32);
    storage
        .decode_reserved_readback_with(|index| assert_eq!(index, 0))
        .unwrap();
    let result = observer.try_take().unwrap().unwrap();
    assert!(result.as_slice().is_empty());
    assert_eq!(budget.usage().reserved_peak_bytes, 0);
    assert_eq!(budget.usage().retained_members, 1);
    drop(result);
    assert_empty(&budget);
}
