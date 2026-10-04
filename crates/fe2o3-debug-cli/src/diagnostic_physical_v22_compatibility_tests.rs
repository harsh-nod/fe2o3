//! Closed old-profile configuration and scope bytes remain unchanged.
use super::*;
#[test]
fn legacy_configuration_hash_bytes_remain_exact() {
    for profile in [Profile::EntryV20, Profile::GlobalCopyV21] {
        let limits = fe2o3_kir_sim::SimulationLimitsV1::default();
        let actual = configuration_for(profile, &[1; 32], 123, &[2; 32], 456, limits).unwrap();
        let mut h = Sha256::new();
        let expected_domain: &[u8] = match profile {
            Profile::EntryV20 => b"fe2o3-debug-physical-entry-v20-cpu-config-v1\0",
            Profile::GlobalCopyV21 => b"fe2o3-debug-physical-global-copy-v21-cpu-config-v1\0",
            Profile::LdsExchangeV22 => unreachable!("legacy-only census"),
        };
        h.update(expected_domain);
        h.update([1; 32]);
        h.update(123u64.to_le_bytes());
        h.update([2; 32]);
        h.update(456u64.to_le_bytes());
        for n in [
            LINE, RESPONSE, PAGE, COMMANDS, WORK, STORAGE, 8192, QUERY_WORK, CELL, SCRATCH, 1, 768,
            8, 16384,
        ] {
            h.update((n as u64).to_le_bytes());
        }
        let l = limits;
        for n in [
            l.max_canonical_bytes,
            l.max_reachable_functions,
            l.max_reachable_operations,
            l.max_call_depth,
            l.max_ssa_values,
            l.max_allocations,
            l.max_allocation_bytes,
            l.max_total_bytes,
            l.max_resident_bytes,
            l.max_memory_access_records,
        ] {
            h.update((n as u64).to_le_bytes());
        }
        for n in [
            l.max_invocations,
            l.max_workgroups,
            l.max_scheduled_slots,
            l.max_steps,
            l.max_events,
        ] {
            h.update(n.to_le_bytes());
        }
        assert_eq!(actual.as_bytes(), <[u8; 32]>::from(h.finalize()));
        for local in 0..64 {
            assert_eq!(profile.logical_wave_lane(local), (0, local as u16));
        }
    }
}
#[test]
fn new_scope_and_hash_domains_do_not_alias_existing_profiles() {
    assert_eq!(Profile::LdsExchangeV22.logical_wave_lane(0), (0, 0));
    assert_eq!(Profile::LdsExchangeV22.logical_wave_lane(63), (0, 63));
    assert_eq!(Profile::LdsExchangeV22.logical_wave_lane(64), (1, 0));
    assert_eq!(Profile::LdsExchangeV22.logical_wave_lane(127), (1, 63));
    for old in [Profile::EntryV20, Profile::GlobalCopyV21] {
        assert_ne!(
            old.configuration_domain(),
            Profile::LdsExchangeV22.configuration_domain()
        );
        assert_ne!(old.page_domain(), Profile::LdsExchangeV22.page_domain());
    }
}
