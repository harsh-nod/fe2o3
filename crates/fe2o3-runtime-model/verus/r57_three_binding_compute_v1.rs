// Independent finite R57 summary model for one bounded three-binding
// persistent-device compute transaction. All allocation, storage, device, VM,
// queue, binder, packet, completion, frontier, generation, and effect facts are
// mathematical inputs. This proves no executable-Rust refinement and no KFD,
// HSA, HIP, packet encoder, signal, firmware, hardware, progress, parity, or
// performance claim.

use vstd::prelude::*;

verus! {

pub open spec fn binding_count_v1() -> nat { 3 }
pub open spec fn packet_count_v1() -> nat { 2 }

#[derive(PartialEq, Eq)]
pub enum MemoryKindV1 { PersistentHbm, Host }

#[derive(PartialEq, Eq)]
pub enum RoleV1 { Read, Write }

#[derive(PartialEq, Eq)]
pub enum EffectV1 { ReadOnly, WriteOnly }

#[derive(PartialEq, Eq)]
pub enum PacketKindV1 { WaitForPrior, Dispatch }

#[derive(PartialEq, Eq)]
pub struct AllocationV1 {
    pub owner: nat,
    pub allocation: nat,
    pub storage: nat,
    pub allocation_generation: nat,
    pub device: nat,
    pub vm: nat,
    pub byte_len: nat,
    pub memory_kind: MemoryKindV1,
    pub initialized: bool,
    pub content_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct BindingV1 {
    pub ordinal: nat,
    pub allocation: nat,
    pub storage: nat,
    pub allocation_generation: nat,
    pub byte_offset: nat,
    pub byte_len: nat,
    pub role: RoleV1,
    pub effect: EffectV1,
    pub requires_initialized: bool,
}

#[derive(PartialEq, Eq)]
pub struct PacketV1 {
    pub packet: nat,
    pub kind: PacketKindV1,
    pub order: nat,
}

#[derive(PartialEq, Eq)]
pub struct PlanV1 {
    pub device: nat,
    pub vm: nat,
    pub queue: nat,
    pub queue_generation: nat,
    pub fixed_binder_authority: nat,
    pub kernel: nat,
    pub dispatch: nat,
    pub transaction_generation: nat,
    pub frontier_before: nat,
    pub completion_signal: nat,
    pub wait_packet: PacketV1,
    pub dispatch_packet: PacketV1,
    pub a: BindingV1,
    pub b: BindingV1,
    pub c: BindingV1,
}

#[derive(PartialEq, Eq)]
pub struct OwnersV1 {
    pub a: AllocationV1,
    pub b: AllocationV1,
    pub c: AllocationV1,
}

pub open spec fn valid_allocation_v1(allocation: AllocationV1) -> bool {
    allocation.owner > 0
        && allocation.allocation > 0
        && allocation.storage > 0
        && allocation.allocation_generation > 0
        && allocation.device > 0
        && allocation.vm > 0
        && allocation.byte_len > 0
        && allocation.memory_kind == MemoryKindV1::PersistentHbm
        && allocation.content_generation > 0
}

pub open spec fn exact_binding_v1(
    plan: PlanV1,
    allocation: AllocationV1,
    binding: BindingV1,
) -> bool {
    valid_allocation_v1(allocation)
        && allocation.allocation == binding.allocation
        && allocation.storage == binding.storage
        && allocation.allocation_generation == binding.allocation_generation
        && allocation.device == plan.device
        && allocation.vm == plan.vm
        && binding.byte_offset == 0
        && binding.byte_len == allocation.byte_len
        && (!binding.requires_initialized || allocation.initialized)
}

pub open spec fn exact_plan_v1(plan: PlanV1) -> bool {
    plan.device > 0
        && plan.vm > 0
        && plan.queue > 0
        && plan.queue_generation > 0
        && plan.fixed_binder_authority > 0
        && plan.kernel > 0
        && plan.dispatch > 0
        && plan.transaction_generation > 0
        && plan.completion_signal > 0
        && plan.wait_packet.packet > 0
        && plan.wait_packet.kind == PacketKindV1::WaitForPrior
        && plan.wait_packet.order == 0
        && plan.dispatch_packet.packet > 0
        && plan.dispatch_packet.packet != plan.wait_packet.packet
        && plan.dispatch_packet.kind == PacketKindV1::Dispatch
        && plan.dispatch_packet.order == 1
        && plan.a.ordinal == 0
        && plan.a.role == RoleV1::Read
        && plan.a.effect == EffectV1::ReadOnly
        && plan.a.requires_initialized
        && plan.b.ordinal == 1
        && plan.b.role == RoleV1::Read
        && plan.b.effect == EffectV1::ReadOnly
        && plan.b.requires_initialized
        && plan.c.ordinal == 2
        && plan.c.role == RoleV1::Write
        && plan.c.effect == EffectV1::WriteOnly
        && !plan.c.requires_initialized
        && plan.a.byte_len == plan.b.byte_len
        && plan.a.byte_len == plan.c.byte_len
        && plan.a.allocation != plan.b.allocation
        && plan.a.allocation != plan.c.allocation
        && plan.b.allocation != plan.c.allocation
        && plan.a.storage != plan.b.storage
        && plan.a.storage != plan.c.storage
        && plan.b.storage != plan.c.storage
}

pub open spec fn admitted_v1(plan: PlanV1, owners: OwnersV1) -> bool {
    exact_plan_v1(plan)
        && exact_binding_v1(plan, owners.a, plan.a)
        && exact_binding_v1(plan, owners.b, plan.b)
        && exact_binding_v1(plan, owners.c, plan.c)
        && owners.a.initialized
        && owners.b.initialized
        && owners.a.allocation != owners.b.allocation
        && owners.a.allocation != owners.c.allocation
        && owners.b.allocation != owners.c.allocation
        && owners.a.storage != owners.b.storage
        && owners.a.storage != owners.c.storage
        && owners.b.storage != owners.c.storage
        && owners.a.owner != owners.b.owner
        && owners.a.owner != owners.c.owner
        && owners.b.owner != owners.c.owner
}

pub open spec fn hostile_cardinality_admitted_v1(count: nat) -> bool { count == 3 }

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { Published, Restored, Quarantined, Completed }

#[derive(PartialEq, Eq)]
pub enum ReasonV1 {
    None,
    OpeningCurrentnessRejected,
    NativeRejectedBeforeEffect,
    AmbiguousAfterWaitPublication,
    AmbiguousAfterDispatchPublication,
    CompletionIdentityMismatch,
    ClosingCurrentnessRejected,
}

#[derive(PartialEq, Eq)]
pub enum PublicationScriptV1 {
    Complete,
    RejectBeforePublication,
    AmbiguousAfterWaitPublication,
    AmbiguousAfterDispatchPublication,
}

#[derive(PartialEq, Eq)]
pub struct OutcomeV1 {
    pub phase: PhaseV1,
    pub reason: ReasonV1,
    pub plan: PlanV1,
    pub owners: OwnersV1,
    pub published_packet_prefix: nat,
}

pub open spec fn publish_v1(
    plan: PlanV1,
    owners: OwnersV1,
    opening_currentness: bool,
    script: PublicationScriptV1,
) -> OutcomeV1 {
    if !opening_currentness {
        OutcomeV1 { phase: PhaseV1::Restored,
            reason: ReasonV1::OpeningCurrentnessRejected, plan, owners,
            published_packet_prefix: 0 }
    } else {
        match script {
            PublicationScriptV1::Complete => OutcomeV1 { phase: PhaseV1::Published,
                reason: ReasonV1::None, plan, owners, published_packet_prefix: 2 },
            PublicationScriptV1::RejectBeforePublication => OutcomeV1 {
                phase: PhaseV1::Restored, reason: ReasonV1::NativeRejectedBeforeEffect,
                plan, owners, published_packet_prefix: 0 },
            PublicationScriptV1::AmbiguousAfterWaitPublication => OutcomeV1 {
                phase: PhaseV1::Quarantined,
                reason: ReasonV1::AmbiguousAfterWaitPublication,
                plan, owners, published_packet_prefix: 1 },
            PublicationScriptV1::AmbiguousAfterDispatchPublication => OutcomeV1 {
                phase: PhaseV1::Quarantined,
                reason: ReasonV1::AmbiguousAfterDispatchPublication,
                plan, owners, published_packet_prefix: 2 },
        }
    }
}

#[derive(PartialEq, Eq)]
pub struct CompletionV1 {
    pub queue: nat,
    pub queue_generation: nat,
    pub dispatch: nat,
    pub transaction_generation: nat,
    pub completion_signal: nat,
    pub frontier_after: nat,
}

pub open spec fn exact_completion_v1(plan: PlanV1, completion: CompletionV1) -> bool {
    completion.queue == plan.queue
        && completion.queue_generation == plan.queue_generation
        && completion.dispatch == plan.dispatch
        && completion.transaction_generation == plan.transaction_generation
        && completion.completion_signal == plan.completion_signal
        && completion.frontier_after == plan.frontier_before + packet_count_v1()
}

pub open spec fn completed_output_v1(output: AllocationV1) -> AllocationV1 {
    AllocationV1 {
        owner: output.owner,
        allocation: output.allocation,
        storage: output.storage,
        allocation_generation: output.allocation_generation,
        device: output.device,
        vm: output.vm,
        byte_len: output.byte_len,
        memory_kind: output.memory_kind,
        initialized: true,
        content_generation: output.content_generation + 1,
    }
}

pub open spec fn complete_v1(
    published: OutcomeV1,
    closing_currentness: bool,
    completion: CompletionV1,
) -> OutcomeV1 {
    if published.phase == PhaseV1::Published
        && closing_currentness
        && exact_completion_v1(published.plan, completion) {
        OutcomeV1 {
            phase: PhaseV1::Completed,
            reason: ReasonV1::None,
            plan: published.plan,
            owners: OwnersV1 {
                a: published.owners.a,
                b: published.owners.b,
                c: completed_output_v1(published.owners.c),
            },
            published_packet_prefix: published.published_packet_prefix,
        }
    } else if published.phase == PhaseV1::Published {
        OutcomeV1 {
            phase: PhaseV1::Quarantined,
            reason: if closing_currentness {
                ReasonV1::CompletionIdentityMismatch
            } else {
                ReasonV1::ClosingCurrentnessRejected
            },
            plan: published.plan,
            owners: published.owners,
            published_packet_prefix: published.published_packet_prefix,
        }
    } else { published }
}

pub open spec fn observe_quarantine_again_v1(quarantined: OutcomeV1) -> OutcomeV1 {
    quarantined
}

// Obligation 1: the admitted binding and packet counts are exact.
pub proof fn exact_constants_v1()
    ensures binding_count_v1() == 3, packet_count_v1() == 2,
{}

// Obligation 2: hostile cardinality three is admitted.
pub proof fn exact_cardinality_is_admitted_v1()
    ensures hostile_cardinality_admitted_v1(3),
{}

// Obligation 3: every hostile cardinality below three rejects.
pub proof fn cardinality_below_three_is_rejected_v1(count: nat)
    requires count < 3,
    ensures !hostile_cardinality_admitted_v1(count),
{}

// Obligation 4: every hostile cardinality above three rejects.
pub proof fn cardinality_above_three_is_rejected_v1(count: nat)
    requires count > 3,
    ensures !hostile_cardinality_admitted_v1(count),
{}

// Obligation 5: A and B are ordered read-only bindings and C is write-only.
pub proof fn exact_roles_effects_and_order_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures plan.a.ordinal == 0, plan.a.role == RoleV1::Read,
        plan.a.effect == EffectV1::ReadOnly,
        plan.b.ordinal == 1, plan.b.role == RoleV1::Read,
        plan.b.effect == EffectV1::ReadOnly,
        plan.c.ordinal == 2, plan.c.role == RoleV1::Write,
        plan.c.effect == EffectV1::WriteOnly,
{}

// Obligation 6: both inputs open initialized.
pub proof fn inputs_are_initialized_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures owners.a.initialized, owners.b.initialized,
{}

// Obligation 7: C's opening initialization state is not required.
pub proof fn output_opening_initialization_is_unconstrained_v1(plan: PlanV1)
    requires exact_plan_v1(plan),
    ensures !plan.c.requires_initialized,
{}

// Obligation 8: the three allocation identities are pairwise distinct.
pub proof fn allocations_are_pairwise_distinct_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures owners.a.allocation != owners.b.allocation,
        owners.a.allocation != owners.c.allocation,
        owners.b.allocation != owners.c.allocation,
{}

// Obligation 9: the three storage identities are pairwise distinct.
pub proof fn storage_is_pairwise_distinct_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures owners.a.storage != owners.b.storage,
        owners.a.storage != owners.c.storage,
        owners.b.storage != owners.c.storage,
{}

// Obligation 10: the three linear owner occurrences are pairwise distinct.
pub proof fn owner_occurrences_are_pairwise_distinct_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures owners.a.owner != owners.b.owner,
        owners.a.owner != owners.c.owner,
        owners.b.owner != owners.c.owner,
{}

// Obligation 11: every binding covers an equal, exact full allocation extent.
pub proof fn bindings_are_full_extent_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures plan.a.byte_offset == 0, plan.a.byte_len == owners.a.byte_len,
        plan.b.byte_offset == 0, plan.b.byte_len == owners.b.byte_len,
        plan.c.byte_offset == 0, plan.c.byte_len == owners.c.byte_len,
        plan.a.byte_len == plan.b.byte_len, plan.a.byte_len == plan.c.byte_len,
{}

// Obligation 12: every owner is on the transaction device.
pub proof fn owners_share_exact_device_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures owners.a.device == plan.device, owners.b.device == plan.device,
        owners.c.device == plan.device,
{}

// Obligation 13: every owner is in the transaction VM.
pub proof fn owners_share_exact_vm_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures owners.a.vm == plan.vm, owners.b.vm == plan.vm, owners.c.vm == plan.vm,
{}

// Obligation 14: all admitted allocations are persistent HBM.
pub proof fn owners_are_persistent_hbm_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures owners.a.memory_kind == MemoryKindV1::PersistentHbm,
        owners.b.memory_kind == MemoryKindV1::PersistentHbm,
        owners.c.memory_kind == MemoryKindV1::PersistentHbm,
{}

// Obligation 15: the fixed binder authority and launch identities are nonzero.
pub proof fn fixed_binder_and_launch_are_authorized_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures plan.fixed_binder_authority > 0, plan.kernel > 0, plan.dispatch > 0,
        plan.transaction_generation > 0,
{}

// Obligation 16: the first and only control packet is WaitForPrior.
pub proof fn one_wait_for_prior_is_first_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures plan.wait_packet.kind == PacketKindV1::WaitForPrior,
        plan.wait_packet.order == 0,
{}

// Obligation 17: the second and only work packet is the dispatch.
pub proof fn one_dispatch_is_second_v1(plan: PlanV1, owners: OwnersV1)
    requires admitted_v1(plan, owners),
    ensures plan.dispatch_packet.kind == PacketKindV1::Dispatch,
        plan.dispatch_packet.order == 1,
        plan.dispatch_packet.packet != plan.wait_packet.packet,
{}

// Obligation 18: WaitForPrior plus dispatch advances the frontier by exactly two.
pub proof fn exact_packet_pair_advances_frontier_by_two_v1(plan: PlanV1)
    ensures plan.frontier_before + packet_count_v1() == plan.frontier_before + 2,
{}

// Obligation 19: opening-currentness rejection restores exact owners.
pub proof fn opening_currentness_failure_restores_exact_owners_v1(
    plan: PlanV1, owners: OwnersV1, script: PublicationScriptV1,
)
    ensures {
        let out = publish_v1(plan, owners, false, script);
        out.phase == PhaseV1::Restored && out.owners == owners
            && out.plan == plan && out.published_packet_prefix == 0
    },
{}

// Obligation 20: a native prepublication rejection restores exact owners.
pub proof fn native_prepublication_failure_restores_exact_owners_v1(
    plan: PlanV1, owners: OwnersV1,
)
    ensures {
        let out = publish_v1(plan, owners, true, PublicationScriptV1::RejectBeforePublication);
        out.phase == PhaseV1::Restored && out.owners == owners
            && out.reason == ReasonV1::NativeRejectedBeforeEffect
    },
{}

// Obligation 21: every restored outcome has a zero publication prefix.
pub proof fn prepublication_restoration_has_no_native_effect_v1(
    plan: PlanV1, owners: OwnersV1, currentness: bool,
)
    ensures {
        let out = publish_v1(plan, owners, currentness,
            PublicationScriptV1::RejectBeforePublication);
        out.phase == PhaseV1::Restored ==> out.published_packet_prefix == 0
    },
{}

// Obligation 22: restoration is all three owners, never a partial roster.
pub proof fn restoration_is_all_or_none_v1(plan: PlanV1, owners: OwnersV1)
    ensures {
        let out = publish_v1(plan, owners, true,
            PublicationScriptV1::RejectBeforePublication);
        out.owners.a == owners.a && out.owners.b == owners.b && out.owners.c == owners.c
            && binding_count_v1() == 3
    },
{}

// Obligation 23: complete publication retains the exact transaction and owners.
pub proof fn complete_publication_retains_exact_transaction_v1(
    plan: PlanV1, owners: OwnersV1,
)
    ensures {
        let out = publish_v1(plan, owners, true, PublicationScriptV1::Complete);
        out.phase == PhaseV1::Published && out.plan == plan && out.owners == owners
            && out.published_packet_prefix == 2
    },
{}

// Obligation 24: ambiguity after WaitForPrior quarantines prefix one.
pub proof fn wait_publication_ambiguity_quarantines_all_v1(
    plan: PlanV1, owners: OwnersV1,
)
    ensures {
        let out = publish_v1(plan, owners, true,
            PublicationScriptV1::AmbiguousAfterWaitPublication);
        out.phase == PhaseV1::Quarantined && out.owners == owners
            && out.published_packet_prefix == 1
    },
{}

// Obligation 25: ambiguity after dispatch quarantines prefix two.
pub proof fn dispatch_publication_ambiguity_quarantines_all_v1(
    plan: PlanV1, owners: OwnersV1,
)
    ensures {
        let out = publish_v1(plan, owners, true,
            PublicationScriptV1::AmbiguousAfterDispatchPublication);
        out.phase == PhaseV1::Quarantined && out.owners == owners
            && out.published_packet_prefix == 2
    },
{}

// Obligation 26: every modeled postpublication ambiguity retains all identities.
pub proof fn quarantine_preserves_every_owner_identity_v1(
    plan: PlanV1, owners: OwnersV1, script: PublicationScriptV1,
)
    requires script == PublicationScriptV1::AmbiguousAfterWaitPublication
        || script == PublicationScriptV1::AmbiguousAfterDispatchPublication,
    ensures {
        let out = publish_v1(plan, owners, true, script);
        out.phase == PhaseV1::Quarantined && out.owners == owners && out.plan == plan
    },
{}

// Obligation 27: quarantine is absorbing under later observation.
pub proof fn quarantine_is_absorbing_v1(quarantined: OutcomeV1)
    requires quarantined.phase == PhaseV1::Quarantined,
    ensures observe_quarantine_again_v1(quarantined) == quarantined,
{}

// Obligation 28: the completion identity authenticates every coordinate.
pub proof fn exact_completion_authenticates_all_coordinates_v1(
    plan: PlanV1, completion: CompletionV1,
)
    requires exact_completion_v1(plan, completion),
    ensures completion.queue == plan.queue,
        completion.queue_generation == plan.queue_generation,
        completion.dispatch == plan.dispatch,
        completion.transaction_generation == plan.transaction_generation,
        completion.completion_signal == plan.completion_signal,
        completion.frontier_after == plan.frontier_before + 2,
{}

// Obligation 29: exact completion moves a published transaction to Completed.
pub proof fn exact_completion_completes_v1(
    plan: PlanV1, owners: OwnersV1, completion: CompletionV1,
)
    requires exact_completion_v1(plan, completion),
    ensures {
        let published = publish_v1(plan, owners, true, PublicationScriptV1::Complete);
        complete_v1(published, true, completion).phase == PhaseV1::Completed
    },
{}

// Obligation 30: an inexact completion quarantines all three owners.
pub proof fn inexact_completion_quarantines_all_v1(
    plan: PlanV1, owners: OwnersV1, completion: CompletionV1,
)
    requires !exact_completion_v1(plan, completion),
    ensures {
        let published = publish_v1(plan, owners, true, PublicationScriptV1::Complete);
        let out = complete_v1(published, true, completion);
        out.phase == PhaseV1::Quarantined && out.owners == owners
            && out.reason == ReasonV1::CompletionIdentityMismatch
    },
{}

// Obligation 31: successful completion leaves both read inputs unchanged.
pub proof fn completion_preserves_both_inputs_v1(
    plan: PlanV1, owners: OwnersV1, completion: CompletionV1,
)
    requires exact_completion_v1(plan, completion),
    ensures {
        let published = publish_v1(plan, owners, true, PublicationScriptV1::Complete);
        let out = complete_v1(published, true, completion);
        out.owners.a == owners.a && out.owners.b == owners.b
    },
{}

// Obligation 32: completion initializes C and advances only its content generation.
pub proof fn completion_initializes_and_advances_output_once_v1(
    plan: PlanV1, owners: OwnersV1, completion: CompletionV1,
)
    requires exact_completion_v1(plan, completion),
    ensures {
        let published = publish_v1(plan, owners, true, PublicationScriptV1::Complete);
        let out = complete_v1(published, true, completion);
        out.owners.c.initialized
            && out.owners.c.content_generation == owners.c.content_generation + 1
            && out.owners.a.content_generation == owners.a.content_generation
            && out.owners.b.content_generation == owners.b.content_generation
    },
{}

// Obligation 33: completion preserves every allocation/storage/generation identity.
pub proof fn completion_preserves_all_storage_identities_v1(
    plan: PlanV1, owners: OwnersV1, completion: CompletionV1,
)
    requires exact_completion_v1(plan, completion),
    ensures {
        let published = publish_v1(plan, owners, true, PublicationScriptV1::Complete);
        let out = complete_v1(published, true, completion);
        out.owners.a.allocation == owners.a.allocation
            && out.owners.a.storage == owners.a.storage
            && out.owners.a.allocation_generation == owners.a.allocation_generation
            && out.owners.b.allocation == owners.b.allocation
            && out.owners.b.storage == owners.b.storage
            && out.owners.b.allocation_generation == owners.b.allocation_generation
            && out.owners.c.allocation == owners.c.allocation
            && out.owners.c.storage == owners.c.storage
            && out.owners.c.allocation_generation == owners.c.allocation_generation
    },
{}

// Obligation 34: closing-currentness loss after publication quarantines all
// three owners at the exact two-packet prefix and never restores them.
pub proof fn closing_currentness_loss_quarantines_all_v1(
    plan: PlanV1, owners: OwnersV1, completion: CompletionV1,
)
    ensures {
        let published = publish_v1(plan, owners, true, PublicationScriptV1::Complete);
        let out = complete_v1(published, false, completion);
        out.phase == PhaseV1::Quarantined && out.owners == owners
            && out.reason == ReasonV1::ClosingCurrentnessRejected
            && out.published_packet_prefix == 2
    },
{}

} // verus!

fn main() {}
