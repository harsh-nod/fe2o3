use super::*;
use fe2o3_runtime_model::{
    DeviceGenerationV1, DeviceKeyV1, PhysicalDeviceIdV1, QueueGenerationV1, QueueInstanceIdV1,
    QueueKeyV1, VmIdV1, VmKeyV1,
};

fn production_source() -> &'static str {
    let root = include_str!("../multi_queue.rs");
    let compact: String = root.split_whitespace().collect();
    for name in ["publication", "plan"] {
        let declaration = format!("#[path=\"multi_queue/{name}.rs\"]mod{name};");
        assert_eq!(compact.matches(&declaration).count(), 1);
    }
    concat!(
        include_str!("../multi_queue.rs"),
        include_str!("publication.rs"),
        include_str!("plan.rs"),
    )
}

#[test]
fn diagnostic_spin_budget_is_a_closed_bounded_roster() {
    for (budget, label, nanoseconds) in [
        (
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current,
            "current",
            0,
        ),
        (
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros250,
            "250us",
            250_000,
        ),
        (
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros500,
            "500us",
            500_000,
        ),
        (
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Millis1,
            "1ms",
            1_000_000,
        ),
        (
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Micros1500,
            "1500us",
            1_500_000,
        ),
        (
            Gfx942SdmaStripedDiagnosticSpinBudgetV1::Millis3,
            "3ms",
            3_000_000,
        ),
    ] {
        assert_eq!(budget.label(), label);
        assert_eq!(budget.nanoseconds(), nanoseconds);
        assert_eq!(
            budget.active_spin_floor(),
            (nanoseconds != 0).then(|| Duration::from_nanos(nanoseconds))
        );
    }
}

fn queue_key(physical: u64, queue: u64, generation: u64) -> QueueKeyV1 {
    QueueKeyV1 {
        vm: VmKeyV1 {
            device: DeviceKeyV1 {
                physical: PhysicalDeviceIdV1(physical),
                generation: DeviceGenerationV1(1),
            },
            id: VmIdV1(1),
        },
        id: QueueInstanceIdV1(queue),
        generation: QueueGenerationV1(generation),
    }
}

#[test]
fn aggregate_observation_visits_every_entry_before_reporting_pending() {
    let entries = [0_usize, 1, 2, 3];
    for pending_index in 0..entries.len() {
        let mut visited = Vec::new();
        let ready = observe_entire_completion_roster(&entries, |entry| {
            visited.push(*entry);
            Ok::<bool, ()>(*entry != pending_index)
        })
        .unwrap();
        assert!(!ready);
        assert_eq!(visited, entries);
    }

    let mut visited = Vec::new();
    let ready = observe_entire_completion_roster(&entries, |entry| {
        visited.push(*entry);
        Ok::<bool, ()>(true)
    })
    .unwrap();
    assert!(ready);
    assert_eq!(visited, entries);
}

#[test]
fn aggregate_observation_stops_only_for_terminal_observation_error() {
    let entries = [0_usize, 1, 2, 3];
    let mut visited = Vec::new();
    let result = observe_entire_completion_roster(&entries, |entry| {
        visited.push(*entry);
        if *entry == 2 {
            Err("injected unexpected completion")
        } else {
            Ok(*entry != 0)
        }
    });
    assert_eq!(result, Err("injected unexpected completion"));
    assert_eq!(visited, [0, 1, 2]);
}

#[test]
fn aggregate_completion_reconstructs_original_request_order() {
    let ticket = |queue_id: u32, slot: u16| Gfx942SdmaCopyTicketV1 {
        owner: queue_key(7, 8, u64::from(queue_id)),
        queue_id,
        slot,
        generation: 1,
    };
    let mut entries = [
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 3,
            queue_ordinal: 1,
            ticket: ticket(11, 9),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 0,
            queue_ordinal: 0,
            ticket: ticket(10, 4),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 2,
            queue_ordinal: 0,
            ticket: ticket(10, 8),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 1,
            queue_ordinal: 1,
            ticket: ticket(11, 5),
        },
    ];
    assert!(order_validated_completion_entries(&mut entries));
    assert_eq!(
        entries.map(|entry| (entry.request_index, entry.queue_ordinal, entry.ticket.slot,)),
        [(0, 0, 4), (1, 1, 5), (2, 0, 8), (3, 1, 9)]
    );

    let mut duplicate = [
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 0,
            queue_ordinal: 0,
            ticket: ticket(10, 4),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 0,
            queue_ordinal: 1,
            ticket: ticket(11, 5),
        },
    ];
    assert!(!order_validated_completion_entries(&mut duplicate));

    let mut missing = [
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 0,
            queue_ordinal: 0,
            ticket: ticket(10, 4),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 2,
            queue_ordinal: 1,
            ticket: ticket(11, 5),
        },
    ];
    assert!(!order_validated_completion_entries(&mut missing));
}

#[test]
fn aggregate_retirement_preflight_rejects_every_missing_record_without_prefix_move() {
    let ticket = |queue_id: u32, slot: u16| Gfx942SdmaCopyTicketV1 {
        owner: queue_key(7, 8, u64::from(queue_id)),
        queue_id,
        slot,
        generation: 1,
    };
    let entries = [
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 0,
            queue_ordinal: 0,
            ticket: ticket(10, 4),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 1,
            queue_ordinal: 1,
            ticket: ticket(11, 5),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 2,
            queue_ordinal: 0,
            ticket: ticket(10, 8),
        },
        ValidatedMultiQueueCompletionEntryV1 {
            request_index: 3,
            queue_ordinal: 1,
            ticket: ticket(11, 9),
        },
    ];
    for missing_request in 0..entries.len() {
        let admitted = entire_validated_completion_roster_remains_present(&entries, |entry| {
            usize::from(entry.request_index) != missing_request
        });
        let moved_count = if admitted { entries.len() } else { 0 };
        assert!(!admitted);
        assert_eq!(moved_count, 0);
    }
    assert!(entire_validated_completion_roster_remains_present(
        &entries,
        |_| true
    ));
}

#[test]
fn aggregate_pending_observation_preserves_preallocated_full_ticket_roster() {
    let ticket = |request_index| Gfx942SdmaCopyTicketV1 {
        owner: queue_key(7, 8, 1),
        queue_id: 10,
        slot: request_index,
        generation: u32::from(request_index) + 1,
    };
    let mut completion = PreparedMultiQueueCompletionV1 {
        ordered: Vec::with_capacity(4),
        completed: Vec::with_capacity(4),
    };
    for request_index in 0_u16..4 {
        completion
            .ordered
            .push(ValidatedMultiQueueCompletionEntryV1 {
                request_index,
                queue_ordinal: 0,
                ticket: ticket(request_index),
            });
    }
    let ordered_pointer = completion.ordered.as_ptr();
    let ordered_capacity = completion.ordered.capacity();
    let completed_pointer = completion.completed.as_ptr();
    let completed_capacity = completion.completed.capacity();
    let exact_roster = completion.ordered.clone();
    for pending_index in 0..completion.ordered.len() {
        assert!(
            !observe_entire_completion_roster(&completion.ordered, |entry| Ok::<bool, ()>(
                usize::from(entry.request_index) != pending_index
            ),)
            .unwrap()
        );
        assert_eq!(completion.ordered, exact_roster);
        assert_eq!(completion.ordered.as_ptr(), ordered_pointer);
        assert_eq!(completion.ordered.capacity(), ordered_capacity);
        assert_eq!(completion.completed.as_ptr(), completed_pointer);
        assert_eq!(completion.completed.capacity(), completed_capacity);
        assert!(completion.completed.is_empty());
    }
}

#[test]
fn successful_aggregate_api_does_not_expose_copyable_ticket_values() {
    let source = production_source();
    let shard_api = source
        .split("impl Gfx942SdmaMultiQueueShardTicketsV1 {")
        .nth(1)
        .unwrap()
        .split("impl fmt::Debug for Gfx942SdmaMultiQueueShardTicketsV1")
        .next()
        .unwrap();
    assert!(shard_api.contains("pub const fn ticket_count"));
    assert!(shard_api.contains("pub(crate) fn tickets"));
    assert!(!shard_api.contains("pub fn tickets"));
    assert!(!shard_api.contains("into_tickets"));

    let submission_api = source
        .split("impl Gfx942SdmaMultiQueueSubmissionV1 {")
        .nth(1)
        .unwrap()
        .split("pub struct Gfx942SdmaMultiQueueCompletedV1")
        .next()
        .unwrap();
    assert!(!submission_api.contains("into_shards"));

    let completion_entry = source
        .split("struct ValidatedMultiQueueCompletionEntryV1")
        .nth(1)
        .unwrap()
        .split("pub(crate) struct PreparedMultiQueueCompletionV1")
        .next()
        .unwrap();
    assert!(completion_entry.contains("ticket: Gfx942SdmaCopyTicketV1"));
    let observation = source
        .split("pub(crate) fn observe_prepared_striped_multi_queue_completion")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn retire_prepared_striped_multi_queue_completion")
        .next()
        .unwrap();
    assert!(observation.contains("owner.validate_ticket(entry.ticket)"));
}

#[test]
fn striped_queue_cursor_is_deterministic_and_wraps() {
    assert_eq!(next_striped_owner(0, 4).unwrap(), 1);
    assert_eq!(next_striped_owner(2, 4).unwrap(), 3);
    assert_eq!(next_striped_owner(3, 4).unwrap(), 0);
    assert!(next_striped_owner(0, 0).is_err());
    assert!(next_striped_owner(4, 4).is_err());
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InjectedMultiQueueFaultV1 {
    Preparation { call: usize },
    RecoverablePublication { call: usize },
    IndeterminatePublication { call: usize },
    ClosingCurrentness,
}

#[derive(Debug, Eq, PartialEq)]
struct InjectedMultiQueueOutcomeV1 {
    succeeded: bool,
    confirmed_queues: Vec<usize>,
    confirmed_requests: Vec<usize>,
    indeterminate_queue: Option<usize>,
    indeterminate_requests: Vec<usize>,
    untouched_requests: Vec<usize>,
    cursor: usize,
}

struct InjectedShardV1 {
    queue_ordinal: usize,
    queue_id: u32,
    request_indices: Vec<u16>,
    tickets: Vec<usize>,
}

struct InjectedSubmissionV1 {
    plan: Gfx942SdmaMultiQueuePlanV1,
    shards: Vec<InjectedShardV1>,
}

fn injected_multi_queue_outcome(
    plan: &Gfx942SdmaMultiQueuePlanV1,
    cursor: usize,
    fault: Option<InjectedMultiQueueFaultV1>,
) -> InjectedMultiQueueOutcomeV1 {
    fn shard_observations(shards: &[InjectedShardV1]) -> (Vec<usize>, Vec<usize>) {
        let queues = shards
            .iter()
            .map(|shard| {
                assert_eq!(shard.queue_id, 10 + shard.queue_ordinal as u32);
                assert_eq!(shard.request_indices.len(), shard.tickets.len());
                assert_eq!(
                    shard
                        .request_indices
                        .iter()
                        .map(|index| usize::from(*index))
                        .collect::<Vec<_>>(),
                    shard.tickets
                );
                shard.queue_ordinal
            })
            .collect();
        let mut requests = shards
            .iter()
            .flat_map(|shard| shard.request_indices.iter().copied())
            .map(usize::from)
            .collect::<Vec<_>>();
        requests.sort_unstable();
        (queues, requests)
    }

    let requests = (0..plan.request_count()).collect::<Vec<_>>();
    let mut preparation_call = 0;
    let prepared: PreparedMultiQueueSdmaBatchV1<Vec<usize>, InjectedShardV1, (u16, usize)> =
        match prepare_multi_queue_batch(
            plan.queue_ids().len(),
            plan.clone(),
            requests,
            |_, requests| {
                let this_call = preparation_call;
                preparation_call += 1;
                if fault == Some(InjectedMultiQueueFaultV1::Preparation { call: this_call }) {
                    Err((
                        Gfx942SdmaErrorV1::Contract("injected preparation failure"),
                        requests,
                    ))
                } else {
                    Ok(requests)
                }
            },
            |prepared| prepared,
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                assert!(matches!(
                    fault,
                    Some(InjectedMultiQueueFaultV1::Preparation { .. })
                ));
                return InjectedMultiQueueOutcomeV1 {
                    succeeded: false,
                    confirmed_queues: Vec::new(),
                    confirmed_requests: Vec::new(),
                    indeterminate_queue: None,
                    indeterminate_requests: Vec::new(),
                    untouched_requests: failure.requests,
                    cursor,
                };
            }
        };
    assert_eq!(prepared.published_capacity.len(), 0);
    assert!(prepared.published_capacity.capacity() >= plan.active_shard_count());
    assert_eq!(prepared.unpublished_capacity.len(), 0);
    assert!(prepared.unpublished_capacity.capacity() >= plan.request_count());

    let mut publication_call = 0;
    let published = publish_multi_queue_batch(
        prepared,
        |queue, prepared| {
            let this_call = publication_call;
            publication_call += 1;
            let queue_id = plan.queue_ids()[queue];
            match fault {
                Some(InjectedMultiQueueFaultV1::RecoverablePublication { call })
                    if call == this_call =>
                {
                    Err((
                        queue_id,
                        PreparedSdmaPublicationFailureV1::Recoverable {
                            error: Gfx942SdmaErrorV1::Contract(
                                "injected recoverable publication failure",
                            ),
                            prepared,
                        },
                    ))
                }
                Some(InjectedMultiQueueFaultV1::IndeterminatePublication { call })
                    if call == this_call =>
                {
                    Err((
                        queue_id,
                        PreparedSdmaPublicationFailureV1::Retained {
                            error: Gfx942SdmaErrorV1::Contract(
                                "injected indeterminate publication failure",
                            ),
                            tickets: prepared,
                        },
                    ))
                }
                _ => Ok((queue_id, prepared)),
            }
        },
        |prepared| prepared,
        |queue_ordinal, queue_id, request_indices, tickets| InjectedShardV1 {
            queue_ordinal,
            queue_id,
            request_indices,
            tickets,
        },
        |request_index, request| (request_index, request),
        |request| request.0,
        |plan, shards| InjectedSubmissionV1 { plan, shards },
    );
    match published {
        Ok(submission) => {
            assert_eq!(submission.plan, *plan);
            assert!(submission.shards.capacity() >= plan.active_shard_count());
            let (confirmed_queues, confirmed_requests) = shard_observations(&submission.shards);
            let succeeded = fault != Some(InjectedMultiQueueFaultV1::ClosingCurrentness);
            InjectedMultiQueueOutcomeV1 {
                succeeded,
                confirmed_queues,
                confirmed_requests,
                indeterminate_queue: None,
                indeterminate_requests: Vec::new(),
                untouched_requests: Vec::new(),
                cursor: if succeeded {
                    cursor_after_multi_queue_outcome(
                        cursor,
                        plan,
                        MultiQueueCursorOutcomeV1::CompleteSuccess,
                    )
                    .unwrap()
                } else {
                    cursor
                },
            }
        }
        Err(failure) => {
            assert!(failure.published.capacity() >= plan.active_shard_count());
            assert!(failure.unpublished.capacity() >= plan.request_count());
            let (confirmed_queues, confirmed_requests) = shard_observations(&failure.published);
            let (indeterminate_queue, indeterminate_requests) = failure
                .indeterminate
                .as_ref()
                .map(|shard| {
                    let (_, requests) = shard_observations(std::slice::from_ref(shard));
                    (Some(shard.queue_ordinal), requests)
                })
                .unwrap_or((None, Vec::new()));
            let untouched_requests = failure
                .unpublished
                .into_iter()
                .map(|(index, request)| {
                    assert_eq!(usize::from(index), request);
                    request
                })
                .collect();
            InjectedMultiQueueOutcomeV1 {
                succeeded: false,
                confirmed_queues,
                confirmed_requests,
                indeterminate_queue,
                indeterminate_requests,
                untouched_requests,
                cursor,
            }
        }
    }
}

#[test]
fn multi_queue_plan_rejects_invalid_duplicate_and_overcapacity_inputs() {
    assert_eq!(
        Gfx942SdmaMultiQueuePlanV1::new(&[], 1, 0),
        Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { actual: 0 })
    );
    assert_eq!(
        Gfx942SdmaMultiQueuePlanV1::new(&[1, 2, 3], 1, 0),
        Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { actual: 3 })
    );
    let too_many_queues = (0..=GFX942_SDMA_MAX_STRIPED_QUEUES_V1 as u32).collect::<Vec<_>>();
    assert!(matches!(
        Gfx942SdmaMultiQueuePlanV1::new(&too_many_queues, 1, 0),
        Err(Gfx942SdmaMultiQueuePlanErrorV1::QueueCount { .. })
    ));
    assert_eq!(
        Gfx942SdmaMultiQueuePlanV1::new(&[7, 8, 7, 9], 1, 0),
        Err(Gfx942SdmaMultiQueuePlanErrorV1::DuplicateQueueId { queue_id: 7 })
    );
    assert!(matches!(
        Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 0, 0),
        Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount { actual: 0, .. })
    ));
    assert!(matches!(
        Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 2 * GFX942_SDMA_MAX_IN_FLIGHT_V1 + 1, 0,),
        Err(Gfx942SdmaMultiQueuePlanErrorV1::RequestCount { .. })
    ));
    assert_eq!(
        Gfx942SdmaMultiQueuePlanV1::new(&[7, 8], 1, 2),
        Err(Gfx942SdmaMultiQueuePlanErrorV1::InvalidCursor {
            actual: 2,
            queue_count: 2,
        })
    );
}

#[test]
fn multi_queue_plan_is_balanced_deterministic_fair_and_current() {
    let queue_ids = [10, 11, 12, 13];
    let plan = Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 10, 2).unwrap();
    assert_eq!(
        (0..10)
            .map(|index| plan.queue_for_request(index).unwrap())
            .collect::<Vec<_>>(),
        [2, 3, 0, 1, 2, 3, 0, 1, 2, 3]
    );
    assert_eq!(
        (0..4)
            .map(|queue| plan.shard_count(queue).unwrap())
            .collect::<Vec<_>>(),
        [2, 2, 3, 3]
    );
    assert!(plan.is_balanced());
    assert_eq!(plan.active_shard_count(), 4);
    assert_eq!(plan.next_queue_after_success(), 0);
    assert!(plan.is_current_for(&queue_ids, 2));
    assert!(!plan.is_current_for(&[10, 12, 11, 13], 2));
    assert!(!plan.is_current_for(&queue_ids, 1));

    let mut cursor = 0;
    let mut selected = Vec::new();
    for _ in 0..8 {
        let single = Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, 1, cursor).unwrap();
        selected.push(single.queue_for_request(0).unwrap());
        cursor = single.next_queue_after_success();
    }
    assert_eq!(selected, [0, 1, 2, 3, 0, 1, 2, 3]);
}

#[test]
fn multi_queue_cursor_advances_only_after_complete_success() {
    let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 3, 1).unwrap();
    assert_eq!(
        cursor_after_multi_queue_outcome(1, &plan, MultiQueueCursorOutcomeV1::CompleteSuccess,)
            .unwrap(),
        0
    );
    for failure_stage in ["preparation", "partial-publication", "terminal"] {
        assert_eq!(
            cursor_after_multi_queue_outcome(1, &plan, MultiQueueCursorOutcomeV1::Failure,)
                .unwrap(),
            1,
            "{failure_stage} failure advanced the cursor",
        );
    }
    assert!(
        cursor_after_multi_queue_outcome(0, &plan, MultiQueueCursorOutcomeV1::CompleteSuccess,)
            .is_err()
    );
}

#[test]
fn multi_queue_partial_progress_accounting_is_exact() {
    let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 10, 2).unwrap();
    let published = [(2, [0_u16, 4, 8].as_slice())];
    let indeterminate = [(3, [1_u16, 5, 9].as_slice())];
    let unpublished = [2_usize, 3, 6, 7];
    assert!(multi_queue_custody_is_exact(
        &plan,
        published.into_iter().chain(indeterminate),
        unpublished,
    ));
    assert!(!multi_queue_custody_is_exact(
        &plan,
        [(2, [0_u16, 4, 8].as_slice()), (3, [1_u16, 5, 9].as_slice())],
        [2_usize, 3, 6, 6],
    ));
    assert!(!multi_queue_custody_is_exact(
        &plan,
        [(1, [0_u16, 4, 8].as_slice()), (3, [1_u16, 5, 9].as_slice())],
        unpublished,
    ));
}

#[test]
fn multi_queue_preflight_gate_rejects_hostile_ordering_without_publication() {
    let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 4, 2).unwrap();
    let mut preflight = MultiQueuePreflightStateV1::new(&plan);
    assert!(!preflight.publication_authorized);
    assert!(
        preflight
            .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
            .is_err()
    );
    assert!(preflight.record_prepared_queue(4).is_err());
    assert!(preflight.record_prepared_queue(2).is_ok());
    assert!(preflight.record_prepared_queue(2).is_err());
    assert!(preflight.authorize_publication().is_err());
    assert!(!preflight.publication_authorized);
    assert!(preflight.record_prepared_queue(3).is_ok());
    assert!(preflight.record_prepared_queue(0).is_ok());
    assert!(preflight.record_prepared_queue(1).is_ok());
    assert!(preflight.authorize_publication().is_ok());
    assert!(preflight.publication_authorized);
    assert!(preflight.record_prepared_queue(0).is_err());
    assert!(preflight.authorize_publication().is_err());
    assert!(
        preflight
            .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
            .is_ok()
    );
    assert!(
        preflight
            .record_publication_observation(2, MultiQueuePublicationObservationV1::Confirmed)
            .is_err()
    );
    assert!(!preflight.publication_is_complete());
}

#[test]
fn multi_queue_injected_coordinator_reports_exact_custody_and_cursor_outcomes() {
    let plan = Gfx942SdmaMultiQueuePlanV1::new(&[10, 11, 12, 13], 10, 2).unwrap();

    assert_eq!(
        injected_multi_queue_outcome(&plan, 2, None),
        InjectedMultiQueueOutcomeV1 {
            succeeded: true,
            confirmed_queues: vec![2, 3, 0, 1],
            confirmed_requests: (0..10).collect(),
            indeterminate_queue: None,
            indeterminate_requests: vec![],
            untouched_requests: vec![],
            cursor: 0,
        }
    );
    assert_eq!(
        injected_multi_queue_outcome(
            &plan,
            2,
            Some(InjectedMultiQueueFaultV1::Preparation { call: 1 }),
        ),
        InjectedMultiQueueOutcomeV1 {
            succeeded: false,
            confirmed_queues: vec![],
            confirmed_requests: vec![],
            indeterminate_queue: None,
            indeterminate_requests: vec![],
            untouched_requests: (0..10).collect(),
            cursor: 2,
        }
    );
    assert_eq!(
        injected_multi_queue_outcome(
            &plan,
            2,
            Some(InjectedMultiQueueFaultV1::RecoverablePublication { call: 1 }),
        ),
        InjectedMultiQueueOutcomeV1 {
            succeeded: false,
            confirmed_queues: vec![2],
            confirmed_requests: vec![0, 4, 8],
            indeterminate_queue: None,
            indeterminate_requests: vec![],
            untouched_requests: vec![1, 2, 3, 5, 6, 7, 9],
            cursor: 2,
        }
    );
    assert_eq!(
        injected_multi_queue_outcome(
            &plan,
            2,
            Some(InjectedMultiQueueFaultV1::IndeterminatePublication { call: 1 }),
        ),
        InjectedMultiQueueOutcomeV1 {
            succeeded: false,
            confirmed_queues: vec![2],
            confirmed_requests: vec![0, 4, 8],
            indeterminate_queue: Some(3),
            indeterminate_requests: vec![1, 5, 9],
            untouched_requests: vec![2, 3, 6, 7],
            cursor: 2,
        }
    );
    assert_eq!(
        injected_multi_queue_outcome(
            &plan,
            2,
            Some(InjectedMultiQueueFaultV1::ClosingCurrentness),
        ),
        InjectedMultiQueueOutcomeV1 {
            succeeded: false,
            confirmed_queues: vec![2, 3, 0, 1],
            confirmed_requests: (0..10).collect(),
            indeterminate_queue: None,
            indeterminate_requests: vec![],
            untouched_requests: vec![],
            cursor: 2,
        }
    );
}

#[test]
fn multi_queue_publication_requires_fully_prepared_private_custody() {
    let source = production_source();
    let coordinator = source
        .split("pub(crate) fn submit_striped_multi_queue_batch")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn commit_striped_multi_queue_success")
        .next()
        .unwrap();
    let prepare = coordinator.find("prepare_multi_queue_batch").unwrap();
    let publish = coordinator.find("publish_multi_queue_batch").unwrap();
    assert!(prepare < publish);

    let prepare_body = source
        .split("fn prepare_multi_queue_batch<")
        .nth(1)
        .unwrap()
        .split("fn publish_multi_queue_batch<")
        .next()
        .unwrap();
    assert!(coordinator.contains("prepare_batch_recoverable"));
    assert!(coordinator.contains("submit_prepared_batch_with_custody"));
    assert!(prepare_body.contains("prepare_shard(queue, queue_requests)"));
    assert!(!prepare_body.contains("publish_shard(queue_ordinal"));

    let publish_body = source
        .split("fn publish_multi_queue_batch<")
        .nth(1)
        .unwrap()
        .split("fn append_prepared_requests")
        .next()
        .unwrap();
    assert!(publish_body.contains("publish_shard(queue_ordinal, shard.batch)"));
    assert!(!publish_body.contains("try_reserve"));
    assert!(!publish_body.contains("Vec::new"));
    assert!(!publish_body.contains(".collect"));
    assert!(!publish_body.contains("to_string"));
}
