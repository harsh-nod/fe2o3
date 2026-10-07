use super::*;
use crate::{
    RuntimeGeneratedResultDomainV1, RuntimeGfx942GeneratedCarrierV1,
    RuntimeGfx942GeneratedCompletionViewV1, RuntimeGfx942GeneratedSourceV1,
};
use std::cell::{Cell, RefCell};

mod async_lending;
mod cancellation;
mod copies;
mod futures;
mod graph;
mod rejected;
pub(super) mod unpublished;

pub(super) struct Borrowed<'a> {
    pub(super) ticks: Cell<usize>,
    pub(super) decoded: &'a Cell<usize>,
    pub(super) dropped: &'a Cell<usize>,
    pub(super) domain: std::sync::Arc<()>,
    pub(super) completion_order: Option<(&'a RefCell<Vec<usize>>, usize)>,
}
impl Drop for Borrowed<'_> {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}
impl RuntimeGfx942GeneratedCarrierV1 for Borrowed<'_> {
    type CurrentnessError = ();
    type Readback = ();
    fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, ()> {
        panic!("no native source in CPU lifecycle fixture")
    }
    fn prepare_readback(&self) -> Result<(), RuntimeGfx942ReadbackErrorV1> {
        Ok(())
    }
    fn install_readback(&mut self, _: ()) {}
}
// SAFETY: private synthetic hooks never inspect source or enter native execution.
#[allow(unsafe_code)]
unsafe impl RuntimeGfx942GeneratedCompletionCarrierV1 for Borrowed<'_> {
    fn completion_domain_v1(
        &self,
    ) -> Result<RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
        Ok(RuntimeGeneratedResultDomainV1::from_owner(
            self.domain.clone(),
        ))
    }
    fn with_completion_view_v1(
        &mut self,
        _: impl for<'a> FnOnce(
            RuntimeGfx942GeneratedCompletionViewV1<'a, ()>,
        ) -> Result<(), NativeError>,
    ) -> Result<(), NativeError> {
        panic!("no native completion view")
    }
    fn complete_readback_v1(self) -> Outcome {
        if let Some((order, index)) = self.completion_order {
            order.borrow_mut().push(index);
        }
        self.decoded.set(self.decoded.get() + 1);
        Ok(())
    }
}

pub(super) fn context() -> RuntimeContextV1<KfdRuntimeBackendV1> {
    RuntimeContextV1::open(KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1()).unwrap()
}

fn hooks<'a, B>() -> Hooks<B, Borrowed<'a>>
where
    B: RuntimeBackendV1<Error = KfdRuntimeBackendErrorV1>
        + RuntimeFlushBackendV1
        + RuntimeAsyncCopyBackendV1,
{
    Hooks {
        domains: |value| {
            value
                .completion_domain_v1()
                .map(CompletionDomainsV1::Singleton)
        },
        decode: RuntimeGfx942PreparedV1::complete_readback_v1,
        progress_graph: |scope| scope.progress_graph_v1(),
        progress_copies: |scope| scope.progress_copies_v1(),
        reserve: |_, _| {
            let (_, projection) = crate::authorized_execution::tests::source_projection_with_access(
                fe2o3_aql::AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
                [crate::Gfx942RuntimeBufferAccessV1::ReadWrite; 3],
            );
            GeneratedHostRosterV1::from_projection(&projection)
        },
        preflight: |_, _, _, _, _| Ok(()),
        ready: |_, _| Ok(true),
        adopt: |_, _, _, _| Ok(()),
        progress: |_, prepared, _, _| {
            let ticks = &prepared.value().ticks;
            if ticks.get() == 0 {
                Ok(true)
            } else {
                ticks.set(ticks.get() - 1);
                Ok(false)
            }
        },
        complete: |context, _, _, hold| {
            context
                .release_unpublished_hold_v1(hold)
                .map(|()| true)
                .map_err(Into::into)
        },
        rejected: |_, _| Ok(false),
        retire_rejected: |_, _, _, _| panic!("no classified rejection in ordinary CPU hooks"),
        unpublished: |_, _| Ok(true),
        retire_unpublished: |_, _| Ok(()),
        copy_progress: RuntimeContextV1::progress_stream_v1,
        graph_submit: RuntimeContextV1::submit_graph_action_v1,
        graph_progress: |context, stream, access| {
            context.drive_stream_with_graph_access_v1(stream, access, B::progress_stream_v1)
        },
    }
}

#[test]
fn scoped_driver_retains_borrowed_concurrent_carriers_until_each_exact_settlement() {
    let mut context = context();
    let device = context.devices()[0].id();
    let streams: Vec<_> = (0..3)
        .map(|_| context.create_stream(device).unwrap())
        .collect();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let prepared: Vec<_> = [3, 0, 2]
        .into_iter()
        .map(|ticks| {
            context.bound_preparation_for_test_v1(Borrowed {
                ticks: Cell::new(ticks),
                decoded: &decoded,
                dropped: &dropped,
                domain: std::sync::Arc::new(()),
                completion_order: None,
            })
        })
        .collect();
    let mut scope = RuntimeGfx942GeneratedScopeV1 {
        epoch: context.scope_epoch.begin().unwrap(),
        context: &mut context,
        slots: Vec::with_capacity(3),
        copies: Vec::with_capacity(3),
        graph: None,
        capacity: 3,
        deadline: Instant::now() + Duration::from_secs(30),
        identity: Rc::new(()),
        invariant: PhantomData,
        hooks: hooks(),
    };
    let tickets: Vec<_> = prepared
        .into_iter()
        .zip(streams)
        .map(|(p, s)| scope.admit(p, s).unwrap())
        .collect();
    assert_eq!(scope.pending_v1(), 3);
    assert_eq!(dropped.get(), 0);
    assert!(matches!(
        scope.check_submission(),
        Err(RuntimeGfx942ScopeErrorV1::Capacity)
    ));
    let foreign = RuntimeGfx942ScopedTicketV1 {
        scope: Rc::new(()),
        index: 0,
        invariant: PhantomData,
    };
    assert!(matches!(
        scope.completion_v1(&foreign),
        Err(RuntimeGfx942ScopeErrorV1::InvalidTicket)
    ));
    for _ in 0..3 {
        scope.progress_v1().unwrap();
    }
    assert!(scope.completion_v1(&tickets[1]).unwrap().is_some());
    assert!(scope.completion_v1(&tickets[0]).unwrap().is_none());
    assert_eq!(decoded.get(), 1);
    assert_eq!(dropped.get(), 1);
    scope.drain_v1().unwrap();
    assert_eq!(decoded.get(), 3);
    assert_eq!(dropped.get(), 3);
    drop(scope);
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn scoped_admission_rejects_capacity_deadline_and_synthetic_device_before_callback() {
    let mut context = context();
    for capacity in [0, MAX_RUNTIME_GFX942_SCOPED_SUBMISSIONS_V1 + 1] {
        let result = context.with_generated_gfx942_scope_v1::<Borrowed<'_>, ()>(
            capacity,
            Instant::now() + Duration::from_secs(30),
            |_| panic!("invalid capacity"),
        );
        assert!(matches!(result, Err(RuntimeGfx942ScopeErrorV1::Capacity)));
    }
    let result =
        context.with_generated_gfx942_scope_v1::<Borrowed<'_>, ()>(1, Instant::now(), |_| {
            panic!("expired admission")
        });
    assert!(matches!(result, Err(RuntimeGfx942ScopeErrorV1::Deadline)));
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    context
        .with_generated_gfx942_scope_v1::<Borrowed<'_>, ()>(
            1,
            Instant::now() + Duration::from_secs(30),
            |scope| {
                assert!(
                    scope
                        .try_submit_v1(device, stream, |_| -> Result<Borrowed<'_>, ()> {
                            panic!("synthetic device cannot invoke preparation")
                        })
                        .is_err()
                );
                assert_eq!(scope.pending_v1(), 0);
            },
        )
        .unwrap();
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}
