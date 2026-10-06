#![cfg(all(
    feature = "engineering-gfx950",
    target_os = "linux",
    target_arch = "x86_64"
))]

use fe2o3_kfd::{
    Gfx950EngineeringPeerGroupV1 as Group, Gfx950EngineeringPeerGuardedMlpBankEntryV1 as BankEntry,
    Gfx950EngineeringPeerGuardedMlpInputsV1 as Inputs,
    Gfx950EngineeringPeerGuardedMlpObservationV1 as Observation,
    Gfx950EngineeringPeerGuardedMlpRankInputsV1 as RankInputs,
    Gfx950EngineeringPeerRetainedGuardedMlpPairV1 as Pair,
    Gfx950EngineeringPeerUnboundGuardedMlpPairV1 as Unbound,
};

#[test]
fn guarded_facade_pair_types_have_no_borrowed_lifetime() {
    fn require_static<T: 'static>() {}
    require_static::<Pair>();
    require_static::<Unbound>();
    require_static::<Observation>();
    require_static::<RankInputs<'static>>();
    require_static::<Inputs<'static>>();
    require_static::<BankEntry<'static>>();
    fn copy_fields(value: Observation) -> ([[u32; 548]; 2], [[u32; 4]; 2], [(u64, u64); 2], u64) {
        (
            value.prefixes,
            value.guards,
            value.observed_queue_frontiers,
            value.segment_host_ns,
        )
    }
    let _ = copy_fields;
}

#[test]
fn guarded_facade_group_signatures_preserve_linear_custody() {
    let _: fn(&mut Group, u32) -> Result<Unbound, String> =
        Group::allocate_guarded_mlp_pair_storage_v1;
    let _: for<'g, 'i, 'k> unsafe fn(
        &'g mut Group,
        Unbound,
        &'i Inputs<'k>,
        u32,
    ) -> Result<Pair, String> = Group::bind_guarded_mlp_pair_unchecked_v1;
    let _: for<'g, 'i, 'k> unsafe fn(
        &'g mut Group,
        Unbound,
        &'i Inputs<'k>,
        u32,
    ) -> Result<Pair, String> = Group::bind_guarded_mlp_pair_exact_own_residual_unchecked_v1;
    let _: for<'g, 'p, 'k> fn(
        &'g mut Group,
        &'p mut Pair,
        Inputs<'k>,
        u32,
    ) -> Result<Observation, String> = Group::dispatch_guarded_mlp_pair_v1;
}

#[test]
fn guarded_facade_mixed_bank_signatures_are_scoped_and_explicit() {
    let _: for<'g, 's, 'e> fn(&'g mut Group, &'s mut [BankEntry<'e>], u32) -> Result<u64, String> =
        Group::validate_guarded_mlp_initial_bank_v1;
    let _: for<'g, 's, 'e> unsafe fn(
        &'g mut Group,
        &'s mut [BankEntry<'e>],
        u32,
    ) -> Result<u64, String> = Group::rearm_guarded_mlp_bank_unchecked_v1;
}
