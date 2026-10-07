use super::xgmi_batch_wait::{
    completed_identities, fixture, retained_identities, write_completion,
};
use super::*;
use crate::shared_memory::PreparationMemoryFixtureV1;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct PollMemory<'a> {
    memory: &'a mut PreparationMemoryFixtureV1,
    observations: usize,
}

impl SdmaSingleMemoryV1 for PollMemory<'_> {
    fn check_queue_operational_currentness(&mut self) -> Result<(), MemorySessionError> {
        panic!("single poll must remain inside its caller's currentness scope")
    }

    fn observe_aql_control_counters_in_current_scope(
        &mut self,
        _: &mut SdmaControlAuthorityV1,
    ) -> Result<(u64, u64), MemorySessionError> {
        panic!("single poll must not observe queue counters")
    }

    fn single_host_facts(
        &self,
        _: &MappedHostBufferV1,
    ) -> Result<crate::shared_memory::SharedGttMappedResourceFactsV1, MemorySessionError> {
        panic!("single poll must not prepare a publication")
    }

    fn single_device_facts(
        &self,
        _: &Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
    ) -> Result<crate::shared_memory::Gfx942DeviceMemoryDispatchFactsV1, MemorySessionError> {
        panic!("single poll must not prepare a publication")
    }

    fn overwrite_mapped_host_visible_subrange_in_current_scope(
        &mut self,
        _: &mut MappedHostBufferV1,
        _: u64,
        _: &[u8],
    ) -> Result<(), MemorySessionError> {
        panic!("single poll must not reset a completion")
    }

    fn write_sdma_ring_slot_in_current_scope(
        &mut self,
        _: &mut SdmaRingAuthorityV1,
        _: u32,
        _: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        panic!("single poll must not publish a packet")
    }

    fn publish_sdma_control_write_release_in_current_scope(
        &mut self,
        _: &mut SdmaControlAuthorityV1,
        _: u64,
        _: u64,
    ) -> Result<(), MemorySessionError> {
        panic!("single poll must not publish a control counter")
    }

    fn observe_mapped_host_visible_i64_at_in_current_scope(
        &mut self,
        token: &mut MappedHostBufferV1,
        offset: u64,
    ) -> Result<i64, MemorySessionError> {
        self.observations += 1;
        self.memory
            .observe_mapped_host_visible_i64_at_in_current_scope(token, offset)
    }

    fn single_doorbell(
        &mut self,
        _: &mut LinuxDoorbellSliceV1,
        _: u64,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        panic!("single poll must not publish a doorbell")
    }
}

fn custody(ticket: Gfx942SdmaCopyTicketV1) -> ComputeXgmiCopyCustodyV1 {
    ComputeXgmiCopyCustodyV1 {
        ticket: Some(ticket),
        completed: None,
    }
}

#[test]
fn compute_xgmi_poll_repeated_pending_observes_once_and_preserves_exact_custody() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    let generations = owner.generations;
    let bytes = memory.sdma_mapped_bytes_v1(owner.completions.as_ref().unwrap());
    let mut custody = custody(tickets[0]);
    let mut memory = PollMemory {
        memory: &mut memory,
        observations: 0,
    };
    let mut closings = 0;
    for expected_observations in 1..=8 {
        let polled = owner.poll_xgmi_in_current_scope(&mut memory, tickets[0]);
        assert!(
            !custody
                .retain_poll_then_check(polled, || {
                    closings += 1;
                    Ok(())
                })
                .unwrap()
        );
        assert_eq!(memory.observations, expected_observations);
        assert_eq!(closings, expected_observations);
        assert_eq!(custody.ticket, Some(tickets[0]));
        assert!(custody.completed.is_none());
        assert_eq!(retained_identities(&owner), identities);
        assert_eq!(owner.generations, generations);
        assert!(!owner.is_poisoned());
        assert_eq!(
            memory
                .memory
                .sdma_mapped_bytes_v1(owner.completions.as_ref().unwrap()),
            bytes
        );
    }
}

#[test]
fn compute_xgmi_poll_ready_observes_once_and_retires_only_the_exact_pair() {
    for slot in 0..2 {
        let (mut memory, mut owner, tickets, identities) = fixture();
        write_completion(
            &mut memory,
            &mut owner,
            slot,
            i64::from(tickets[slot].generation),
        );
        let mut memory = PollMemory {
            memory: &mut memory,
            observations: 0,
        };
        let mut custody = custody(tickets[slot]);
        let polled = owner.poll_xgmi_in_current_scope(&mut memory, tickets[slot]);
        assert!(custody.retain_poll_then_check(polled, || Ok(())).unwrap());
        assert_eq!(memory.observations, 1);
        assert_eq!(custody.ticket, Some(tickets[slot]));
        assert_eq!(
            completed_identities(vec![custody.completed.take().unwrap()]),
            [identities[slot]]
        );
        assert_eq!(retained_identities(&owner), [identities[1 - slot]]);
        assert!(!owner.is_poisoned());
        assert!(
            owner
                .poll_xgmi_in_current_scope(&mut memory, tickets[slot])
                .is_err()
        );
        assert_eq!(memory.observations, 1);
        assert_eq!(retained_identities(&owner), [identities[1 - slot]]);
    }
}

#[test]
fn compute_xgmi_poll_malformed_tickets_never_observe_or_retire() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    let mut memory = PollMemory {
        memory: &mut memory,
        observations: 0,
    };
    for kind in 0..5 {
        let mut bad = tickets[0];
        match kind {
            0 => bad.owner = queue_key(8, 11, 13),
            1 => bad.queue_id += 1,
            2 => bad.slot = u16::MAX,
            3 => bad.generation += 1,
            _ => bad.slot = 2,
        }
        assert!(matches!(
            owner.poll_xgmi_in_current_scope(&mut memory, bad),
            Err(Gfx942SdmaErrorV1::Contract(_))
        ));
        assert_eq!(memory.observations, 0);
        assert_eq!(retained_identities(&owner), identities);
        assert!(!owner.is_poisoned());
    }
}

#[test]
fn compute_xgmi_poll_wrong_fences_preserve_native_error_and_all_mapping_owners() {
    for value in [-1, 1, 4, i64::MAX] {
        let (mut memory, mut owner, tickets, identities) = fixture();
        write_completion(&mut memory, &mut owner, 0, value);
        let mut memory = PollMemory {
            memory: &mut memory,
            observations: 0,
        };
        let mut custody = custody(tickets[0]);
        let polled = owner.poll_xgmi_in_current_scope(&mut memory, tickets[0]);
        assert!(matches!(
            custody.retain_poll_then_check(polled, || panic!("native error takes precedence")),
            Err(Gfx942SdmaErrorV1::Contract(
                "unexpected XGMI SDMA completion value"
            ))
        ));
        assert_eq!(memory.observations, 1);
        assert!(owner.is_poisoned());
        assert_eq!(custody.ticket, Some(tickets[0]));
        assert!(custody.completed.is_none());
        assert_eq!(retained_identities(&owner), identities);
    }
}

#[test]
fn compute_xgmi_poll_mapped_observation_error_preserves_exact_custody() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    let short_cpu = memory.allocate::<HostVisibleCoherentGttV1>(8).unwrap();
    let short = memory.map(short_cpu).unwrap();
    let _original_completion_arena = owner.completions.replace(short);
    let mut memory = PollMemory {
        memory: &mut memory,
        observations: 0,
    };
    let mut custody = custody(tickets[1]);
    // Slot one lies beyond the real mapped arena's requested eight-byte extent.
    let polled = owner.poll_xgmi_in_current_scope(&mut memory, tickets[1]);
    assert!(matches!(
        custody.retain_poll_then_check(polled, || panic!("native error takes precedence")),
        Err(Gfx942SdmaErrorV1::Memory(
            MemorySessionError::KernelResultMalformed("fake acquired i64 range")
        ))
    ));
    assert_eq!(memory.observations, 1);
    assert_eq!(custody.ticket, Some(tickets[1]));
    assert!(custody.completed.is_none());
    assert_eq!(retained_identities(&owner), identities);
}

#[test]
fn compute_xgmi_poll_mapped_observation_unwind_preserves_exact_custody_and_payload() {
    let (mut memory, mut owner, tickets, identities) = fixture();
    memory.sdma_mapping_panic_v1(owner.completions.as_ref().unwrap(), "observe_i64_acquire");
    let mut memory = PollMemory {
        memory: &mut memory,
        observations: 0,
    };
    let mut custody = custody(tickets[0]);
    let payload = catch_unwind(AssertUnwindSafe(|| {
        let polled = owner.poll_xgmi_in_current_scope(&mut memory, tickets[0]);
        let _ = custody.retain_poll_then_check(polled, || panic!("observation unwound"));
    }))
    .unwrap_err();
    assert_eq!(
        payload.downcast_ref::<(&'static str, &'static str)>(),
        Some(&("N1 mapped panic", "observe_i64_acquire"))
    );
    assert_eq!(memory.observations, 1);
    assert_eq!(custody.ticket, Some(tickets[0]));
    assert!(custody.completed.is_none());
    assert_eq!(retained_identities(&owner), identities);
}

#[test]
fn compute_xgmi_poll_roots_pending_or_completed_custody_before_every_closing_outcome() {
    for ready in [false, true] {
        for fault in 0..3 {
            let (mut memory, mut owner, tickets, identities) = fixture();
            if ready {
                write_completion(&mut memory, &mut owner, 0, i64::from(tickets[0].generation));
            }
            let mut memory = PollMemory {
                memory: &mut memory,
                observations: 0,
            };
            let mut custody = custody(tickets[0]);
            let mut closings = 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let polled = owner.poll_xgmi_in_current_scope(&mut memory, tickets[0]);
                custody.retain_poll_then_check(polled, || {
                    closings += 1;
                    match fault {
                        0 => Ok(()),
                        1 => Err(Gfx942SdmaErrorV1::Contract("closing currentness")),
                        _ => panic!("closing currentness"),
                    }
                })
            }));
            assert_eq!(memory.observations, 1);
            assert_eq!(closings, 1);
            assert_eq!(result.is_err(), fault == 2);
            if let Ok(result) = result {
                match fault {
                    0 => assert_eq!(result.unwrap(), ready),
                    1 => assert!(matches!(
                        result,
                        Err(Gfx942SdmaErrorV1::Contract("closing currentness"))
                    )),
                    _ => unreachable!(),
                }
            }
            assert_eq!(custody.ticket, Some(tickets[0]));
            if ready {
                assert_eq!(
                    completed_identities(vec![custody.completed.take().unwrap()]),
                    [identities[0]]
                );
                assert_eq!(retained_identities(&owner), [identities[1]]);
            } else {
                assert!(custody.completed.is_none());
                assert_eq!(retained_identities(&owner), identities);
            }
        }
    }
}

#[test]
fn compute_xgmi_rooted_poll_uses_one_lower_sample_between_full_route_checks() {
    let source = include_str!("../compute_xgmi.rs");
    let method = source
        .split("pub(crate) fn poll_compute_xgmi_rooted_v1(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn wait_compute_xgmi_rooted_v1(")
        .next()
        .unwrap();
    let checks: Vec<_> = method
        .match_indices("Self::validate_route_currentness(")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(checks.len(), 2);
    assert_eq!(method.matches("XgmiRouteCurrentnessV1::Full").count(), 2);
    assert_eq!(method.matches(".poll_xgmi_in_current_scope(").count(), 1);
    let sample = method.find(".poll_xgmi_in_current_scope(").unwrap();
    let root = method.find("custody.retain_poll_then_check(").unwrap();
    assert!(checks[0] < sample && sample < root && root < checks[1]);
    assert!(method.find("custody.completed.is_some()").unwrap() < checks[0]);
    for blocking in ["wait_", "Duration", "sleep(", "loop {", "while "] {
        assert!(!method.contains(blocking));
    }
}
