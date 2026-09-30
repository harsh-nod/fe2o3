use super::*;
use fe2o3_runtime_model::{
    ContextAllocationReferenceV1, ContextAllocationWriteV1, ContextWriterReferenceV1,
};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

type E = ContextVersionJournalErrorV1;
type Status = ContextProducerReadStatusV1;

#[derive(Debug, Eq, PartialEq)]
enum Call {
    Input(usize, usize, usize),
    ActiveLookup(usize),
    ActiveStatus(usize),
    QueuedLookup(usize),
    QueuedStatus(usize),
    Credit(u64, bool),
    Live(u64),
    ActiveCount,
    QueuedCount,
}

type Calls = Rc<RefCell<Vec<Call>>>;

// The actual helper body consumes real retained/request types. These scripted
// query/account observations are not an alternate journal or a native backend.
struct Journal {
    active: Vec<(Result<ContextProducerReadV1, E>, Result<Status, E>)>,
    queued: Vec<(Result<ContextQueuedProducerReadV1, E>, Result<Status, E>)>,
    calls: Calls,
}

impl Journal {
    fn lookup_producer_read(
        &self,
        reference: ContextProducerReadReferenceV1,
    ) -> Result<ContextProducerReadV1, E> {
        self.calls
            .borrow_mut()
            .push(Call::ActiveLookup(reference.slot));
        self.active[reference.slot].0
    }

    fn producer_read_status(&self, reference: ContextProducerReadReferenceV1) -> Result<Status, E> {
        self.calls
            .borrow_mut()
            .push(Call::ActiveStatus(reference.slot));
        self.active[reference.slot].1
    }

    fn lookup_queued_producer_read(
        &self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<ContextQueuedProducerReadV1, E> {
        self.calls
            .borrow_mut()
            .push(Call::QueuedLookup(reference.slot));
        self.queued[reference.slot].0
    }

    fn queued_producer_read_status(
        &self,
        reference: ContextQueuedProducerReadReferenceV1,
    ) -> Result<Status, E> {
        self.calls
            .borrow_mut()
            .push(Call::QueuedStatus(reference.slot));
        self.queued[reference.slot].1
    }
}

struct Versions {
    journal: Journal,
    live: HashMap<RuntimeAllocationIdV1, Result<ContextAllocationReferenceV1, E>>,
    calls: Calls,
}

impl Versions {
    fn validate_live(
        &self,
        id: RuntimeAllocationIdV1,
        _: &AllocationRecordV1,
    ) -> Result<ContextAllocationReferenceV1, E> {
        self.calls.borrow_mut().push(Call::Live(id.local));
        self.live[&id]
    }
}

struct Launch {
    dependencies_held: bool,
    dependencies: Vec<ScalarPeerDependencyV1>,
    sources: Vec<ContextReadSourceV1>,
}

struct Peer {
    directed: Option<()>,
    dependencies_held: bool,
    dependencies: Vec<ScalarPeerDependencyV1>,
    source: ContextReadSourceV1,
}

struct Context {
    producer_launches: HashMap<RuntimeSubmissionIdV1, Launch>,
    scalar_peer_copies: HashMap<RuntimeSubmissionIdV1, Peer>,
    allocations: HashMap<RuntimeAllocationIdV1, AllocationRecordV1>,
    backend_allocations: HashSet<u64>,
}

struct Owner {
    context: Context,
    versions: Versions,
    root: RetainedProducerReadV1,
    credit_returns: Vec<bool>,
    calls: Calls,
    custody: Box<u64>,
}

struct Observations<'a> {
    owner: &'a Owner,
    next_credit: usize,
    launch: bool,
}

impl Observations<'_> {
    fn input_count(&self) -> usize {
        self.owner.root.inputs.len()
    }
    fn active_count(&self) -> usize {
        self.owner.calls.borrow_mut().push(Call::ActiveCount);
        self.owner.root.references.len()
    }
    fn queued_count(&self) -> usize {
        self.owner.calls.borrow_mut().push(Call::QueuedCount);
        self.owner.root.queued_references.len()
    }
    fn observe_expected_credit(
        &mut self,
        allocation: RuntimeAllocationIdV1,
        device: RuntimeDeviceIdV1,
        bytes: u64,
    ) -> bool {
        let source = self.owner.context.allocations[&allocation];
        assert_eq!((device, bytes), (source.device, source.byte_len));
        let answer = self.owner.credit_returns[self.next_credit];
        self.next_credit += 1;
        self.owner
            .calls
            .borrow_mut()
            .push(Call::Credit(allocation.local, answer));
        answer
    }
    fn validate(
        &mut self,
        index: usize,
        active_index: &mut usize,
        queued_index: &mut usize,
    ) -> Result<Status, E> {
        let owner = self.owner;
        owner
            .calls
            .borrow_mut()
            .push(Call::Input(index, *active_index, *queued_index));
        let context = &owner.context;
        let versions = &owner.versions;
        let root = &owner.root;
        let id = id();
        let consumer = consumer();
        let launch = self.launch;
        producer_input_validate_body!(
            completion_journal_rust_syntax,
            context,
            versions,
            root,
            id,
            consumer,
            launch,
            index,
            active_index,
            queued_index,
            self
        )
    }
    fn reconcile(&mut self) -> Result<Status, E> {
        let invalid_reference = E::InvalidReference;
        producer_input_fold_body!(
            completion_journal_rust_syntax,
            self,
            invalid_reference,
            (index, aggregate, active_index, queued_index, input_count),
            [],
            [],
            []
        )
    }
}

fn id() -> RuntimeSubmissionIdV1 {
    RuntimeSubmissionIdV1::new(17, 9)
}
fn consumer() -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: 17,
        local: 9,
        kind: ContextWriterKindV1::Submission,
    }
}

fn fixture(rows: &[(bool, Status)]) -> Owner {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut result = Owner {
        context: Context {
            producer_launches: HashMap::new(),
            scalar_peer_copies: HashMap::new(),
            allocations: HashMap::new(),
            backend_allocations: HashSet::new(),
        },
        versions: Versions {
            journal: Journal {
                active: Vec::new(),
                queued: Vec::new(),
                calls: Rc::clone(&calls),
            },
            live: HashMap::new(),
            calls: Rc::clone(&calls),
        },
        root: RetainedProducerReadV1 {
            domain: ProducerReadDomainV1::Launch,
            inputs: Vec::new(),
            requests: Vec::new(),
            references: Vec::new(),
            queued_requests: Vec::new(),
            queued_references: Vec::new(),
            marker: None,
        },
        credit_returns: vec![true; rows.len()],
        calls,
        custody: Box::new(731),
    };
    let producer = ContextWriterReferenceV1 {
        slot: 2,
        key: ContextWriterKeyV1 {
            context_generation: 17,
            local: 5,
            kind: ContextWriterKindV1::Submission,
        },
    };
    for (index, &(queued, status)) in rows.iter().enumerate() {
        let allocation = RuntimeAllocationIdV1::new(17, 10 + index as u64);
        let device = RuntimeDeviceIdV1::new(17, 1);
        let reference = ContextAllocationReferenceV1 {
            slot: index,
            key: ContextAllocationKeyV1 {
                context_generation: 17,
                local: allocation.local,
            },
        };
        let member = ContextAllocationWriteV1 {
            allocation: reference,
            device: enrollment(allocation, device, 64).device,
            byte_extent: 64,
        };
        let source = ContextReadSourceV1 {
            region: RuntimeMemoryRegionV1 {
                allocation,
                byte_offset: 0,
                byte_len: 64,
                access: RuntimeAccessV1::Read,
            },
            record: AllocationRecordV1 {
                backend_allocation: 100 + index as u64,
                device,
                kind: RuntimeMemoryKindV1::DeviceLocal,
                byte_len: 64,
                journal: Some(reference),
            },
        };
        let dependency = ScalarPeerDependencyV1 {
            ordinal: index,
            event: RuntimeEventIdV1::new(17, 7 + index as u64),
            backend_event: 70 + index as u64,
            submission: RuntimeSubmissionIdV1::new(17, 5),
            backend_submission: 50,
            stream: RuntimeStreamIdV1::new(17, 2),
            device,
        };
        let request = if queued {
            let request = ContextQueuedProducerReadV1 {
                allocation: member,
                byte_offset: 0,
                byte_len: 64,
                producer,
            };
            let slot = result.root.queued_references.len();
            result.root.queued_requests.push(request);
            result
                .root
                .queued_references
                .push(ContextQueuedProducerReadReferenceV1 {
                    slot,
                    incarnation: 20 + slot as u64,
                    consumer: consumer(),
                });
            result
                .versions
                .journal
                .queued
                .push((Ok(request), Ok(status)));
            ProducerReadRequestV1::Queued(request)
        } else {
            let request = ContextProducerReadV1 {
                read: ContextAllocationReadV1 {
                    allocation: reference,
                    device: member.device,
                    byte_extent: 64,
                    byte_offset: 0,
                    byte_len: 64,
                    attempt_epoch: 1,
                    content_lineage: 1,
                },
                producer,
            };
            let slot = result.root.references.len();
            result.root.requests.push(request);
            result.root.references.push(ContextProducerReadReferenceV1 {
                slot,
                incarnation: 10 + slot as u64,
                consumer: consumer(),
            });
            result
                .versions
                .journal
                .active
                .push((Ok(request), Ok(status)));
            ProducerReadRequestV1::Active(request)
        };
        result.root.inputs.push(ProducerInputV1 {
            source,
            dependency,
            request,
        });
        result.context.allocations.insert(allocation, source.record);
        result
            .context
            .backend_allocations
            .insert(source.record.backend_allocation);
        result.versions.live.insert(allocation, Ok(reference));
    }
    result.context.producer_launches.insert(
        id(),
        Launch {
            dependencies_held: true,
            dependencies: result
                .root
                .inputs
                .iter()
                .map(|input| input.dependency)
                .collect(),
            sources: result
                .root
                .inputs
                .iter()
                .map(|input| input.source)
                .collect(),
        },
    );
    if let Some(input) = result.root.inputs.first() {
        result.context.scalar_peer_copies.insert(
            id(),
            Peer {
                directed: Some(()),
                dependencies_held: true,
                dependencies: vec![input.dependency],
                source: input.source,
            },
        );
        result.root.marker = Some(result.root.complete_marker());
    }
    result
}

fn run(owner: &Owner) -> Result<Status, E> {
    let before = (
        &*owner.custody as *const u64,
        owner.root.inputs.as_ptr(),
        owner.root.references.clone(),
        owner.root.queued_references.clone(),
        owner.root.requests.clone(),
        owner.root.queued_requests.clone(),
    );
    let result = Observations {
        owner,
        next_credit: 0,
        launch: true,
    }
    .reconcile();
    assert_eq!(
        before,
        (
            &*owner.custody as *const u64,
            owner.root.inputs.as_ptr(),
            owner.root.references.clone(),
            owner.root.queued_references.clone(),
            owner.root.requests.clone(),
            owner.root.queued_requests.clone()
        )
    );
    result
}

#[test]
fn producer_input_fold_visits_mixed_families_and_checks_both_final_counts() {
    let owner = fixture(&[
        (true, Status::Success),
        (false, Status::Pending),
        (true, Status::Success),
    ]);
    assert_eq!(run(&owner), Ok(Status::Pending));
    assert_eq!(
        *owner.calls.borrow(),
        vec![
            Call::Input(0, 0, 0),
            Call::QueuedLookup(0),
            Call::QueuedStatus(0),
            Call::Credit(10, true),
            Call::Live(10),
            Call::Input(1, 0, 1),
            Call::ActiveLookup(0),
            Call::ActiveStatus(0),
            Call::Credit(11, true),
            Call::Live(11),
            Call::Input(2, 1, 1),
            Call::QueuedLookup(1),
            Call::QueuedStatus(1),
            Call::Credit(12, true),
            Call::Live(12),
            Call::ActiveCount,
            Call::QueuedCount
        ]
    );
}

#[test]
fn producer_input_fold_status_precedence_is_not_early_termination() {
    let statuses = [
        Status::Success,
        Status::Pending,
        Status::NoEffect,
        Status::Unknown,
    ];
    for (left_index, left) in statuses.iter().enumerate() {
        for (right_index, right) in statuses.iter().enumerate() {
            let owner = fixture(&[(false, *left), (true, *right)]);
            assert_eq!(run(&owner), Ok(statuses[left_index.max(right_index)]));
            assert!(owner.calls.borrow().contains(&Call::Live(11)));
        }
    }
    let mut owner = fixture(&[(false, Status::Unknown), (true, Status::Success)]);
    owner.root.queued_references[0].consumer.local += 1;
    assert_eq!(run(&owner), Err(E::InvalidReference));
    assert_eq!(owner.calls.borrow().last(), Some(&Call::Input(1, 1, 0)));
}

#[test]
fn producer_input_fold_credit_observations_are_reached_individually() {
    let mut owner = fixture(&[(false, Status::Unknown), (true, Status::Success)]);
    owner.credit_returns[1] = false;
    owner
        .versions
        .live
        .insert(RuntimeAllocationIdV1::new(17, 11), Err(E::InvalidState));
    assert_eq!(run(&owner), Err(E::InvalidReference));
    assert!(owner.calls.borrow().contains(&Call::Credit(10, true)));
    assert_eq!(owner.calls.borrow().last(), Some(&Call::Credit(11, false)));
    owner.calls.borrow_mut().clear();
    owner.context.backend_allocations.remove(&100);
    assert_eq!(run(&owner), Err(E::InvalidReference));
    assert!(
        !owner
            .calls
            .borrow()
            .iter()
            .any(|call| matches!(call, Call::Credit(..) | Call::Live(..)))
    );
}

#[test]
fn producer_input_fold_first_error_and_cursor_advance_order_are_exact() {
    for queued in [false, true] {
        let mut owner = fixture(&[(queued, Status::Success)]);
        if queued {
            owner.versions.journal.queued[0].0 = Err(E::InvalidState);
        } else {
            owner.versions.journal.active[0].0 = Err(E::InvalidState);
        }
        owner.credit_returns[0] = false;
        let mut observations = Observations {
            owner: &owner,
            next_credit: 0,
            launch: true,
        };
        let (mut active, mut queued_index) = (0, 0);
        assert_eq!(
            observations.validate(0, &mut active, &mut queued_index),
            Err(E::InvalidState)
        );
        assert_eq!((active, queued_index), (0, 0));
        owner.calls.borrow_mut().clear();
        if queued {
            owner.root.queued_requests[0].producer.key.local += 1;
        } else {
            owner.root.requests[0].producer.key.local += 1;
        }
        let mut observations = Observations {
            owner: &owner,
            next_credit: 0,
            launch: true,
        };
        assert_eq!(
            observations.validate(0, &mut active, &mut queued_index),
            Err(E::InvalidReference)
        );
        assert_eq!(*owner.calls.borrow(), vec![Call::Input(0, 0, 0)]);
        if queued {
            owner.root.queued_requests[0].producer.key.local -= 1;
        } else {
            owner.root.requests[0].producer.key.local -= 1;
        }
        if queued {
            owner.versions.journal.queued[0] =
                (Ok(owner.root.queued_requests[0]), Err(E::AllocationBusy));
        } else {
            owner.versions.journal.active[0] = (Ok(owner.root.requests[0]), Err(E::AllocationBusy));
        }
        let mut observations = Observations {
            owner: &owner,
            next_credit: 0,
            launch: true,
        };
        assert_eq!(
            observations.validate(0, &mut active, &mut queued_index),
            Err(E::AllocationBusy)
        );
        assert_eq!((active, queued_index), if queued { (0, 1) } else { (1, 0) });
        assert_eq!(observations.next_credit, 0);
    }
}

#[test]
fn producer_input_fold_missing_overflowing_and_excess_family_references_fail_closed() {
    for queued in [false, true] {
        let owner = fixture(&[(queued, Status::Success)]);
        let mut observations = Observations {
            owner: &owner,
            next_credit: 0,
            launch: true,
        };
        let (mut active, mut queued_index) = if queued {
            (0, usize::MAX)
        } else {
            (usize::MAX, 0)
        };
        assert_eq!(
            observations.validate(0, &mut active, &mut queued_index),
            Err(E::InvalidReference)
        );
        assert_eq!(observations.next_credit, 0);

        let mut owner = fixture(&[(queued, Status::Success)]);
        if queued {
            owner.root.queued_references.clear();
        } else {
            owner.root.references.clear();
        }
        assert_eq!(run(&owner), Err(E::InvalidReference));
        assert_eq!(*owner.calls.borrow(), vec![Call::Input(0, 0, 0)]);

        let mut owner = fixture(&[(queued, Status::Unknown), (queued, Status::Success)]);
        if queued {
            owner.root.queued_references[0].incarnation = u64::MAX;
        } else {
            owner.root.references[0].incarnation = u64::MAX;
        }
        assert_eq!(run(&owner), Err(E::InvalidReference));
        assert_eq!(
            owner.calls.borrow().last(),
            Some(&Call::Input(1, usize::from(!queued), usize::from(queued)))
        );

        let mut owner = fixture(&[(queued, Status::Success)]);
        if queued {
            owner
                .root
                .queued_references
                .push(owner.root.queued_references[0]);
        } else {
            owner.root.references.push(owner.root.references[0]);
        }
        assert_eq!(run(&owner), Err(E::InvalidReference));
        assert_eq!(
            owner.calls.borrow().last(),
            Some(if queued {
                &Call::QueuedCount
            } else {
                &Call::ActiveCount
            })
        );
    }
}

#[test]
fn producer_input_fold_binding_and_live_error_order_stays_lazy() {
    let mut owner = fixture(&[(false, Status::Success)]);
    owner
        .versions
        .live
        .insert(RuntimeAllocationIdV1::new(17, 10), Err(E::AllocationBusy));
    assert_eq!(run(&owner), Err(E::AllocationBusy));
    assert_eq!(owner.calls.borrow().last(), Some(&Call::Live(10)));
    owner.calls.borrow_mut().clear();
    owner
        .context
        .producer_launches
        .get_mut(&id())
        .unwrap()
        .dependencies_held = false;
    assert_eq!(run(&owner), Err(E::InvalidReference));
    assert_eq!(owner.calls.borrow().last(), Some(&Call::ActiveStatus(0)));
    owner.calls.borrow_mut().clear();
    assert_eq!(
        Observations {
            owner: &owner,
            next_credit: 0,
            launch: false
        }
        .reconcile(),
        Err(E::AllocationBusy)
    );
    owner
        .context
        .scalar_peer_copies
        .get_mut(&id())
        .unwrap()
        .directed = None;
    owner.calls.borrow_mut().clear();
    assert_eq!(
        Observations {
            owner: &owner,
            next_credit: 0,
            launch: false
        }
        .reconcile(),
        Err(E::InvalidReference)
    );
    assert_eq!(owner.calls.borrow().last(), Some(&Call::ActiveStatus(0)));
}
