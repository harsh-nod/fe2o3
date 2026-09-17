//! The original completion observer, independent of owner-thread custody.

use super::*;

/// Opaque identity of the original host result gate, not execution authority.
/// It extends only gate/account metadata lifetime, not charged output or native custody.
#[doc(hidden)]
#[derive(Clone)]
pub struct RuntimeGeneratedResultDomainV1(Arc<dyn Send + Sync>);

impl RuntimeGeneratedResultDomainV1 {
    /// Retains an existing gate allocation without allocating another domain.
    pub fn from_owner<T: Send + Sync + 'static>(owner: Arc<T>) -> Self {
        Self(owner)
    }

    pub fn matches_owner<T: Send + Sync + 'static>(&self, owner: &Arc<T>) -> bool {
        Arc::as_ptr(&self.0).cast::<()>() == Arc::as_ptr(owner).cast::<()>()
    }
}

impl fmt::Debug for RuntimeGeneratedResultDomainV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeGeneratedResultDomainV1")
            .finish_non_exhaustive()
    }
}

/// Successful completion of the original decoder and its exact result gate.
/// This is not a Worker proof, native capability, or permission to replay work.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::RuntimeGeneratedCompletionReceiptV1;
/// fn duplicate(value: RuntimeGeneratedCompletionReceiptV1) { value.clone(); }
/// ```
#[derive(Debug)]
pub struct RuntimeGeneratedCompletionReceiptV1 {
    domain: RuntimeGeneratedResultDomainV1,
}

impl RuntimeGeneratedCompletionReceiptV1 {
    #[doc(hidden)]
    pub fn matches_owner<T: Send + Sync + 'static>(&self, owner: &Arc<T>) -> bool {
        self.domain.matches_owner(owner)
    }
}

pub type RuntimeAsyncGeneratedCompletionResultV1 = Result<
    Result<RuntimeGeneratedCompletionReceiptV1, crate::RuntimeGfx942ReadbackErrorV1>,
    RuntimeAsyncEngineCallErrorV1,
>;

/// Move-only observer of an activated generated invocation's original reply.
/// Dropping it does not cancel execution or release driver/native/readback custody.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::RuntimeAsyncGeneratedCompletionV1;
/// fn duplicate(value: RuntimeAsyncGeneratedCompletionV1) { value.clone(); }
/// ```
/// ```compile_fail,E0616
/// use fe2o3_runtime::RuntimeAsyncGeneratedCompletionV1;
/// fn extract(value: RuntimeAsyncGeneratedCompletionV1) { let _ = value.future; }
/// ```
#[must_use = "dropping the completion observer does not cancel the invocation"]
pub struct RuntimeAsyncGeneratedCompletionV1 {
    pub(super) future: RuntimeAsyncCommandFutureV1<GeneratedCompletionOutcomeV1>,
    pub(super) domain: Option<RuntimeGeneratedResultDomainV1>,
    pub(super) worker_thread: Arc<OnceLock<thread::ThreadId>>,
}

impl fmt::Debug for RuntimeAsyncGeneratedCompletionV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeAsyncGeneratedCompletionV1")
            .finish_non_exhaustive()
    }
}

/// Blocking admission failure returns the unchanged observer.
#[derive(Debug)]
pub struct RuntimeAsyncGeneratedJoinFailureV1 {
    pub completion: RuntimeAsyncGeneratedCompletionV1,
    pub error: RuntimeAsyncEngineCallErrorV1,
}

impl fmt::Display for RuntimeAsyncGeneratedJoinFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl Error for RuntimeAsyncGeneratedJoinFailureV1 {}

impl RuntimeAsyncGeneratedCompletionV1 {
    pub(in crate::async_engine) fn try_take_ready_v1(
        &mut self,
    ) -> Option<RuntimeAsyncGeneratedCompletionResultV1> {
        let result = self.future.try_take_ready_v1()?;
        Some(self.finish_v1(result))
    }

    fn finish_v1(
        &mut self,
        result: Result<GeneratedCompletionOutcomeV1, RuntimeAsyncEngineCallErrorV1>,
    ) -> RuntimeAsyncGeneratedCompletionResultV1 {
        match result {
            Err(error) => Err(error),
            Ok(Err(error)) => Ok(Err(error)),
            Ok(Ok(())) => self
                .domain
                .take()
                .map(|domain| Ok(RuntimeGeneratedCompletionReceiptV1 { domain }))
                .ok_or(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket),
        }
    }
    /// Tests host output binding without granting completion or execution authority.
    #[doc(hidden)]
    pub fn matches_owner<T: Send + Sync + 'static>(&self, owner: &Arc<T>) -> bool {
        self.domain
            .as_ref()
            .is_some_and(|domain| domain.matches_owner(owner))
    }

    /// Waits on the same future and reply, never by executing a Context command.
    /// The owner thread is rejected before blocking and retains this observer.
    /// This has no deadline: uncertain process-retained custody may remain pending.
    pub fn try_join(
        self,
    ) -> Result<RuntimeAsyncGeneratedCompletionResultV1, RuntimeAsyncGeneratedJoinFailureV1> {
        if self
            .worker_thread
            .get()
            .is_some_and(|id| *id == thread::current().id())
        {
            return Err(RuntimeAsyncGeneratedJoinFailureV1 {
                completion: self,
                error: RuntimeAsyncEngineCallErrorV1::ReentrantCall,
            });
        }
        Ok(owned::join_observer_v1(self))
    }

    #[cfg(test)]
    pub(in crate::async_engine) fn result_probe_for_test_v1(
        &self,
    ) -> impl Fn() -> Option<Result<GeneratedCompletionOutcomeV1, RuntimeAsyncEngineCallErrorV1>>
    + Send
    + Sync
    + 'static {
        self.future.result_probe_for_test_v1()
    }
}

impl Future for RuntimeAsyncGeneratedCompletionV1 {
    type Output = RuntimeAsyncGeneratedCompletionResultV1;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.future).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => Poll::Ready(self.finish_v1(result)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn observer(
        budget: &Arc<reply_budget::ReplyBudgetV1>,
        gate: &Arc<()>,
        worker_thread: Arc<OnceLock<thread::ThreadId>>,
    ) -> (
        owned::Reply<GeneratedCompletionOutcomeV1>,
        RuntimeAsyncGeneratedCompletionV1,
    ) {
        let (reply, future) = owned::Reply::budgeted_pair(budget).unwrap();
        (
            reply,
            RuntimeAsyncGeneratedCompletionV1 {
                future,
                domain: Some(RuntimeGeneratedResultDomainV1::from_owner(gate.clone())),
                worker_thread,
            },
        )
    }

    struct WakeCount(AtomicUsize);
    impl std::task::Wake for WakeCount {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn completion_observer_mints_only_exact_domain_after_original_reply() {
        let budget = reply_budget::ReplyBudgetV1::new(1);
        let gate = Arc::new(());
        let foreign = Arc::new(());
        let (mut reply, mut completion) = observer(&budget, &gate, Arc::new(OnceLock::new()));
        let first = Arc::new(WakeCount(AtomicUsize::new(0)));
        let second = Arc::new(WakeCount(AtomicUsize::new(0)));
        for wake in [&first, &second] {
            assert!(
                Pin::new(&mut completion)
                    .poll(&mut Context::from_waker(&Waker::from(wake.clone())))
                    .is_pending()
            );
        }
        reply.complete(Ok(Ok(())));
        reply.complete(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped));
        assert_eq!(first.0.load(Ordering::SeqCst), 0);
        assert_eq!(second.0.load(Ordering::SeqCst), 1);
        let Poll::Ready(Ok(Ok(receipt))) =
            Pin::new(&mut completion).poll(&mut Context::from_waker(Waker::noop()))
        else {
            panic!("completed original reply")
        };
        assert!(receipt.matches_owner(&gate));
        assert!(!receipt.matches_owner(&foreign));
        drop(completion);
        drop(reply);
        assert_eq!(Arc::strong_count(&gate), 2, "only receipt metadata remains");
        assert!(
            owned::Reply::<()>::budgeted_pair(&budget).is_ok(),
            "no second reply or retained reply credit"
        );
        drop(receipt);
        assert_eq!(Arc::strong_count(&gate), 1);
    }

    #[test]
    fn completion_join_rejects_owner_thread_and_preserves_pending_observer() {
        let budget = reply_budget::ReplyBudgetV1::new(1);
        let gate = Arc::new(());
        let worker = Arc::new(OnceLock::new());
        worker.set(thread::current().id()).unwrap();
        let (mut reply, completion) = observer(&budget, &gate, worker);
        let failure = completion.try_join().unwrap_err();
        assert_eq!(failure.error, RuntimeAsyncEngineCallErrorV1::ReentrantCall);
        assert!(owned::Reply::<()>::budgeted_pair(&budget).is_err());
        let (sent, received) = std::sync::mpsc::sync_channel(1);
        let waiter = thread::spawn(move || {
            sent.send(failure.completion.try_join().unwrap()).unwrap();
        });
        reply.complete(Ok(Ok(())));
        drop(reply);
        let receipt = received
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()
            .unwrap();
        waiter.join().unwrap();
        assert!(receipt.matches_owner(&gate));
        assert!(owned::Reply::<()>::budgeted_pair(&budget).is_ok());
    }

    #[test]
    fn completion_observer_errors_and_drop_never_mint_a_receipt_or_lose_credit() {
        for mode in 0..4 {
            let budget = reply_budget::ReplyBudgetV1::new(1);
            let gate = Arc::new(());
            let (mut reply, mut completion) = observer(&budget, &gate, Arc::new(OnceLock::new()));
            match mode {
                0 => reply.complete(Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)),
                1 => reply.complete(Ok(Err(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage))),
                2 => {
                    completion.domain = None;
                    reply.complete(Ok(Ok(())));
                }
                _ => {
                    drop(completion);
                    assert!(owned::Reply::<()>::budgeted_pair(&budget).is_err());
                    reply.complete(Ok(Ok(())));
                    drop(reply);
                    assert!(owned::Reply::<()>::budgeted_pair(&budget).is_ok());
                    continue;
                }
            }
            let result = completion.try_join().unwrap();
            match mode {
                0 => assert!(matches!(
                    result,
                    Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
                )),
                1 => assert!(matches!(
                    result,
                    Ok(Err(crate::RuntimeGfx942ReadbackErrorV1::InvalidStorage))
                )),
                2 => assert!(matches!(
                    result,
                    Err(RuntimeAsyncEngineCallErrorV1::InvalidPreparedTicket)
                )),
                _ => unreachable!(),
            }
            drop(reply);
            assert!(owned::Reply::<()>::budgeted_pair(&budget).is_ok());
        }
    }
}
