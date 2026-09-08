use vstd::prelude::*;

verus! {

pub open spec fn max_epochs_per_lane_v1() -> nat { 64 }
pub open spec fn max_explicit_success_dependencies_v1() -> nat { 32 }

#[derive(PartialEq, Eq)]
pub enum ExecutionClassV1 {
    OrdinaryFixedDispatch,
    PersistentN1,
    ThreeBindingPersistentN3,
}

#[derive(PartialEq, Eq)]
pub enum PhaseV1 {
    Queued,
    Prepared,
    Published,
    Completed,
    PhysicallyRetired,
    HostCommitted,
    Quarantined,
}

#[derive(PartialEq, Eq)]
pub enum TerminalStatusV1 {
    Succeeded,
    Failed,
}

#[derive(PartialEq, Eq)]
pub enum PublicationOrderingWitnessV1 {
    NoPredecessor,
    CompletionObserved(EpochIdentityV1),
    WaitForPrior(EpochIdentityV1),
}

#[derive(PartialEq, Eq)]
pub struct LaneIdentityV1 {
    pub device_id: nat,
    pub device_generation: nat,
    pub lane_id: nat,
    pub lane_generation: nat,
}

#[derive(PartialEq, Eq)]
pub struct EpochIdentityV1 {
    pub lane: LaneIdentityV1,
    pub slot: nat,
    pub slot_generation: nat,
    pub logical_epoch: nat,
    pub submission_id: nat,
}

#[derive(PartialEq, Eq)]
pub struct RecipeStorageFingerprintV1 {
    pub kernel_id: nat,
    pub module_identity: nat,
    pub dispatch_shape_identity: nat,
    pub storage_identity: nat,
    pub bindings_identity: nat,
}

pub struct EntryV1 {
    pub identity: EpochIdentityV1,
    pub class: ExecutionClassV1,
    pub ordered_predecessor: Option<EpochIdentityV1>,
    pub publication_ordering_witness: Option<PublicationOrderingWitnessV1>,
    pub explicit_success_dependencies: Set<EpochIdentityV1>,
    pub explicit_dependency_count: nat,
    pub recipe_storage: RecipeStorageFingerprintV1,
    pub phase: PhaseV1,
    pub terminal_status: Option<TerminalStatusV1>,
    pub owns_custody: bool,
    pub pending_ordered_predecessor_retain: bool,
    pub deferred_ordered_predecessor_retain: bool,
    pub native_effect_count: nat,
    pub committed_effect_count: nat,
    pub profile_visible: bool,
}

pub struct LaneV1 {
    pub identity: LaneIdentityV1,
    pub entries: Seq<EntryV1>,
    pub current: bool,
    pub quarantined: bool,
}

pub open spec fn valid_lane_identity_v1(identity: LaneIdentityV1) -> bool {
    &&& identity.device_id > 0
    &&& identity.device_generation > 0
    &&& identity.lane_id > 0
    &&& identity.lane_generation > 0
}

pub open spec fn valid_epoch_identity_v1(identity: EpochIdentityV1) -> bool {
    &&& valid_lane_identity_v1(identity.lane)
    &&& identity.slot < max_epochs_per_lane_v1()
    &&& identity.slot_generation > 0
    &&& identity.logical_epoch > 0
    &&& identity.submission_id > 0
}

pub open spec fn valid_recipe_storage_v1(recipe: RecipeStorageFingerprintV1) -> bool {
    &&& recipe.kernel_id > 0
    &&& recipe.module_identity > 0
    &&& recipe.dispatch_shape_identity > 0
    &&& recipe.storage_identity > 0
    &&& recipe.bindings_identity > 0
}

pub open spec fn execution_class_admitted_v1(class: ExecutionClassV1) -> bool {
    class == ExecutionClassV1::OrdinaryFixedDispatch
}

pub open spec fn internally_completed_v1(entry: EntryV1) -> bool {
    entry.phase == PhaseV1::Completed
        || entry.phase == PhaseV1::PhysicallyRetired
        || entry.phase == PhaseV1::HostCommitted
}

pub open spec fn internally_succeeded_v1(entry: EntryV1) -> bool {
    internally_completed_v1(entry)
        && entry.terminal_status == Some(TerminalStatusV1::Succeeded)
}

pub open spec fn ordered_predecessor_ready_v1(predecessor: EntryV1) -> bool {
    internally_completed_v1(predecessor)
}

pub open spec fn physical_chain_capable_v1(
    predecessor: EntryV1,
    successor: EntryV1,
) -> bool {
    &&& predecessor.identity == successor.ordered_predecessor.unwrap()
    &&& predecessor.identity.lane == successor.identity.lane
    &&& predecessor.identity.logical_epoch < successor.identity.logical_epoch
    &&& predecessor.class == ExecutionClassV1::OrdinaryFixedDispatch
    &&& predecessor.recipe_storage == successor.recipe_storage
    &&& predecessor.owns_custody
    &&& (predecessor.phase == PhaseV1::Published
        || predecessor.phase == PhaseV1::Completed
        || predecessor.phase == PhaseV1::PhysicallyRetired)
}

pub open spec fn ordering_witness_binds_v1(
    predecessor: Option<EpochIdentityV1>,
    witness: PublicationOrderingWitnessV1,
) -> bool {
    match (predecessor, witness) {
        (None, PublicationOrderingWitnessV1::NoPredecessor) => true,
        (Some(expected), PublicationOrderingWitnessV1::CompletionObserved(observed))
        | (Some(expected), PublicationOrderingWitnessV1::WaitForPrior(observed)) => {
            expected == observed
        },
        _ => false,
    }
}

pub open spec fn ordering_authority_v1(
    successor: EntryV1,
    predecessor: Option<EntryV1>,
    witness: PublicationOrderingWitnessV1,
) -> bool {
    match (successor.ordered_predecessor, predecessor, witness) {
        (None, None, PublicationOrderingWitnessV1::NoPredecessor) => true,
        (
            Some(expected),
            Some(anchor),
            PublicationOrderingWitnessV1::CompletionObserved(observed),
        ) => {
            &&& observed == expected
            &&& anchor.identity == expected
            &&& anchor.phase == PhaseV1::HostCommitted
        },
        (
            Some(expected),
            Some(anchor),
            PublicationOrderingWitnessV1::WaitForPrior(observed),
        ) => {
            &&& observed == expected
            &&& anchor.identity == expected
            &&& physical_chain_capable_v1(anchor, successor)
        },
        _ => false,
    }
}

pub open spec fn wait_for_prior_orders_v1(
    predecessor: EntryV1,
    successor: EntryV1,
) -> bool {
    &&& successor.ordered_predecessor == Some(predecessor.identity)
    &&& successor.publication_ordering_witness
        == Some(PublicationOrderingWitnessV1::WaitForPrior(predecessor.identity))
    &&& predecessor.identity.lane == successor.identity.lane
    &&& predecessor.identity.logical_epoch < successor.identity.logical_epoch
}

pub open spec fn explicit_success_dependency_ready_v1(dependency: EntryV1) -> bool {
    internally_succeeded_v1(dependency)
}

pub open spec fn exact_explicit_success_dependencies_v1(
    entry: EntryV1,
    dependencies: Seq<EntryV1>,
) -> bool {
    &&& dependencies.len() == entry.explicit_dependency_count
    &&& forall|index: int| 0 <= index < dependencies.len() ==> {
        &&& entry.explicit_success_dependencies.contains(dependencies[index].identity)
        &&& explicit_success_dependency_ready_v1(dependencies[index])
    }
    &&& forall|identity: EpochIdentityV1|
        entry.explicit_success_dependencies.contains(identity)
            ==> exists|index: int| 0 <= index < dependencies.len()
                && dependencies[index].identity == identity
}

pub open spec fn valid_entry_v1(entry: EntryV1) -> bool {
    &&& valid_epoch_identity_v1(entry.identity)
    &&& valid_recipe_storage_v1(entry.recipe_storage)
    &&& execution_class_admitted_v1(entry.class)
    &&& entry.explicit_dependency_count == entry.explicit_success_dependencies.len()
    &&& entry.explicit_dependency_count <= max_explicit_success_dependencies_v1()
    &&& !entry.explicit_success_dependencies.contains(entry.identity)
    &&& ((entry.pending_ordered_predecessor_retain
        || entry.deferred_ordered_predecessor_retain)
        ==> entry.ordered_predecessor.is_some())
    &&& !(entry.pending_ordered_predecessor_retain
        && entry.deferred_ordered_predecessor_retain)
    &&& match entry.phase {
        PhaseV1::Queued | PhaseV1::Prepared => {
            &&& entry.owns_custody
            &&& entry.terminal_status.is_none()
            &&& entry.native_effect_count == 0
            &&& entry.committed_effect_count == 0
            &&& !entry.profile_visible
            &&& entry.publication_ordering_witness.is_none()
            &&& entry.pending_ordered_predecessor_retain
                == entry.ordered_predecessor.is_some()
            &&& !entry.deferred_ordered_predecessor_retain
        },
        PhaseV1::Published => {
            &&& entry.owns_custody
            &&& entry.terminal_status.is_none()
            &&& entry.native_effect_count == 1
            &&& entry.committed_effect_count == 0
            &&& !entry.profile_visible
            &&& entry.publication_ordering_witness.is_some()
            &&& ordering_witness_binds_v1(
                entry.ordered_predecessor,
                entry.publication_ordering_witness.unwrap(),
            )
            &&& !entry.pending_ordered_predecessor_retain
            &&& entry.deferred_ordered_predecessor_retain
                == match entry.publication_ordering_witness.unwrap() {
                    PublicationOrderingWitnessV1::WaitForPrior(_) => true,
                    _ => false,
                }
        },
        PhaseV1::Completed | PhaseV1::PhysicallyRetired => {
            &&& entry.owns_custody
            &&& entry.terminal_status.is_some()
            &&& entry.native_effect_count == 1
            &&& entry.committed_effect_count == 0
            &&& !entry.profile_visible
            &&& entry.publication_ordering_witness.is_some()
            &&& ordering_witness_binds_v1(
                entry.ordered_predecessor,
                entry.publication_ordering_witness.unwrap(),
            )
            &&& !entry.pending_ordered_predecessor_retain
            &&& entry.deferred_ordered_predecessor_retain
                == match entry.publication_ordering_witness.unwrap() {
                    PublicationOrderingWitnessV1::WaitForPrior(_) => true,
                    _ => false,
                }
        },
        PhaseV1::HostCommitted => {
            &&& !entry.owns_custody
            &&& entry.terminal_status.is_some()
            &&& entry.native_effect_count == 1
            &&& entry.committed_effect_count == 1
            &&& entry.profile_visible
            &&& entry.publication_ordering_witness.is_some()
            &&& ordering_witness_binds_v1(
                entry.ordered_predecessor,
                entry.publication_ordering_witness.unwrap(),
            )
            &&& !entry.pending_ordered_predecessor_retain
            &&& !entry.deferred_ordered_predecessor_retain
        },
        PhaseV1::Quarantined => {
            &&& entry.owns_custody
            &&& entry.committed_effect_count == 0
            &&& !entry.profile_visible
        },
    }
}

pub open spec fn same_slot_incarnation_v1(
    first: EpochIdentityV1,
    second: EpochIdentityV1,
) -> bool {
    first.lane == second.lane
        && first.slot == second.slot
        && first.slot_generation == second.slot_generation
}

pub open spec fn valid_lane_v1(lane: LaneV1) -> bool {
    &&& valid_lane_identity_v1(lane.identity)
    &&& (lane.quarantined ==> !lane.current)
    &&& lane.entries.len() <= max_epochs_per_lane_v1()
    &&& forall|index: int| 0 <= index < lane.entries.len() ==> {
        &&& valid_entry_v1(lane.entries[index])
        &&& lane.entries[index].identity.lane == lane.identity
        &&& lane.entries[index].phase != PhaseV1::HostCommitted
        &&& lane.quarantined ==> lane.entries[index].phase == PhaseV1::Quarantined
    }
    &&& forall|first: int, second: int|
        0 <= first < second < lane.entries.len() ==> {
            &&& lane.entries[first].identity != lane.entries[second].identity
            &&& lane.entries[first].identity.slot != lane.entries[second].identity.slot
            &&& lane.entries[first].identity.submission_id
                != lane.entries[second].identity.submission_id
            &&& lane.entries[first].identity.logical_epoch
                < lane.entries[second].identity.logical_epoch
        }
}

pub open spec fn exact_witness_v1(
    entry: EntryV1,
    identity: EpochIdentityV1,
    recipe_storage: RecipeStorageFingerprintV1,
) -> bool {
    entry.identity == identity && entry.recipe_storage == recipe_storage
}

pub open spec fn prepare_v1(
    entry: EntryV1,
    identity: EpochIdentityV1,
    recipe_storage: RecipeStorageFingerprintV1,
) -> EntryV1 {
    if entry.phase == PhaseV1::Queued
        && exact_witness_v1(entry, identity, recipe_storage) {
        EntryV1 { phase: PhaseV1::Prepared, ..entry }
    } else {
        entry
    }
}

pub open spec fn publication_allowed_v1(
    entry: EntryV1,
    predecessor: Option<EntryV1>,
    ordering_witness: PublicationOrderingWitnessV1,
    explicit_dependencies: Seq<EntryV1>,
) -> bool {
    &&& entry.phase == PhaseV1::Prepared
    &&& execution_class_admitted_v1(entry.class)
    &&& ordering_authority_v1(entry, predecessor, ordering_witness)
    &&& exact_explicit_success_dependencies_v1(entry, explicit_dependencies)
}

pub open spec fn publish_v1(
    entry: EntryV1,
    identity: EpochIdentityV1,
    recipe_storage: RecipeStorageFingerprintV1,
    predecessor: Option<EntryV1>,
    ordering_witness: PublicationOrderingWitnessV1,
    explicit_dependencies: Seq<EntryV1>,
) -> EntryV1 {
    if exact_witness_v1(entry, identity, recipe_storage)
        && publication_allowed_v1(
            entry,
            predecessor,
            ordering_witness,
            explicit_dependencies,
        ) {
        EntryV1 {
            phase: PhaseV1::Published,
            publication_ordering_witness: Some(ordering_witness),
            pending_ordered_predecessor_retain: false,
            deferred_ordered_predecessor_retain:
                match ordering_witness {
                    PublicationOrderingWitnessV1::WaitForPrior(_) => true,
                    _ => false,
                },
            native_effect_count: 1,
            ..entry
        }
    } else {
        entry
    }
}

pub open spec fn retry_publication_v1(entry: EntryV1) -> EntryV1 { entry }

pub open spec fn observe_completion_v1(
    entry: EntryV1,
    identity: EpochIdentityV1,
    recipe_storage: RecipeStorageFingerprintV1,
    status: TerminalStatusV1,
) -> EntryV1 {
    if entry.phase == PhaseV1::Published
        && exact_witness_v1(entry, identity, recipe_storage) {
        EntryV1 {
            phase: PhaseV1::Completed,
            terminal_status: Some(status),
            ..entry
        }
    } else {
        entry
    }
}

pub open spec fn retire_physical_v1(
    entry: EntryV1,
    identity: EpochIdentityV1,
    recipe_storage: RecipeStorageFingerprintV1,
) -> EntryV1 {
    if entry.phase == PhaseV1::Completed
        && exact_witness_v1(entry, identity, recipe_storage) {
        EntryV1 { phase: PhaseV1::PhysicallyRetired, ..entry }
    } else {
        entry
    }
}

pub open spec fn restore_custody_or_quarantine_v1(
    entry: EntryV1,
    restored_identity: EpochIdentityV1,
    restored_recipe_storage: RecipeStorageFingerprintV1,
) -> EntryV1 {
    if exact_witness_v1(entry, restored_identity, restored_recipe_storage) {
        entry
    } else {
        EntryV1 {
            phase: PhaseV1::Quarantined,
            owns_custody: true,
            committed_effect_count: 0,
            profile_visible: false,
            ..entry
        }
    }
}

pub open spec fn quarantine_entry_v1(entry: EntryV1) -> EntryV1 {
    EntryV1 {
        phase: PhaseV1::Quarantined,
        owns_custody: true,
        committed_effect_count: 0,
        profile_visible: false,
        ..entry
    }
}

pub open spec fn quarantine_lane_v1(lane: LaneV1) -> LaneV1 {
    LaneV1 {
        entries: Seq::new(lane.entries.len(), |index: int| {
            quarantine_entry_v1(lane.entries[index])
        }),
        current: false,
        quarantined: true,
        ..lane
    }
}

pub open spec fn observe_completion_or_quarantine_lane_v1(
    lane: LaneV1,
    index: int,
    observed_identity: EpochIdentityV1,
    observed_recipe_storage: RecipeStorageFingerprintV1,
    status: TerminalStatusV1,
) -> LaneV1 {
    if 0 <= index < lane.entries.len()
        && lane.entries[index].phase == PhaseV1::Published
        && exact_witness_v1(
            lane.entries[index],
            observed_identity,
            observed_recipe_storage,
        ) {
        LaneV1 {
            entries: lane.entries.update(
                index,
                observe_completion_v1(
                    lane.entries[index],
                    observed_identity,
                    observed_recipe_storage,
                    status,
                ),
            ),
            ..lane
        }
    } else {
        quarantine_lane_v1(lane)
    }
}

pub open spec fn host_commit_v1(entry: EntryV1, is_frontier: bool) -> EntryV1 {
    if entry.phase == PhaseV1::PhysicallyRetired && is_frontier {
        EntryV1 {
            phase: PhaseV1::HostCommitted,
            owns_custody: false,
            pending_ordered_predecessor_retain: false,
            deferred_ordered_predecessor_retain: false,
            committed_effect_count: 1,
            profile_visible: true,
            ..entry
        }
    } else {
        entry
    }
}

pub open spec fn commit_prefix_v1(entries: Seq<EntryV1>, prefix_len: nat) -> Seq<EntryV1> {
    Seq::new(entries.len(), |index: int| {
        if 0 <= index < prefix_len {
            host_commit_v1(entries[index], true)
        } else {
            entries[index]
        }
    })
}

pub open spec fn is_physically_retired_prefix_v1(
    entries: Seq<EntryV1>,
    prefix_len: nat,
) -> bool {
    &&& prefix_len <= entries.len()
    &&& forall|index: int| 0 <= index < prefix_len
        ==> entries[index].phase == PhaseV1::PhysicallyRetired
}

pub open spec fn cancellation_admitted_v1(entry: EntryV1, is_tail: bool) -> bool {
    is_tail && (entry.phase == PhaseV1::Queued || entry.phase == PhaseV1::Prepared)
}

pub open spec fn host_status_visible_v1(entry: EntryV1) -> bool {
    entry.phase == PhaseV1::HostCommitted
        && entry.terminal_status.is_some()
        && entry.profile_visible
        && entry.committed_effect_count == 1
        && !entry.owns_custody
}

pub proof fn ordinary_is_the_only_admitted_class_v1(class: ExecutionClassV1)
    ensures execution_class_admitted_v1(class)
        <==> class == ExecutionClassV1::OrdinaryFixedDispatch,
{
}

pub proof fn persistent_n1_is_excluded_v1()
    ensures !execution_class_admitted_v1(ExecutionClassV1::PersistentN1),
{
}

pub proof fn three_binding_n3_is_excluded_v1()
    ensures !execution_class_admitted_v1(ExecutionClassV1::ThreeBindingPersistentN3),
{
}

pub proof fn fixed_lane_roster_is_bounded_v1(lane: LaneV1)
    requires valid_lane_v1(lane),
    ensures lane.entries.len() <= 64,
{
}

pub proof fn live_slots_are_unique_v1(lane: LaneV1, first: int, second: int)
    requires
        valid_lane_v1(lane),
        0 <= first < second < lane.entries.len(),
    ensures lane.entries[first].identity.slot != lane.entries[second].identity.slot,
{
}

pub proof fn fresh_slot_generation_prevents_aba_v1(
    old: EpochIdentityV1,
    fresh: EpochIdentityV1,
)
    requires
        old.lane == fresh.lane,
        old.slot == fresh.slot,
        fresh.slot_generation > old.slot_generation,
    ensures
        fresh != old,
        !same_slot_incarnation_v1(old, fresh),
{
}

pub proof fn logical_epochs_are_strictly_ordered_v1(lane: LaneV1, first: int, second: int)
    requires
        valid_lane_v1(lane),
        0 <= first < second < lane.entries.len(),
    ensures lane.entries[first].identity.logical_epoch
        < lane.entries[second].identity.logical_epoch,
{
}

pub proof fn exact_prepare_preserves_recipe_and_custody_v1(entry: EntryV1)
    requires valid_entry_v1(entry), entry.phase == PhaseV1::Queued,
    ensures {
        let prepared = prepare_v1(entry, entry.identity, entry.recipe_storage);
        &&& prepared.phase == PhaseV1::Prepared
        &&& prepared.identity == entry.identity
        &&& prepared.recipe_storage == entry.recipe_storage
        &&& prepared.owns_custody
        &&& valid_entry_v1(prepared)
    },
{
}

pub proof fn substituted_prepare_is_no_effect_v1(
    entry: EntryV1,
    substituted: RecipeStorageFingerprintV1,
)
    requires substituted != entry.recipe_storage,
    ensures prepare_v1(entry, entry.identity, substituted) == entry,
{
}

pub proof fn ordered_failure_still_satisfies_completion_order_v1(entry: EntryV1)
    requires
        entry.phase == PhaseV1::Completed,
        entry.terminal_status == Some(TerminalStatusV1::Failed),
    ensures ordered_predecessor_ready_v1(entry),
{
}

pub proof fn failed_explicit_dependency_does_not_satisfy_success_v1(entry: EntryV1)
    requires
        entry.phase == PhaseV1::Completed,
        entry.terminal_status == Some(TerminalStatusV1::Failed),
    ensures !explicit_success_dependency_ready_v1(entry),
{
}

pub proof fn ordered_and_explicit_dependency_polarity_are_distinct_v1(entry: EntryV1)
    requires
        entry.phase == PhaseV1::Completed,
        entry.terminal_status == Some(TerminalStatusV1::Failed),
    ensures
        ordered_predecessor_ready_v1(entry),
        !explicit_success_dependency_ready_v1(entry),
{
}

pub proof fn missing_wait_for_prior_blocks_early_publication_v1(
    entry: EntryV1,
    predecessor: EntryV1,
)
    requires
        valid_entry_v1(entry),
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor == Some(predecessor.identity),
    ensures publish_v1(
        entry,
        entry.identity,
        entry.recipe_storage,
        Some(predecessor),
        PublicationOrderingWitnessV1::NoPredecessor,
        Seq::empty(),
    ) == entry,
{
}

pub proof fn foreign_wait_for_prior_blocks_publication_v1(
    entry: EntryV1,
    predecessor: EntryV1,
    foreign: EpochIdentityV1,
)
    requires
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor == Some(predecessor.identity),
        foreign != predecessor.identity,
    ensures publish_v1(
        entry,
        entry.identity,
        entry.recipe_storage,
        Some(predecessor),
        PublicationOrderingWitnessV1::WaitForPrior(foreign),
        Seq::empty(),
    ) == entry,
{
}

pub proof fn mismatched_recipe_storage_blocks_physical_chain_v1(
    entry: EntryV1,
    predecessor: EntryV1,
)
    requires
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor == Some(predecessor.identity),
        predecessor.recipe_storage != entry.recipe_storage,
    ensures publish_v1(
        entry,
        entry.identity,
        entry.recipe_storage,
        Some(predecessor),
        PublicationOrderingWitnessV1::WaitForPrior(predecessor.identity),
        Seq::empty(),
    ) == entry,
{
}

pub proof fn published_predecessor_allows_early_chain_v1(
    entry: EntryV1,
    predecessor: EntryV1,
)
    requires
        valid_entry_v1(entry),
        valid_entry_v1(predecessor),
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor == Some(predecessor.identity),
        predecessor.phase == PhaseV1::Published,
        predecessor.identity.lane == entry.identity.lane,
        predecessor.identity.logical_epoch < entry.identity.logical_epoch,
        predecessor.recipe_storage == entry.recipe_storage,
        entry.explicit_dependency_count == 0,
    ensures {
        let published = publish_v1(
            entry,
            entry.identity,
            entry.recipe_storage,
            Some(predecessor),
            PublicationOrderingWitnessV1::WaitForPrior(predecessor.identity),
            Seq::empty(),
        );
        &&& published.phase == PhaseV1::Published
        &&& wait_for_prior_orders_v1(predecessor, published)
        &&& published.deferred_ordered_predecessor_retain
        &&& !published.pending_ordered_predecessor_retain
        &&& valid_entry_v1(published)
    },
{
}

pub proof fn completed_predecessor_allows_wait_for_prior_chain_v1(
    entry: EntryV1,
    predecessor: EntryV1,
)
    requires
        valid_entry_v1(entry),
        valid_entry_v1(predecessor),
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor == Some(predecessor.identity),
        predecessor.phase == PhaseV1::Completed,
        predecessor.identity.lane == entry.identity.lane,
        predecessor.identity.logical_epoch < entry.identity.logical_epoch,
        predecessor.recipe_storage == entry.recipe_storage,
        entry.explicit_dependency_count == 0,
    ensures publication_allowed_v1(
        entry,
        Some(predecessor),
        PublicationOrderingWitnessV1::WaitForPrior(predecessor.identity),
        Seq::empty(),
    ),
{
}

pub proof fn physically_retired_predecessor_allows_wait_for_prior_chain_v1(
    entry: EntryV1,
    predecessor: EntryV1,
)
    requires
        valid_entry_v1(entry),
        valid_entry_v1(predecessor),
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor == Some(predecessor.identity),
        predecessor.phase == PhaseV1::PhysicallyRetired,
        predecessor.identity.lane == entry.identity.lane,
        predecessor.identity.logical_epoch < entry.identity.logical_epoch,
        predecessor.recipe_storage == entry.recipe_storage,
        entry.explicit_dependency_count == 0,
    ensures publication_allowed_v1(
        entry,
        Some(predecessor),
        PublicationOrderingWitnessV1::WaitForPrior(predecessor.identity),
        Seq::empty(),
    ),
{
}

pub proof fn host_committed_is_completion_authority_not_chain_anchor_v1(
    entry: EntryV1,
    predecessor: EntryV1,
)
    requires
        valid_entry_v1(entry),
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor == Some(predecessor.identity),
        predecessor.phase == PhaseV1::HostCommitted,
        entry.explicit_dependency_count == 0,
    ensures
        publication_allowed_v1(
            entry,
            Some(predecessor),
            PublicationOrderingWitnessV1::CompletionObserved(predecessor.identity),
            Seq::empty(),
        ),
        !publication_allowed_v1(
            entry,
            Some(predecessor),
            PublicationOrderingWitnessV1::WaitForPrior(predecessor.identity),
            Seq::empty(),
        ),
{
}

pub proof fn failed_explicit_dependency_blocks_publication_v1(
    entry: EntryV1,
    dependency: EntryV1,
)
    requires
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor.is_none(),
        entry.explicit_success_dependencies.contains(dependency.identity),
        dependency.phase == PhaseV1::Completed,
        dependency.terminal_status == Some(TerminalStatusV1::Failed),
    ensures publish_v1(
        entry,
        entry.identity,
        entry.recipe_storage,
        None,
        PublicationOrderingWitnessV1::NoPredecessor,
        seq![dependency],
    ) == entry,
{
}

pub proof fn exact_publication_preserves_recipe_and_takes_one_effect_v1(entry: EntryV1)
    requires
        valid_entry_v1(entry),
        entry.phase == PhaseV1::Prepared,
        entry.ordered_predecessor.is_none(),
        entry.explicit_dependency_count == 0,
    ensures {
        let published = publish_v1(
            entry,
            entry.identity,
            entry.recipe_storage,
            None,
            PublicationOrderingWitnessV1::NoPredecessor,
            Seq::empty(),
        );
        &&& published.phase == PhaseV1::Published
        &&& published.identity == entry.identity
        &&& published.recipe_storage == entry.recipe_storage
        &&& published.native_effect_count == 1
        &&& published.owns_custody
        &&& valid_entry_v1(published)
    },
{
}

pub proof fn substituted_publication_is_no_effect_v1(
    entry: EntryV1,
    substituted: RecipeStorageFingerprintV1,
)
    requires substituted != entry.recipe_storage,
    ensures publish_v1(
        entry,
        entry.identity,
        substituted,
        None,
        PublicationOrderingWitnessV1::NoPredecessor,
        Seq::empty(),
    ) == entry,
{
}

pub proof fn retryable_publication_is_exactly_no_effect_v1(entry: EntryV1)
    ensures retry_publication_v1(entry) == entry,
{
}

pub proof fn retryable_publication_retains_deferred_predecessor_v1(entry: EntryV1)
    requires entry.deferred_ordered_predecessor_retain,
    ensures retry_publication_v1(entry).deferred_ordered_predecessor_retain,
{
}

pub proof fn exact_completion_preserves_recipe_and_hides_host_state_v1(
    entry: EntryV1,
    status: TerminalStatusV1,
)
    requires valid_entry_v1(entry), entry.phase == PhaseV1::Published,
    ensures {
        let completed = observe_completion_v1(
            entry,
            entry.identity,
            entry.recipe_storage,
            status,
        );
        &&& completed.phase == PhaseV1::Completed
        &&& completed.terminal_status == Some(status)
        &&& completed.recipe_storage == entry.recipe_storage
        &&& completed.owns_custody
        &&& !host_status_visible_v1(completed)
        &&& valid_entry_v1(completed)
    },
{
}

pub proof fn foreign_completion_quarantines_whole_lane_v1(
    lane: LaneV1,
    index: int,
    foreign: EpochIdentityV1,
    status: TerminalStatusV1,
)
    requires
        valid_lane_v1(lane),
        0 <= index < lane.entries.len(),
        lane.entries[index].phase == PhaseV1::Published,
        foreign != lane.entries[index].identity,
    ensures {
        let quarantined = observe_completion_or_quarantine_lane_v1(
            lane,
            index,
            foreign,
            lane.entries[index].recipe_storage,
            status,
        );
        &&& quarantined.quarantined
        &&& !quarantined.current
        &&& quarantined.entries.len() == lane.entries.len()
        &&& forall|live_index: int| 0 <= live_index < quarantined.entries.len()
            ==> #[trigger] quarantined.entries[live_index].phase == PhaseV1::Quarantined
                && quarantined.entries[live_index].owns_custody
    },
{
}

pub proof fn exact_physical_retirement_preserves_recipe_and_hides_host_state_v1(entry: EntryV1)
    requires valid_entry_v1(entry), entry.phase == PhaseV1::Completed,
    ensures {
        let retired = retire_physical_v1(entry, entry.identity, entry.recipe_storage);
        &&& retired.phase == PhaseV1::PhysicallyRetired
        &&& retired.terminal_status == entry.terminal_status
        &&& retired.recipe_storage == entry.recipe_storage
        &&& retired.owns_custody
        &&& !host_status_visible_v1(retired)
        &&& valid_entry_v1(retired)
    },
{
}

pub proof fn precommit_phases_hide_status_effect_profile_and_custody_release_v1(entry: EntryV1)
    requires
        valid_entry_v1(entry),
        entry.phase != PhaseV1::HostCommitted,
    ensures !host_status_visible_v1(entry),
{
}

pub proof fn exact_custody_restore_is_no_effect_v1(entry: EntryV1)
    ensures restore_custody_or_quarantine_v1(
        entry,
        entry.identity,
        entry.recipe_storage,
    ) == entry,
{
}

pub proof fn substituted_custody_restore_quarantines_v1(
    entry: EntryV1,
    substituted: RecipeStorageFingerprintV1,
)
    requires substituted != entry.recipe_storage,
    ensures {
        let quarantined = restore_custody_or_quarantine_v1(
            entry,
            entry.identity,
            substituted,
        );
        &&& quarantined.phase == PhaseV1::Quarantined
        &&& quarantined.owns_custody
        &&& quarantined.recipe_storage == entry.recipe_storage
        &&& !host_status_visible_v1(quarantined)
    },
{
}

pub proof fn lane_quarantine_is_complete_and_custody_preserving_v1(lane: LaneV1, index: int)
    requires
        valid_lane_v1(lane),
        0 <= index < lane.entries.len(),
    ensures {
        let quarantined = quarantine_lane_v1(lane);
        &&& quarantined.quarantined
        &&& !quarantined.current
        &&& quarantined.entries.len() == lane.entries.len()
        &&& quarantined.entries[index].phase == PhaseV1::Quarantined
        &&& quarantined.entries[index].owns_custody
        &&& quarantined.entries[index].identity == lane.entries[index].identity
        &&& quarantined.entries[index].recipe_storage == lane.entries[index].recipe_storage
    },
{
}

pub proof fn valid_quarantined_lane_is_not_current_v1(lane: LaneV1)
    requires valid_lane_v1(lane), lane.quarantined,
    ensures !lane.current,
{
}

pub proof fn quarantine_is_host_invisible_v1(entry: EntryV1)
    ensures !host_status_visible_v1(quarantine_entry_v1(entry)),
{
}

pub proof fn nonfrontier_physical_retirement_cannot_commit_v1(entry: EntryV1)
    requires entry.phase == PhaseV1::PhysicallyRetired,
    ensures host_commit_v1(entry, false) == entry,
{
}

pub proof fn frontier_commit_is_exact_and_host_visible_v1(entry: EntryV1)
    requires valid_entry_v1(entry), entry.phase == PhaseV1::PhysicallyRetired,
    ensures {
        let committed = host_commit_v1(entry, true);
        &&& committed.phase == PhaseV1::HostCommitted
        &&& committed.identity == entry.identity
        &&& committed.recipe_storage == entry.recipe_storage
        &&& committed.terminal_status == entry.terminal_status
        &&& !committed.owns_custody
        &&& !committed.deferred_ordered_predecessor_retain
        &&& committed.committed_effect_count == 1
        &&& committed.profile_visible
        &&& host_status_visible_v1(committed)
        &&& valid_entry_v1(committed)
    },
{
}

pub proof fn host_commit_is_exactly_once_v1(entry: EntryV1)
    requires valid_entry_v1(entry), entry.phase == PhaseV1::PhysicallyRetired,
    ensures host_commit_v1(host_commit_v1(entry, true), true)
        == host_commit_v1(entry, true),
{
}

pub proof fn deferred_predecessor_retain_survives_until_commit_v1(entry: EntryV1)
    requires
        valid_entry_v1(entry),
        entry.deferred_ordered_predecessor_retain,
        entry.phase == PhaseV1::PhysicallyRetired,
    ensures
        entry.deferred_ordered_predecessor_retain,
        !host_commit_v1(entry, true).deferred_ordered_predecessor_retain,
{
}

pub proof fn commit_prefix_retains_length_v1(entries: Seq<EntryV1>, prefix_len: nat)
    ensures commit_prefix_v1(entries, prefix_len).len() == entries.len(),
{
}

pub proof fn retired_prefix_commits_exactly_v1(
    entries: Seq<EntryV1>,
    prefix_len: nat,
    index: int,
)
    requires
        is_physically_retired_prefix_v1(entries, prefix_len),
        0 <= index < prefix_len,
        valid_entry_v1(entries[index]),
    ensures
        commit_prefix_v1(entries, prefix_len)[index].phase == PhaseV1::HostCommitted,
        host_status_visible_v1(commit_prefix_v1(entries, prefix_len)[index]),
{
}

pub proof fn out_of_order_retired_suffix_is_unchanged_v1(
    entries: Seq<EntryV1>,
    later: int,
)
    requires
        0 < later < entries.len(),
        entries[later].phase == PhaseV1::PhysicallyRetired,
        entries[0].phase != PhaseV1::PhysicallyRetired,
    ensures
        commit_prefix_v1(entries, 0)[later] == entries[later],
        !host_status_visible_v1(commit_prefix_v1(entries, 0)[later]),
{
}

pub proof fn suffix_outside_commit_prefix_is_unchanged_v1(
    entries: Seq<EntryV1>,
    prefix_len: nat,
    index: int,
)
    requires
        prefix_len <= index < entries.len(),
    ensures commit_prefix_v1(entries, prefix_len)[index] == entries[index],
{
}

pub proof fn tail_unpublished_cancellation_is_admitted_v1(entry: EntryV1)
    requires entry.phase == PhaseV1::Queued || entry.phase == PhaseV1::Prepared,
    ensures cancellation_admitted_v1(entry, true),
{
}

pub proof fn non_tail_cancellation_is_rejected_v1(entry: EntryV1)
    ensures !cancellation_admitted_v1(entry, false),
{
}

pub proof fn published_cancellation_is_too_late_v1(entry: EntryV1)
    requires entry.phase == PhaseV1::Published,
    ensures !cancellation_admitted_v1(entry, true),
{
}

pub proof fn completed_cancellation_is_too_late_v1(entry: EntryV1)
    requires entry.phase == PhaseV1::Completed
        || entry.phase == PhaseV1::PhysicallyRetired
        || entry.phase == PhaseV1::HostCommitted
        || entry.phase == PhaseV1::Quarantined,
    ensures !cancellation_admitted_v1(entry, true),
{
}

pub proof fn exact_transition_sequence_preserves_immutable_identity_v1(
    entry: EntryV1,
    status: TerminalStatusV1,
)
    requires
        valid_entry_v1(entry),
        entry.phase == PhaseV1::Queued,
        entry.ordered_predecessor.is_none(),
        entry.explicit_dependency_count == 0,
    ensures {
        let prepared = prepare_v1(entry, entry.identity, entry.recipe_storage);
        let published = publish_v1(
            prepared,
            prepared.identity,
            prepared.recipe_storage,
            None,
            PublicationOrderingWitnessV1::NoPredecessor,
            Seq::empty(),
        );
        let completed = observe_completion_v1(
            published,
            published.identity,
            published.recipe_storage,
            status,
        );
        let retired = retire_physical_v1(
            completed,
            completed.identity,
            completed.recipe_storage,
        );
        let committed = host_commit_v1(retired, true);
        &&& committed.identity == entry.identity
        &&& committed.recipe_storage == entry.recipe_storage
        &&& committed.phase == PhaseV1::HostCommitted
        &&& committed.terminal_status == Some(status)
        &&& host_status_visible_v1(committed)
        &&& valid_entry_v1(committed)
    },
{
}

}
