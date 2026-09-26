use super::*;
use crate::generated_runtime_results::bundle_completion::{self, BundleStateV1};
use fe2o3_runtime::{RuntimeAsyncEngineCallErrorV1, RuntimeGfx942ReadbackErrorV1};

fn word_snapshot(output: &GeneratedRuntimeChargedResultV1<u32>) -> (usize, Vec<u32>, usize) {
    let state = output.slot.state.lock().unwrap();
    let OutputState::Prepared { result, gate } = &*state else {
        panic!("expected retained prepared output")
    };
    (
        result.as_slice().as_ptr() as usize,
        result.as_slice().to_vec(),
        Arc::as_ptr(gate) as usize,
    )
}

#[test]
fn bundle_completion_binding_accepts_unready_and_ready_then_moves_original_storage() {
    let outputs = outputs();
    let word_pointer = outputs
        .words
        .with_seed::<u32, _>(|values| Ok(values.as_ptr()))
        .unwrap();
    let half_pointer = outputs
        .halves
        .with_seed::<u16, _>(|values| Ok(values.as_ptr()))
        .unwrap();
    let mut bundle = (outputs.word_result, outputs.half_result);
    let domain = RuntimeGeneratedResultDomainV1::from_owner(outputs.gate.clone());
    let usage = outputs.budget.usage();
    assert!(
        bundle
            .check_binding_matching_v1(|gate| domain.matches_owner(gate))
            .is_ok()
    );
    assert!(matches!(
        bundle.take_completed_matching_v1(|gate| domain.matches_owner(gate)),
        Err(GeneratedRuntimeTypedOutputErrorV1::BindingMismatch)
    ));
    outputs
        .words
        .decode(&[10, 0, 0, 0, 20, 0, 0, 0], &outputs.gate)
        .unwrap();
    outputs
        .halves
        .decode(&[30, 0, 40, 0, 50, 0], &outputs.gate)
        .unwrap();
    outputs.gate.commit();
    assert!(
        bundle
            .check_binding_matching_v1(|gate| domain.matches_owner(gate))
            .is_ok()
    );
    let (words, halves) = bundle
        .take_completed_matching_v1(|gate| domain.matches_owner(gate))
        .unwrap();
    assert_eq!(words.as_slice().as_ptr(), word_pointer);
    assert_eq!(halves.as_slice().as_ptr(), half_pointer);
    assert_eq!(words.as_slice(), &[10, 20]);
    assert_eq!(halves.as_slice(), &[30, 40, 50]);
    assert_eq!(outputs.budget.usage(), usage);
    assert!(matches!(
        bundle.take_completed_matching_v1(|_| true),
        Err(GeneratedRuntimeTypedOutputErrorV1::OutputUnavailable)
    ));
    drop(words);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
    assert_eq!(outputs.budget.usage().retained_members, 1);
    drop(halves);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 0);
    assert_eq!(outputs.budget.usage().retained_members, 0);
}

#[test]
fn bundle_completion_busy_second_binding_preserves_every_slot_and_credit() {
    let outputs = outputs();
    let bundle = (outputs.word_result, outputs.half_result);
    let before = word_snapshot(&bundle.0);
    let usage = outputs.budget.usage();
    let lock = bundle.1.slot.state.lock().unwrap();
    assert!(matches!(
        bundle.check_binding_matching_v1(|gate| Arc::ptr_eq(gate, &outputs.gate)),
        Err(GeneratedRuntimeTypedBindErrorV1::Busy)
    ));
    assert_eq!(word_snapshot(&bundle.0), before);
    assert_eq!(outputs.budget.usage(), usage);
    drop(lock);
    assert!(
        bundle
            .check_binding_matching_v1(|gate| Arc::ptr_eq(gate, &outputs.gate))
            .is_ok()
    );
}

#[test]
fn bundle_completion_late_poison_never_consumes_an_earlier_result() {
    let outputs = outputs();
    let mut bundle = (outputs.word_result, outputs.half_result);
    outputs.gate.commit();
    let before = word_snapshot(&bundle.0);
    let usage = outputs.budget.usage();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _locked = bundle.1.slot.state.lock().unwrap();
            panic!("scripted later-slot poison");
        }))
        .is_err()
    );
    assert!(matches!(
        bundle.check_binding_matching_v1(|_| true),
        Err(GeneratedRuntimeTypedBindErrorV1::Output(
            GeneratedRuntimeTypedOutputErrorV1::Custody
        ))
    ));
    assert!(matches!(
        bundle.take_completed_matching_v1(|_| true),
        Err(GeneratedRuntimeTypedOutputErrorV1::Custody)
    ));
    assert_eq!(word_snapshot(&bundle.0), before);
    assert_eq!(outputs.budget.usage(), usage);
    drop(bundle);
    assert_eq!(outputs.budget.usage(), usage);
    drop(outputs.words);
    drop(outputs.halves);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 0);
}

#[test]
fn bundle_completion_late_foreign_unready_unbound_and_stale_states_preserve_prefix() {
    for mode in 0..5 {
        let outputs = outputs();
        let mut bundle = (outputs.word_result, outputs.half_result);
        let mut retained_result = None;
        let mut retained_state = None;
        if mode == 4 {
            outputs.halves.slot.abandon();
        }
        outputs.gate.commit();
        match mode {
            0 | 1 => {
                let foreign = Arc::new(ResultReadyGateV1::new(&outputs.budget));
                if mode == 0 {
                    foreign.commit();
                }
                if let OutputState::Prepared { gate, .. } =
                    &mut *bundle.1.slot.state.lock().unwrap()
                {
                    *gate = foreign;
                }
            }
            2 => {
                retained_state = Some(std::mem::replace(
                    &mut *bundle.1.slot.state.lock().unwrap(),
                    OutputState::Unbound,
                ))
            }
            3 => retained_result = bundle.1.take_completed_matching_v1(|_| true).unwrap(),
            _ => {}
        }
        let before = word_snapshot(&bundle.0);
        let usage = outputs.budget.usage();
        let expected = if mode < 3 {
            GeneratedRuntimeTypedOutputErrorV1::BindingMismatch
        } else {
            GeneratedRuntimeTypedOutputErrorV1::OutputUnavailable
        };
        let error = bundle
            .take_completed_matching_v1(|gate| mode == 1 || Arc::ptr_eq(gate, &outputs.gate))
            .err()
            .unwrap();
        assert_eq!(error, expected, "mode {mode}");
        assert_eq!(word_snapshot(&bundle.0), before, "mode {mode}");
        assert_eq!(outputs.budget.usage(), usage);
        drop(retained_result);
        drop(retained_state);
    }
}

#[test]
fn bundle_completion_duplicate_slots_reject_before_any_lock_or_callback() {
    let outputs = outputs();
    let duplicate = GeneratedRuntimeChargedResultV1 {
        slot: outputs.word_result.slot.clone(),
    };
    let mut bundle = (outputs.word_result, duplicate);
    let slot = bundle.0.slot.clone();
    let locked = slot.state.lock().unwrap();
    let usage = outputs.budget.usage();
    assert!(matches!(
        bundle.check_binding_matching_v1(|_| panic!("duplicate must reject first")),
        Err(GeneratedRuntimeTypedBindErrorV1::Output(
            GeneratedRuntimeTypedOutputErrorV1::BindingMismatch
        ))
    ));
    assert!(matches!(
        bundle.take_completed_matching_v1(|_| panic!("duplicate must reject first")),
        Err(GeneratedRuntimeTypedOutputErrorV1::BindingMismatch)
    ));
    assert_eq!(outputs.budget.usage(), usage);
    drop(locked);
    assert_eq!(word_snapshot(&bundle.0).1, [1, 2]);
}

fn homogeneous_outputs(
    count: usize,
) -> (
    GeneratedRuntimeResultBudgetV1,
    Arc<ResultReadyGateV1>,
    Vec<ChargedOutputCustodyV1>,
    Vec<GeneratedRuntimeChargedResultV1<u8>>,
) {
    let budget = GeneratedRuntimeResultBudgetV1::new(count as u64 * 2, count).unwrap();
    let (owners, observers): (Vec<_>, Vec<_>) = (0..count)
        .map(|_| ChargedOutputCustodyV1::new::<u8>(1))
        .unzip();
    let mut preflight = ResultPreflightV1::new().unwrap();
    for owner in &owners {
        preflight
            .push(
                ResultDescriptorV1::new::<u8>(
                    1,
                    Gfx942RuntimeBufferAccessV1::WriteOnly,
                    Some(owner),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let mut binding = preflight.reserve(&budget).unwrap();
    for (index, owner) in owners.iter().enumerate() {
        let descriptor =
            ResultDescriptorV1::new::<u8>(1, Gfx942RuntimeBufferAccessV1::WriteOnly, Some(owner))
                .unwrap();
        owner
            .bind_seed(
                vec![index as u8].into_boxed_slice(),
                binding.take(&descriptor).unwrap(),
            )
            .unwrap();
    }
    assert!(binding.complete());
    (budget, binding.gate.clone(), owners, observers)
}

#[test]
fn bundle_completion_three_members_never_commit_before_middle_or_last_validation() {
    for failing in [1, 2] {
        let (budget, gate, owners, observers) = homogeneous_outputs(3);
        let mut iter = observers.into_iter();
        let mut bundle = (
            iter.next().unwrap(),
            iter.next().unwrap(),
            iter.next().unwrap(),
        );
        owners[failing].slot.abandon();
        gate.commit();
        let usage = budget.usage();
        assert!(matches!(
            bundle.take_completed_matching_v1(|observed| Arc::ptr_eq(observed, &gate)),
            Err(GeneratedRuntimeTypedOutputErrorV1::OutputUnavailable)
        ));
        assert_eq!(budget.usage(), usage);
        for (index, owner) in owners.iter().enumerate() {
            if index != failing {
                let observed = {
                    let slot = owner
                        .slot
                        .as_any()
                        .downcast_ref::<OutputSlot<u8>>()
                        .unwrap();
                    let state = slot.state.lock().unwrap();
                    match &*state {
                        OutputState::Prepared { result, .. } => Some(result.as_slice().to_vec()),
                        _ => None,
                    }
                };
                assert_eq!(observed, Some(vec![index as u8]));
            }
        }
    }
}

#[test]
fn bundle_completion_maximum_abi_tuple_preserves_every_original_slot() {
    let (budget, gate, owners, observers) = homogeneous_outputs(MAX_ABI_FIELDS);
    let pointers: Vec<_> = owners
        .iter()
        .map(|owner| {
            owner
                .with_seed::<u8, _>(|values| Ok(values.as_ptr()))
                .unwrap()
        })
        .collect();
    let mut iter = observers.into_iter();
    macro_rules! take {
        ($index:tt) => {
            iter.next().unwrap()
        };
    }
    macro_rules! maximum_tuple {
        ($($index:tt),+) => {{
            let mut bundle = ($(take!($index),)+);
            assert!(bundle.check_binding_matching_v1(|observed| Arc::ptr_eq(observed, &gate)).is_ok());
            gate.commit();
            let results = bundle.take_completed_matching_v1(|observed| Arc::ptr_eq(observed, &gate)).unwrap();
            $(assert_eq!(results.$index.as_slice().as_ptr(), pointers[$index]);
              assert_eq!(results.$index.as_slice(), &[$index as u8]);)+
            assert_eq!(budget.usage().reserved_peak_bytes, 128);
            drop(results);
        }};
    }
    maximum_tuple!(
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
        25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47,
        48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63
    );
    assert!(iter.next().is_none());
    assert_eq!(budget.usage().reserved_peak_bytes, 0);
    assert_eq!(budget.usage().retained_members, 0);
}

#[test]
fn bundle_completion_poll_preserves_pending_and_panicking_observers_then_extracts_once() {
    let outputs = outputs();
    let drops = Arc::new(AtomicUsize::new(0));
    let mut completion = Some(scripted(&outputs.gate, &drops));
    let originals = (
        Arc::downgrade(&outputs.word_result.slot),
        Arc::downgrade(&outputs.half_result.slot),
    );
    let mut bundle = Some((outputs.word_result, outputs.half_result));
    let usage = outputs.budget.usage();
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..2 {
        assert!(
            typed_completion::poll_with(&mut completion, &mut bundle, &mut cx, |_, _| panic!(
                "pending cannot extract"
            ))
            .is_pending()
        );
        let current = bundle.as_ref().unwrap();
        assert!(Arc::ptr_eq(
            &originals.0.upgrade().unwrap(),
            &current.0.slot
        ));
        assert!(Arc::ptr_eq(
            &originals.1.upgrade().unwrap(),
            &current.1.slot
        ));
        assert_eq!(outputs.budget.usage(), usage);
    }
    completion.as_mut().unwrap().panics = true;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || typed_completion::poll_with(&mut completion, &mut bundle, &mut cx, |_, _| panic!(
                "panicking poll cannot extract"
            ))
        ))
        .is_err()
    );
    assert!(completion.is_some() && bundle.is_some());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    outputs.gate.commit();
    completion.as_mut().unwrap().panics = false;
    completion.as_mut().unwrap().ready = true;
    let result = typed_completion::poll_with(
        &mut completion,
        &mut bundle,
        &mut cx,
        |mut bundle, domain| {
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            bundle
                .take_completed_matching_v1(|gate| domain.matches_owner(gate))
                .unwrap()
        },
    );
    let Poll::Ready((words, halves)) = result else {
        panic!("ready metadata");
    };
    assert!(completion.is_none() && bundle.is_none());
    assert_eq!(words.as_slice(), &[1, 2]);
    assert_eq!(halves.as_slice(), &[3, 4, 5]);
    assert_eq!(outputs.budget.usage(), usage);
}

#[test]
fn bundle_completion_join_rejection_preserves_complete_tuple_for_retry() {
    let outputs = outputs();
    let drops = Arc::new(AtomicUsize::new(0));
    let completion = scripted(&outputs.gate, &drops);
    let originals = (
        Arc::downgrade(&outputs.word_result.slot),
        Arc::downgrade(&outputs.half_result.slot),
    );
    let bundle = (outputs.word_result, outputs.half_result);
    let usage = outputs.budget.usage();
    let rejected = typed_completion::join_with(
        completion,
        bundle,
        |completion| Err::<(), _>((completion, RuntimeAsyncEngineCallErrorV1::ReentrantCall)),
        |_, _| panic!("rejected join cannot extract"),
    );
    let (completion, bundle, error) = rejected.expect_err("join rejection");
    assert_eq!(error, RuntimeAsyncEngineCallErrorV1::ReentrantCall);
    assert!(Arc::ptr_eq(&originals.0.upgrade().unwrap(), &bundle.0.slot));
    assert!(Arc::ptr_eq(&originals.1.upgrade().unwrap(), &bundle.1.slot));
    assert_eq!(outputs.budget.usage(), usage);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    outputs.gate.commit();
    let results = typed_completion::join_with(
        completion,
        bundle,
        |mut completion| {
            Ok::<_, (ScriptedCompletion, RuntimeAsyncEngineCallErrorV1)>(
                completion.domain.take().unwrap(),
            )
        },
        |mut bundle, domain| bundle.take_completed_matching_v1(|gate| domain.matches_owner(gate)),
    )
    .ok()
    .unwrap()
    .unwrap();
    assert_eq!(results.0.as_slice(), &[1, 2]);
    assert_eq!(results.1.as_slice(), &[3, 4, 5]);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn bundle_completion_failure_and_observer_loss_preserve_producer_owned_storage() {
    for mode in 0..2 {
        let outputs = outputs();
        let originals = (
            Arc::downgrade(&outputs.word_result.slot),
            Arc::downgrade(&outputs.half_result.slot),
        );
        let usage = outputs.budget.usage();
        let outcome = if mode == 0 {
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        } else {
            Ok(Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage))
        };
        let mut failure =
            bundle_completion::finish((outputs.word_result, outputs.half_result), outcome)
                .err()
                .unwrap();
        assert!(failure.receipt.is_none());
        assert!(Arc::ptr_eq(
            &originals.0.upgrade().unwrap(),
            &failure.outputs.0.slot
        ));
        assert!(Arc::ptr_eq(
            &originals.1.upgrade().unwrap(),
            &failure.outputs.1.slot
        ));
        assert!(failure.outputs.0.try_take().unwrap().is_none());
        assert!(failure.outputs.1.try_take().unwrap().is_none());
        assert_eq!(outputs.budget.usage(), usage);
        match mode {
            0 => assert!(matches!(
                failure.error,
                GeneratedRuntimeTypedCompletionErrorV1::Engine(
                    RuntimeAsyncEngineCallErrorV1::EngineStopped
                )
            )),
            _ => assert!(matches!(
                failure.error,
                GeneratedRuntimeTypedCompletionErrorV1::Readback(
                    RuntimeGfx942ReadbackErrorV1::InvalidStorage
                )
            )),
        }
        drop(failure);
        assert_eq!(outputs.budget.usage(), usage);
        drop(outputs.words);
        assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
        drop(outputs.halves);
        assert_eq!(outputs.budget.usage().reserved_peak_bytes, 0);
    }
}

#[test]
fn bundle_completion_public_observers_and_results_are_send() {
    fn send<T: Send>() {}
    type Bundle = (
        GeneratedRuntimeChargedResultV1<u32>,
        GeneratedRuntimeChargedResultV1<u16>,
    );
    send::<GeneratedRuntimeTypedBundleCompletionV1<Bundle>>();
    send::<GeneratedRuntimeTypedBundleCompletionV1<()>>();
    send::<GeneratedRuntimeTypedBundleCompletionV1<(GeneratedRuntimeChargedResultV1<u32>,)>>();
    send::<GeneratedRuntimeTypedBundleBindFailureV1<Bundle>>();
    send::<GeneratedRuntimeTypedBundleFailureV1<Bundle>>();
    send::<GeneratedRuntimeTypedBundleJoinFailureV1<Bundle>>();
    send::<
        GeneratedRuntimeCompletedBundleV1<<Bundle as GeneratedRuntimeTypedOutputBundleV1>::Results>,
    >();
}

#[test]
fn singleton_bundle_moves_original_storage_and_keeps_its_charge_until_disposal() {
    let outputs = outputs();
    let mut bundle = (outputs.word_result,);
    let before = word_snapshot(&bundle.0);
    let usage = outputs.budget.usage();
    let matches = |gate: &Arc<ResultReadyGateV1>| Arc::ptr_eq(gate, &outputs.gate);
    assert!(bundle.check_binding_matching_v1(matches).is_ok());
    assert!(matches!(
        bundle.take_completed_matching_v1(matches),
        Err(GeneratedRuntimeTypedOutputErrorV1::BindingMismatch)
    ));
    assert_eq!(word_snapshot(&bundle.0), before);
    outputs
        .words
        .decode(&[10, 0, 0, 0, 20, 0, 0, 0], &outputs.gate)
        .unwrap();
    outputs.gate.commit();
    let (result,) = bundle.take_completed_matching_v1(matches).unwrap();
    assert_eq!(result.as_slice().as_ptr() as usize, before.0);
    assert_eq!(result.as_slice(), &[10, 20]);
    assert_eq!(outputs.budget.usage(), usage);
    assert!(matches!(
        bundle.take_completed_matching_v1(matches),
        Err(GeneratedRuntimeTypedOutputErrorV1::OutputUnavailable)
    ));
    drop(result);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
    assert_eq!(outputs.budget.usage().retained_members, 1);
}

#[test]
fn singleton_bundle_busy_foreign_and_poisoned_states_do_not_consume_storage() {
    let outputs = outputs();
    let bundle = (outputs.word_result,);
    let before = word_snapshot(&bundle.0);
    let usage = outputs.budget.usage();
    let lock = bundle.0.slot.state.lock().unwrap();
    assert!(matches!(
        bundle.check_binding_matching_v1(|_| true),
        Err(GeneratedRuntimeTypedBindErrorV1::Busy)
    ));
    drop(lock);
    assert!(matches!(
        bundle.check_binding_matching_v1(|_| false),
        Err(GeneratedRuntimeTypedBindErrorV1::Output(
            GeneratedRuntimeTypedOutputErrorV1::BindingMismatch
        ))
    ));
    assert_eq!(word_snapshot(&bundle.0), before);
    assert_eq!(outputs.budget.usage(), usage);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _lock = bundle.0.slot.state.lock().unwrap();
            panic!("scripted singleton slot poison");
        }))
        .is_err()
    );
    assert!(matches!(
        bundle.check_binding_matching_v1(|_| true),
        Err(GeneratedRuntimeTypedBindErrorV1::Output(
            GeneratedRuntimeTypedOutputErrorV1::Custody
        ))
    ));
    assert_eq!(outputs.budget.usage(), usage);
    drop(bundle);
    assert_eq!(outputs.budget.usage(), usage);
}

#[test]
fn empty_bundle_shared_poll_driver_waits_without_touching_uncollected_outputs() {
    // Scripted metadata tests observer plumbing, not receipt construction or a
    // successful protected/native invocation.
    let outputs = outputs();
    let before = word_snapshot(&outputs.word_result);
    let usage = outputs.budget.usage();
    let drops = Arc::new(AtomicUsize::new(0));
    let mut completion = Some(scripted(&outputs.gate, &drops));
    let mut bundle = Some(());
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..2 {
        assert!(
            typed_completion::poll_with(&mut completion, &mut bundle, &mut cx, |(), _| panic!(
                "pending empty bundle cannot finish"
            ))
            .is_pending()
        );
        assert!(bundle.is_some());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
    completion.as_mut().unwrap().ready = true;
    assert!(
        typed_completion::poll_with(&mut completion, &mut bundle, &mut cx, |(), domain| assert!(
            domain.matches_owner(&outputs.gate)
        ))
        .is_ready()
    );
    assert!(completion.is_none() && bundle.is_none());
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(word_snapshot(&outputs.word_result), before);
    assert_eq!(outputs.budget.usage(), usage);
}

#[test]
fn empty_bundle_errors_do_not_fabricate_receipts_or_refund_uncollected_outputs() {
    let outputs = outputs();
    let usage = outputs.budget.usage();
    let before = word_snapshot(&outputs.word_result);
    for outcome in [
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped),
        Ok(Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)),
    ] {
        let failure = bundle_completion::finish((), outcome).err().unwrap();
        assert!(failure.receipt.is_none());
        assert_eq!(word_snapshot(&outputs.word_result), before);
        assert_eq!(outputs.budget.usage(), usage);
    }
}

#[test]
fn empty_bundle_shared_join_rejection_preserves_original_observer() {
    let outputs = outputs();
    let drops = Arc::new(AtomicUsize::new(0));
    let future = scripted(&outputs.gate, &drops);
    let (future, (), error) = typed_completion::join_with(
        future,
        (),
        |future| Err::<(), _>((future, RuntimeAsyncEngineCallErrorV1::ReentrantCall)),
        |(), ()| panic!("rejected join cannot finish"),
    )
    .unwrap_err();
    assert_eq!(error, RuntimeAsyncEngineCallErrorV1::ReentrantCall);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert!(future.domain.as_ref().unwrap().matches_owner(&outputs.gate));
    let usage = outputs.budget.usage();
    drop(future);
    assert_eq!(outputs.budget.usage(), usage);
}
