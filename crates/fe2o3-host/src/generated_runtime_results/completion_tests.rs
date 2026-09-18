use super::*;
use fe2o3_runtime::RuntimeGeneratedResultDomainV1;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::task::{Context, Poll, Waker};

mod bundle_tests;

// Inert metadata exercises the shared observer driver, never receipt authority.
struct ScriptedCompletion {
    domain: Option<RuntimeGeneratedResultDomainV1>,
    ready: bool,
    panics: bool,
    polls: usize,
    drops: Arc<AtomicUsize>,
}

impl Future for ScriptedCompletion {
    type Output = RuntimeGeneratedResultDomainV1;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        self.polls += 1;
        assert!(!self.panics, "scripted poll panic");
        if self.ready {
            Poll::Ready(self.domain.take().expect("one metadata outcome"))
        } else {
            Poll::Pending
        }
    }
}

impl Drop for ScriptedCompletion {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn scripted(gate: &Arc<ResultReadyGateV1>, drops: &Arc<AtomicUsize>) -> ScriptedCompletion {
    ScriptedCompletion {
        domain: Some(RuntimeGeneratedResultDomainV1::from_owner(gate.clone())),
        ready: false,
        panics: false,
        polls: 0,
        drops: drops.clone(),
    }
}

#[test]
fn typed_completion_poll_driver_preserves_pending_and_panicking_observers_then_extracts_once() {
    let outputs = outputs();
    let drops = Arc::new(AtomicUsize::new(0));
    let mut completion = Some(scripted(&outputs.gate, &drops));
    let pointer = outputs
        .words
        .with_seed::<u32, _>(|values| Ok(values.as_ptr()))
        .unwrap();
    let slot = Arc::downgrade(&outputs.word_result.slot);
    let mut output = Some(outputs.word_result);
    let usage = outputs.budget.usage();
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..2 {
        assert!(
            typed_completion::poll_with(&mut completion, &mut output, &mut cx, |_, _| panic!(
                "pending must not extract"
            ))
            .is_pending()
        );
        assert!(Arc::ptr_eq(
            &slot.upgrade().unwrap(),
            &output.as_ref().unwrap().slot
        ));
        assert_eq!(outputs.budget.usage(), usage);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
    }
    completion.as_mut().unwrap().panics = true;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            typed_completion::poll_with(&mut completion, &mut output, &mut cx, |_, _| {
                panic!("panic must not extract")
            })
        }))
        .is_err()
    );
    assert!(completion.is_some() && output.is_some());
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(outputs.budget.usage(), usage);
    outputs
        .words
        .decode(&[10, 0, 0, 0, 20, 0, 0, 0], &outputs.gate)
        .unwrap();
    outputs.gate.commit();
    let future = completion.as_mut().unwrap();
    assert_eq!(future.polls, 3);
    future.panics = false;
    future.ready = true;
    let result = typed_completion::poll_with(
        &mut completion,
        &mut output,
        &mut cx,
        |mut output, domain| {
            assert_eq!(
                drops.load(Ordering::SeqCst),
                1,
                "completion drops before extraction"
            );
            output
                .take_completed_matching_v1(|gate| domain.matches_owner(gate))
                .unwrap()
                .unwrap()
        },
    );
    let Poll::Ready(result) = result else {
        panic!("completed metadata")
    };
    assert!(completion.is_none() && output.is_none());
    assert_eq!(result.as_slice().as_ptr(), pointer);
    assert_eq!(result.as_slice(), &[10, 20]);
    assert_eq!(outputs.budget.usage(), usage);
    drop(result);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
}

#[test]
fn typed_completion_pending_drop_keeps_producer_storage_and_credit() {
    let outputs = outputs();
    let drops = Arc::new(AtomicUsize::new(0));
    let mut completion = Some(scripted(&outputs.gate, &drops));
    let mut output = Some(outputs.word_result);
    let usage = outputs.budget.usage();
    assert!(
        typed_completion::poll_with(
            &mut completion,
            &mut output,
            &mut Context::from_waker(Waker::noop()),
            |_, _| panic!("pending")
        )
        .is_pending()
    );
    drop(completion);
    drop(output);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(outputs.budget.usage(), usage);
    assert_eq!(
        outputs
            .words
            .with_seed::<u32, _>(|values| Ok(values.to_vec()))
            .unwrap(),
        [1, 2]
    );
    drop(outputs.words);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
}

#[test]
fn typed_completion_join_driver_restores_exact_observers_on_rejection() {
    use fe2o3_runtime::RuntimeAsyncEngineCallErrorV1;
    let outputs = outputs();
    let drops = Arc::new(AtomicUsize::new(0));
    let completion = scripted(&outputs.gate, &drops);
    let slot = Arc::downgrade(&outputs.word_result.slot);
    let usage = outputs.budget.usage();
    let rejected = typed_completion::join_with(
        completion,
        outputs.word_result,
        |completion| Err::<(), _>((completion, RuntimeAsyncEngineCallErrorV1::ReentrantCall)),
        |_, _| panic!("rejected join must not extract"),
    );
    let (completion, output, error) = rejected.expect_err("rejected join");
    assert_eq!(error, RuntimeAsyncEngineCallErrorV1::ReentrantCall);
    assert!(
        completion
            .domain
            .as_ref()
            .unwrap()
            .matches_owner(&outputs.gate)
    );
    assert!(Arc::ptr_eq(&slot.upgrade().unwrap(), &output.slot));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    assert_eq!(outputs.budget.usage(), usage);
    outputs.gate.commit();
    let result = typed_completion::join_with(
        completion,
        output,
        |mut completion| {
            Ok::<_, (ScriptedCompletion, RuntimeAsyncEngineCallErrorV1)>(
                completion.domain.take().unwrap(),
            )
        },
        |mut output, domain| {
            assert_eq!(drops.load(Ordering::SeqCst), 1);
            output
                .take_completed_matching_v1(|gate| domain.matches_owner(gate))
                .unwrap()
                .unwrap()
        },
    )
    .ok()
    .expect("joined metadata");
    assert_eq!(result.as_slice(), &[1, 2]);
    drop(result);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
}

#[test]
fn typed_completion_poisoned_binding_preserves_original_storage_and_credits() {
    let mut outputs = outputs();
    let slot = outputs.word_result.slot.clone();
    let usage = outputs.budget.usage();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _locked = slot.state.lock().unwrap();
            panic!("scripted slot poison");
        }))
        .is_err()
    );
    assert!(matches!(
        outputs.word_result.check_completion_binding_v1(|_| true),
        Err(GeneratedRuntimeTypedBindErrorV1::Output(
            GeneratedRuntimeTypedOutputErrorV1::Custody
        ))
    ));
    assert!(matches!(
        outputs.word_result.take_completed_matching_v1(|_| true),
        Err(Error::Custody)
    ));
    assert_eq!(outputs.budget.usage(), usage);
    drop(outputs.word_result);
    assert_eq!(outputs.budget.usage(), usage);
    drop(outputs.words);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
    assert!(matches!(
        *slot.state.lock().err().expect("poisoned slot").into_inner(),
        OutputState::Unavailable
    ));
}

#[test]
fn typed_completion_binding_rejects_foreign_or_busy_output_without_consumption() {
    let mut outputs = outputs();
    let domain = RuntimeGeneratedResultDomainV1::from_owner(outputs.gate.clone());
    let foreign = RuntimeGeneratedResultDomainV1::from_owner(Arc::new(()));
    let usage = outputs.budget.usage();
    assert!(
        outputs
            .word_result
            .check_completion_binding_v1(|gate| domain.matches_owner(gate))
            .is_ok()
    );
    assert!(matches!(
        outputs
            .word_result
            .check_completion_binding_v1(|gate| foreign.matches_owner(gate)),
        Err(GeneratedRuntimeTypedBindErrorV1::Output(
            GeneratedRuntimeTypedOutputErrorV1::BindingMismatch
        ))
    ));
    let slot = outputs.word_result.slot.clone();
    let guard = slot.state.lock().unwrap();
    assert!(matches!(
        outputs
            .word_result
            .check_completion_binding_v1(|_| panic!("no identity callback under contention")),
        Err(GeneratedRuntimeTypedBindErrorV1::Busy)
    ));
    drop(guard);
    assert_eq!(outputs.budget.usage(), usage);
    assert!(
        outputs
            .word_result
            .check_completion_binding_v1(|gate| domain.matches_owner(gate))
            .is_ok()
    );
    outputs.gate.commit();
    let result = outputs
        .word_result
        .take_completed_matching_v1(|gate| domain.matches_owner(gate))
        .unwrap()
        .unwrap();
    assert!(matches!(
        outputs.word_result.check_completion_binding_v1(|_| true),
        Err(GeneratedRuntimeTypedBindErrorV1::Output(
            GeneratedRuntimeTypedOutputErrorV1::OutputUnavailable
        ))
    ));
    drop(result);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
}

#[test]
fn typed_completion_failure_preserves_observer_without_exposing_or_refunding_output() {
    use fe2o3_runtime::{RuntimeAsyncEngineCallErrorV1, RuntimeGfx942ReadbackErrorV1};
    for mode in 0..2 {
        let outputs = outputs();
        let original_slot = Arc::downgrade(&outputs.word_result.slot);
        let usage = outputs.budget.usage();
        let outcome = if mode == 0 {
            Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
        } else {
            Ok(Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage))
        };
        let mut failure = typed_completion::finish(outputs.word_result, outcome)
            .err()
            .expect("failed completion");
        assert!(failure.receipt.is_none());
        assert!(Arc::ptr_eq(
            &original_slot.upgrade().unwrap(),
            &failure.output.slot
        ));
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
        assert!(failure.output.try_take().unwrap().is_none());
        assert_eq!(outputs.budget.usage(), usage);
        drop(failure);
        assert_eq!(
            outputs.budget.usage(),
            usage,
            "original producer retains custody"
        );
        drop(outputs.words);
        assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
        drop(outputs.halves);
        assert_eq!(outputs.budget.usage().reserved_peak_bytes, 0);
    }
}

struct Outputs {
    budget: GeneratedRuntimeResultBudgetV1,
    gate: Arc<ResultReadyGateV1>,
    words: ChargedOutputCustodyV1,
    word_result: GeneratedRuntimeChargedResultV1<u32>,
    halves: ChargedOutputCustodyV1,
    half_result: GeneratedRuntimeChargedResultV1<u16>,
}

fn outputs() -> Outputs {
    let budget = GeneratedRuntimeResultBudgetV1::new(28, 2).unwrap();
    let (words, word_result) = ChargedOutputCustodyV1::new::<u32>(2);
    let (halves, half_result) = ChargedOutputCustodyV1::new::<u16>(3);
    let word_descriptor = || {
        ResultDescriptorV1::new::<u32>(2, Gfx942RuntimeBufferAccessV1::ReadWrite, Some(&words))
            .unwrap()
    };
    let half_descriptor = || {
        ResultDescriptorV1::new::<u16>(3, Gfx942RuntimeBufferAccessV1::WriteOnly, Some(&halves))
            .unwrap()
    };
    let mut preflight = ResultPreflightV1::new().unwrap();
    preflight.push(word_descriptor()).unwrap();
    preflight.push(half_descriptor()).unwrap();
    let mut binding = preflight.reserve(&budget).unwrap();
    words
        .bind_seed(
            vec![1u32, 2].into_boxed_slice(),
            binding.take(&word_descriptor()).unwrap(),
        )
        .unwrap();
    halves
        .bind_seed(
            vec![3u16, 4, 5].into_boxed_slice(),
            binding.take(&half_descriptor()).unwrap(),
        )
        .unwrap();
    assert!(binding.complete());
    Outputs {
        budget,
        gate: binding.gate.clone(),
        words,
        word_result,
        halves,
        half_result,
    }
}

#[test]
fn completed_domain_extraction_preserves_heterogeneous_storage_and_exact_credits() {
    let mut outputs = outputs();
    let domain = RuntimeGeneratedResultDomainV1::from_owner(outputs.gate.clone());
    let foreign = RuntimeGeneratedResultDomainV1::from_owner(Arc::new(ResultReadyGateV1::new(
        &outputs.budget,
    )));
    let words = outputs
        .words
        .with_seed::<u32, _>(|values| Ok((values.as_ptr(), values.len())))
        .unwrap();
    let halves = outputs
        .halves
        .with_seed::<u16, _>(|values| Ok((values.as_ptr(), values.len())))
        .unwrap();
    let before = outputs.budget.usage();
    assert_eq!(before.reserved_peak_bytes, 28);
    assert_eq!(before.retained_members, 2);
    assert!(matches!(
        outputs
            .word_result
            .take_completed_matching_v1(|gate| domain.matches_owner(gate)),
        Err(Error::BindingMismatch)
    ));
    outputs
        .words
        .decode(&[10, 0, 0, 0, 20, 0, 0, 0], &outputs.gate)
        .unwrap();
    outputs
        .halves
        .decode(&[30, 0, 40, 0, 50, 0], &outputs.gate)
        .unwrap();
    // This fixture checks extraction and custody, not native execution or receipt minting.
    outputs.gate.commit();
    assert!(matches!(
        outputs
            .word_result
            .take_completed_matching_v1(|gate| foreign.matches_owner(gate)),
        Err(Error::BindingMismatch)
    ));
    assert_eq!(outputs.budget.usage(), before);
    let word_result = outputs
        .word_result
        .take_completed_matching_v1(|gate| domain.matches_owner(gate))
        .unwrap()
        .unwrap();
    let half_result = outputs
        .half_result
        .take_completed_matching_v1(|gate| domain.matches_owner(gate))
        .unwrap()
        .unwrap();
    assert_eq!((word_result.as_slice().as_ptr(), word_result.len()), words);
    assert_eq!((half_result.as_slice().as_ptr(), half_result.len()), halves);
    assert_eq!(word_result.as_slice(), &[10, 20]);
    assert_eq!(half_result.as_slice(), &[30, 40, 50]);
    assert_eq!(outputs.budget.usage(), before);
    assert!(matches!(
        outputs
            .word_result
            .take_completed_matching_v1(|gate| domain.matches_owner(gate)),
        Err(Error::OutputUnavailable)
    ));
    drop(word_result);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 12);
    assert_eq!(outputs.budget.usage().retained_members, 1);
    drop(half_result);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 0);
    assert_eq!(outputs.budget.usage().retained_members, 0);
    assert!(
        domain.matches_owner(&outputs.gate),
        "metadata holds no result debit"
    );
}

#[test]
fn completed_domain_extraction_retries_contention_without_taking_storage() {
    let mut outputs = outputs();
    let domain = RuntimeGeneratedResultDomainV1::from_owner(outputs.gate.clone());
    outputs.gate.commit();
    let usage = outputs.budget.usage();
    let slot = outputs.word_result.slot.clone();
    let locked = slot.state.lock().unwrap();
    assert!(
        outputs
            .word_result
            .take_completed_matching_v1(|gate| domain.matches_owner(gate))
            .unwrap()
            .is_none()
    );
    assert_eq!(outputs.budget.usage(), usage);
    drop(locked);
    let result = outputs
        .word_result
        .take_completed_matching_v1(|gate| domain.matches_owner(gate))
        .unwrap()
        .unwrap();
    assert_eq!(result.as_slice(), &[1, 2]);
    assert_eq!(outputs.budget.usage(), usage);
}

#[test]
fn result_observer_loss_does_not_refund_retained_owner_and_domain_does_not_keep_debit() {
    let outputs = outputs();
    let domain = RuntimeGeneratedResultDomainV1::from_owner(outputs.gate.clone());
    let usage = outputs.budget.usage();
    drop(outputs.word_result);
    drop(outputs.half_result);
    assert_eq!(outputs.budget.usage(), usage);
    drop(outputs.words);
    drop(outputs.halves);
    assert_eq!(outputs.budget.usage().reserved_peak_bytes, 0);
    assert_eq!(outputs.budget.usage().retained_members, 0);
    assert!(!outputs.gate.ready());
    assert!(domain.matches_owner(&outputs.gate));
}
