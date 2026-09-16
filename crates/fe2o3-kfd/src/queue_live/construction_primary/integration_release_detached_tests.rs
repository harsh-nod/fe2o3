//! Genuine pristine-abort owners join the detached ledger and primary admission.

use super::*;
use crate::queue::live::data_insertion::DetachedInsertionLedgerV1;
use crate::queue::live::data_release::{DataReleaseContextV1, settle_data_release_v1};
use crate::shared_memory::{DataCleanupCustodyV1, DispatchDataReleaseV1};

struct ReleaseContext<'a> {
    parent: &'a mut Parent,
    retained: Vec<DataCleanupCustodyV1>,
    loans: usize,
    retakes: usize,
}

impl DataReleaseContextV1 for ReleaseContext<'_> {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let p = &self.parent;
        if p.poisoned
            || !p.unpublished.quiescent(
                p.completion.ensure_releasable().is_ok(),
                p.dispatch.is_some(),
                p.detached_generation,
                p.detached_count,
                p.detached_identities.len(),
                p.detached_next,
            )
        {
            return Err(Gfx942DispatchBindingErrorV1::ResourcePhase.into());
        }
        Ok(())
    }

    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_> {
        DetachedInsertionLedgerV1 {
            identities: &mut self.parent.detached_identities,
            count: &mut self.parent.detached_count,
            next: &mut self.parent.detached_next,
        }
    }

    fn release(
        &mut self,
        root: &mut DataCleanupCustodyV1,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        let before = ledger_snapshot(self.parent);
        let (lower, retake) = execute_live_model_custody_v1(
            self,
            |context| {
                let e = &mut context.parent.engine;
                assert!(e.backend.foundation_in_engine);
                let loan = e.backend.session.primary_loan(&mut e.foundation)?;
                e.backend.foundation_in_engine = false;
                context.loans += 1;
                Ok::<_, ComputeAqlQueueSessionErrorV1>(loan)
            },
            |context| context.parent.engine.backend.session.release_data(root),
            |context, loan| {
                let e = &mut context.parent.engine;
                assert!(!e.backend.foundation_in_engine);
                e.backend.session.primary_reclaim(&mut e.foundation, loan)?;
                e.backend.foundation_in_engine = true;
                context.retakes += 1;
                Ok(())
            },
            |context| context.parent.poison_release(),
        )?;
        assert_eq!(
            ledger_snapshot(self.parent),
            before,
            "ledger commits only after retake"
        );
        retake?;
        lower?;
        assert!(root.is_complete());
        Ok(())
    }

    fn retain(&mut self, root: DataCleanupCustodyV1) {
        self.retained.push(root);
    }

    fn poison(&mut self, panicked: bool) {
        self.parent.poison_release();
        if panicked {
            Fixture::poison();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct LedgerSnapshot {
    count: usize,
    recycled: Option<u64>,
    identities: Vec<Gfx942FixedDispatchStorageIdentityV1>,
    insertion: Option<usize>,
    continuation: (usize, u64),
    storage: (usize, usize),
}

fn ledger_snapshot(parent: &Parent) -> LedgerSnapshot {
    let continuation = parent.unpublished.continuation.as_ref().unwrap();
    LedgerSnapshot {
        count: parent.detached_count,
        recycled: parent.detached_generation,
        identities: parent.detached_identities.clone(),
        insertion: parent.detached_next,
        continuation: (
            core::ptr::from_ref(continuation) as usize,
            continuation.next_generation_for_test(),
        ),
        storage: (
            parent.detached_identities.as_ptr() as usize,
            parent.detached_identities.capacity(),
        ),
    }
}

fn assert_borrowed_rejection(parent: &Parent, t: &Rc<RefCell<Trace>>, gate: &LocalGateV1) {
    let ledger = ledger_snapshot(parent);
    let memory = parent
        .engine
        .backend
        .session
        .primary_release_memory_snapshot_v1(&parent.engine.foundation);
    let calls = t.borrow().calls.clone();
    let resources = original_resource_ids(parent);
    let signal = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
    let local = t.borrow().local_resources.live();
    assert!(matches!(
        parent.preflight_release(),
        Err(ComputeAqlQueueSessionErrorV1::Contract(
            "unsupported or busy primary release"
        ))
    ));
    assert_eq!(ledger_snapshot(parent), ledger);
    assert_eq!(
        parent
            .engine
            .backend
            .session
            .primary_release_memory_snapshot_v1(&parent.engine.foundation),
        memory
    );
    assert_eq!(t.borrow().calls, calls);
    assert_eq!(original_resource_ids(parent), resources);
    assert_eq!(
        Memory::primary_token_identity(parent.signals.as_ref().unwrap()),
        signal
    );
    assert_eq!(t.borrow().local_resources.live(), local);
    assert!(parent.engine.backend.foundation_in_engine && !parent.poisoned);
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
}

#[test]
fn constructed_primary_release_after_pristine_abort_requires_settled_detached_ledger() {
    let (mut parent, t, gate) = constructed(true);
    let ids = original_resource_ids(&parent);
    let signal_id = Memory::primary_token_identity(parent.signals.as_ref().unwrap());
    let before = RetainedControlSnapshotV1::ordinary_owner_v1(parent.dispatch.as_ref().unwrap());
    let buffers = parent
        .dispatch
        .as_ref()
        .unwrap()
        .prepare_pristine_abort_v1()
        .unwrap();
    let engine = &mut parent.engine;
    let loan = engine
        .backend
        .session
        .primary_loan(&mut engine.foundation)
        .unwrap();
    engine.backend.foundation_in_engine = false;
    let mut abort = parent
        .dispatch
        .take()
        .unwrap()
        .begin_pristine_abort_v1(buffers);
    abort.release_controls(&mut engine.backend.session).unwrap();
    engine
        .backend
        .session
        .primary_reclaim(&mut engine.foundation, loan)
        .unwrap();
    engine.backend.foundation_in_engine = true;
    let (continuation, data, identities) = abort.into_detached();
    assert_eq!(
        data.iter()
            .map(Gfx942FixedDispatchDataV1::sdma_storage_identity)
            .collect::<Vec<_>>(),
        before.data_order_v1()
    );
    assert_eq!(
        data.iter()
            .map(Gfx942FixedDispatchDataV1::storage_identity)
            .collect::<Vec<_>>(),
        identities
    );
    parent.unpublished.continuation = Some(continuation);
    parent.detached_count = data.len();
    parent.detached_identities = identities;
    let original = ledger_snapshot(&parent);
    let mut context = ReleaseContext {
        parent: &mut parent,
        retained: Vec::new(),
        loans: 0,
        retakes: 0,
    };
    for (index, owner) in data.into_iter().enumerate() {
        assert_borrowed_rejection(context.parent, &t, &gate);
        let result = settle_data_release_v1(&mut context, owner);
        assert!(!result.transport);
        result.into_result().unwrap();
        assert_eq!(context.parent.detached_count, original.count - index - 1);
        assert_eq!(
            context.parent.detached_identities,
            original.identities[index + 1..]
        );
        assert_eq!(context.parent.detached_next, Some(0));
        assert_eq!(
            ledger_snapshot(context.parent).continuation,
            original.continuation
        );
    }
    assert!(context.retained.is_empty());
    assert_eq!(
        (context.loans, context.retakes),
        (original.count, original.count)
    );
    drop(context);
    parent.preflight_release().unwrap();

    // Borrowed rejection must neither consume the continuation nor start teardown.
    parent.detached_count = 1;
    assert_borrowed_rejection(&parent, &t, &gate);
    parent.detached_count = 0;
    parent.detached_identities.push(original.identities[0]);
    assert_borrowed_rejection(&parent, &t, &gate);
    parent.detached_identities.clear();
    parent.detached_generation = Some(1);
    assert_borrowed_rejection(&parent, &t, &gate);
    parent.detached_generation = None;
    parent.detached_next = Some(1);
    assert_borrowed_rejection(&parent, &t, &gate);
    parent.detached_next = Some(0);
    parent.preflight_release().unwrap();

    let settled = ledger_snapshot(&parent);
    let mut state = PrimaryReleaseStateV1::<Fixture>::new();
    state.release_in_place(&mut parent).unwrap();
    assert_eq!(
        state
            .resources
            .as_ref()
            .unwrap()
            .observation()
            .controls
            .map(|c| c.identity),
        ids
    );
    assert_eq!(
        state.signals.as_ref().unwrap().observation().identity,
        signal_id
    );
    assert!(state.dispatch.is_none() && state.complete && !parent.poisoned);
    assert_eq!(ledger_snapshot(&parent), settled);
    parent
        .engine
        .backend
        .session
        .primary_assert_all_released_v1();
    assert_no_retry(&mut parent, &mut state, &t);
    assert_eq!(ledger_snapshot(&parent), settled);
    drop(state);
    drop(parent);
    assert_eq!(gate.teardown_count(), 0);
    assert_eq!(gate.observation(), (false, false));
    assert_eq!(t.borrow().local_resources.live(), (0, 0, 0));
}
