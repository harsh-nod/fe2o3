//! Executable finite R57 model for one three-binding persistent-device compute
//! transaction.
//!
//! The admitted shape is exactly two initialized, read-only inputs and one
//! write-only output, each naming a distinct full-extent persistent HBM
//! allocation on one device, VM, and queue. Publication contains exactly one
//! `WaitForPrior` control packet followed by exactly one dispatch packet.
//! Initialization promotion additionally requires a caller-supplied abstract
//! full-write certificate from a separately named coverage issuer.
//!
//! This addressless model performs no I/O and grants no KFD, HSA, HIP, packet,
//! signal, dispatch, progress, hardware, refinement, parity, or performance
//! authority. The certificate, identities, and observations are
//! caller-constructed mathematical inputs.

use alloc::vec::Vec;

pub const R57_BINDING_COUNT_V1: usize = 3;
pub const R57_PACKET_COUNT_V1: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57MemoryKindV1 {
    PersistentHbm,
    Host,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57BindingRoleV1 {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57BindingEffectV1 {
    ReadOnly,
    WriteOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57PacketKindV1 {
    WaitForPrior,
    Dispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57WriteCoverageV1 {
    FullExtent,
    Partial,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R57AllocationIdentityV1 {
    pub allocation: u64,
    pub storage: u64,
    pub allocation_generation: u64,
    pub device: u32,
    pub vm: u64,
    pub byte_len: u64,
    pub memory_kind: R57MemoryKindV1,
}

impl R57AllocationIdentityV1 {
    pub const fn valid_model_only(self) -> bool {
        self.allocation != 0
            && self.storage != 0
            && self.allocation_generation != 0
            && self.device != 0
            && self.vm != 0
            && self.byte_len != 0
            && matches!(self.memory_kind, R57MemoryKindV1::PersistentHbm)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R57OwnerSnapshotV1 {
    pub owner_occurrence: u64,
    pub identity: R57AllocationIdentityV1,
    pub initialized: bool,
    pub content_generation: u64,
}

/// Linear model token for one persistent allocation owner.
///
/// ```compile_fail
/// use fe2o3_runtime_model::R57OwnedAllocationV1;
/// fn duplicate(owner: R57OwnedAllocationV1) {
///     let first = owner;
///     let second = owner;
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct R57OwnedAllocationV1 {
    snapshot: R57OwnerSnapshotV1,
}

impl R57OwnedAllocationV1 {
    pub fn new_model_only(
        owner_occurrence: u64,
        identity: R57AllocationIdentityV1,
        initialized: bool,
        content_generation: u64,
    ) -> Option<Self> {
        (owner_occurrence != 0 && identity.valid_model_only() && content_generation != 0).then_some(
            Self {
                snapshot: R57OwnerSnapshotV1 {
                    owner_occurrence,
                    identity,
                    initialized,
                    content_generation,
                },
            },
        )
    }

    pub const fn snapshot_model_only(&self) -> R57OwnerSnapshotV1 {
        self.snapshot
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R57BindingSpecV1 {
    pub ordinal: u8,
    pub allocation: u64,
    pub storage: u64,
    pub allocation_generation: u64,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub role: R57BindingRoleV1,
    pub effect: R57BindingEffectV1,
    pub requires_initialized: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R57PacketIdentityV1 {
    pub packet: u64,
    pub kind: R57PacketKindV1,
    pub order: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R57FullWriteCertificateV1 {
    pub issuer_authority: u64,
    pub binder_authority: u64,
    pub device: u32,
    pub vm: u64,
    pub queue: u64,
    pub queue_generation: u64,
    pub kernel: u64,
    pub dispatch: u64,
    pub transaction_generation: u64,
    pub allocation: u64,
    pub storage: u64,
    pub allocation_generation: u64,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub coverage: R57WriteCoverageV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R57ThreeBindingPlanV1 {
    pub device: u32,
    pub vm: u64,
    pub queue: u64,
    pub queue_generation: u64,
    pub fixed_binder_authority: u64,
    pub full_write_issuer_authority: u64,
    pub kernel: u64,
    pub dispatch: u64,
    pub transaction_generation: u64,
    pub frontier_before: u64,
    pub completion_signal: u64,
    pub wait_packet: R57PacketIdentityV1,
    pub dispatch_packet: R57PacketIdentityV1,
    pub full_write: R57FullWriteCertificateV1,
    pub bindings: [R57BindingSpecV1; R57_BINDING_COUNT_V1],
}

impl R57ThreeBindingPlanV1 {
    pub fn frontier_after_model_only(self) -> Option<u64> {
        self.frontier_before
            .checked_add(u64::from(R57_PACKET_COUNT_V1))
    }

    pub fn valid_model_only(self) -> bool {
        self.device != 0
            && self.vm != 0
            && self.queue != 0
            && self.queue_generation != 0
            && self.fixed_binder_authority != 0
            && self.full_write_issuer_authority != 0
            && self.full_write_issuer_authority != self.fixed_binder_authority
            && self.kernel != 0
            && self.dispatch != 0
            && self.transaction_generation != 0
            && self.completion_signal != 0
            && self.frontier_after_model_only().is_some()
            && self.wait_packet.packet != 0
            && self.wait_packet.kind == R57PacketKindV1::WaitForPrior
            && self.wait_packet.order == 0
            && self.dispatch_packet.packet != 0
            && self.dispatch_packet.packet != self.wait_packet.packet
            && self.dispatch_packet.kind == R57PacketKindV1::Dispatch
            && self.dispatch_packet.order == 1
            && self.full_write.issuer_authority == self.full_write_issuer_authority
            && self.full_write.binder_authority == self.fixed_binder_authority
            && self.full_write.device == self.device
            && self.full_write.vm == self.vm
            && self.full_write.queue == self.queue
            && self.full_write.queue_generation == self.queue_generation
            && self.full_write.kernel == self.kernel
            && self.full_write.dispatch == self.dispatch
            && self.full_write.transaction_generation == self.transaction_generation
            && self.full_write.allocation == self.bindings[2].allocation
            && self.full_write.storage == self.bindings[2].storage
            && self.full_write.allocation_generation == self.bindings[2].allocation_generation
            && self.full_write.byte_offset == 0
            && self.full_write.byte_len == self.bindings[2].byte_len
            && self.full_write.coverage == R57WriteCoverageV1::FullExtent
            && self.bindings[0].ordinal == 0
            && self.bindings[0].role == R57BindingRoleV1::Read
            && self.bindings[0].effect == R57BindingEffectV1::ReadOnly
            && self.bindings[0].requires_initialized
            && self.bindings[1].ordinal == 1
            && self.bindings[1].role == R57BindingRoleV1::Read
            && self.bindings[1].effect == R57BindingEffectV1::ReadOnly
            && self.bindings[1].requires_initialized
            && self.bindings[2].ordinal == 2
            && self.bindings[2].role == R57BindingRoleV1::Write
            && self.bindings[2].effect == R57BindingEffectV1::WriteOnly
            && !self.bindings[2].requires_initialized
            && self.bindings[0].byte_len == self.bindings[1].byte_len
            && self.bindings[0].byte_len == self.bindings[2].byte_len
            && distinct_binding_identities(&self.bindings)
    }
}

fn distinct_binding_identities(bindings: &[R57BindingSpecV1; R57_BINDING_COUNT_V1]) -> bool {
    for left in 0..R57_BINDING_COUNT_V1 {
        for right in (left + 1)..R57_BINDING_COUNT_V1 {
            if bindings[left].allocation == bindings[right].allocation
                || bindings[left].storage == bindings[right].storage
            {
                return false;
            }
        }
    }
    true
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57PreparationErrorV1 {
    BindingCount,
    InvalidPlan,
    DuplicateOwnerOccurrence,
    BindingIdentity,
    CrossDevice,
    CrossVm,
    NonFullExtent,
    InputUninitialized,
    OutputGenerationExhausted,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R57PreparationFailureV1 {
    pub error: R57PreparationErrorV1,
    owners: Vec<R57OwnedAllocationV1>,
}

impl R57PreparationFailureV1 {
    pub fn owner_snapshots_model_only(&self) -> Vec<R57OwnerSnapshotV1> {
        self.owners
            .iter()
            .map(R57OwnedAllocationV1::snapshot_model_only)
            .collect()
    }

    pub fn into_owners_model_only(self) -> Vec<R57OwnedAllocationV1> {
        self.owners
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R57PreparedThreeBindingV1 {
    plan: R57ThreeBindingPlanV1,
    owners: [R57OwnedAllocationV1; R57_BINDING_COUNT_V1],
    opening: [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1],
}

impl R57PreparedThreeBindingV1 {
    pub const fn plan_model_only(&self) -> R57ThreeBindingPlanV1 {
        self.plan
    }

    pub const fn opening_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.opening
    }

    pub fn owner_snapshots_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.owners
            .each_ref()
            .map(|owner| owner.snapshot_model_only())
    }
}

pub fn r57_prepare_three_binding_compute_model_only(
    plan: R57ThreeBindingPlanV1,
    owners: Vec<R57OwnedAllocationV1>,
) -> Result<R57PreparedThreeBindingV1, R57PreparationFailureV1> {
    if owners.len() != R57_BINDING_COUNT_V1 {
        return Err(R57PreparationFailureV1 {
            error: R57PreparationErrorV1::BindingCount,
            owners,
        });
    }
    if !plan.valid_model_only() {
        return Err(R57PreparationFailureV1 {
            error: R57PreparationErrorV1::InvalidPlan,
            owners,
        });
    }

    for left in 0..R57_BINDING_COUNT_V1 {
        for right in (left + 1)..R57_BINDING_COUNT_V1 {
            if owners[left].snapshot_model_only().owner_occurrence
                == owners[right].snapshot_model_only().owner_occurrence
            {
                return Err(R57PreparationFailureV1 {
                    error: R57PreparationErrorV1::DuplicateOwnerOccurrence,
                    owners,
                });
            }
        }
    }

    let validation_error = owners.iter().enumerate().find_map(|(index, owner)| {
        let snapshot = owner.snapshot_model_only();
        let spec = plan.bindings[index];
        if snapshot.identity.allocation != spec.allocation
            || snapshot.identity.storage != spec.storage
            || snapshot.identity.allocation_generation != spec.allocation_generation
        {
            Some(R57PreparationErrorV1::BindingIdentity)
        } else if snapshot.identity.device != plan.device {
            Some(R57PreparationErrorV1::CrossDevice)
        } else if snapshot.identity.vm != plan.vm {
            Some(R57PreparationErrorV1::CrossVm)
        } else if spec.byte_offset != 0 || spec.byte_len != snapshot.identity.byte_len {
            Some(R57PreparationErrorV1::NonFullExtent)
        } else if spec.requires_initialized && !snapshot.initialized {
            Some(R57PreparationErrorV1::InputUninitialized)
        } else if index == 2 && snapshot.content_generation == u64::MAX {
            Some(R57PreparationErrorV1::OutputGenerationExhausted)
        } else {
            None
        }
    });

    if let Some(error) = validation_error {
        return Err(R57PreparationFailureV1 { error, owners });
    }

    let owners: [R57OwnedAllocationV1; R57_BINDING_COUNT_V1] = match owners.try_into() {
        Ok(owners) => owners,
        Err(owners) => {
            return Err(R57PreparationFailureV1 {
                error: R57PreparationErrorV1::BindingCount,
                owners,
            });
        }
    };
    let opening = owners.each_ref().map(|owner| owner.snapshot_model_only());
    Ok(R57PreparedThreeBindingV1 {
        plan,
        owners,
        opening,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57PublicationScriptV1 {
    Complete,
    RejectBeforePublication,
    AmbiguousAfterWaitPublication,
    AmbiguousAfterDispatchPublication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57RestorationReasonV1 {
    OpeningCurrentnessRejected,
    NativeRejectedBeforeEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57QuarantineReasonV1 {
    AmbiguousAfterWaitPublication,
    AmbiguousAfterDispatchPublication,
    CompletionIdentityMismatch,
    ClosingCurrentnessRejected,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R57RestoredThreeBindingV1 {
    pub reason: R57RestorationReasonV1,
    owners: [R57OwnedAllocationV1; R57_BINDING_COUNT_V1],
    opening: [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1],
}

impl R57RestoredThreeBindingV1 {
    pub fn owner_snapshots_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.owners
            .each_ref()
            .map(|owner| owner.snapshot_model_only())
    }

    pub const fn opening_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.opening
    }

    pub fn into_owners_model_only(self) -> [R57OwnedAllocationV1; R57_BINDING_COUNT_V1] {
        self.owners
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R57PublishedThreeBindingV1 {
    plan: R57ThreeBindingPlanV1,
    owners: [R57OwnedAllocationV1; R57_BINDING_COUNT_V1],
    opening: [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1],
}

impl R57PublishedThreeBindingV1 {
    pub const fn plan_model_only(&self) -> R57ThreeBindingPlanV1 {
        self.plan
    }

    pub fn owner_snapshots_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.owners
            .each_ref()
            .map(|owner| owner.snapshot_model_only())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R57QuarantinedThreeBindingV1 {
    pub reason: R57QuarantineReasonV1,
    pub published_packet_prefix: u8,
    plan: R57ThreeBindingPlanV1,
    owners: [R57OwnedAllocationV1; R57_BINDING_COUNT_V1],
    opening: [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1],
}

impl R57QuarantinedThreeBindingV1 {
    pub fn owner_snapshots_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.owners
            .each_ref()
            .map(|owner| owner.snapshot_model_only())
    }

    pub const fn opening_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.opening
    }

    pub const fn plan_model_only(&self) -> R57ThreeBindingPlanV1 {
        self.plan
    }

    pub fn observe_again_model_only(self) -> Self {
        self
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R57PublishOutcomeV1 {
    Published(R57PublishedThreeBindingV1),
    Restored(R57RestoredThreeBindingV1),
    Quarantined(R57QuarantinedThreeBindingV1),
}

pub fn r57_publish_three_binding_compute_model_only(
    prepared: R57PreparedThreeBindingV1,
    opening_currentness: bool,
    script: R57PublicationScriptV1,
) -> R57PublishOutcomeV1 {
    let R57PreparedThreeBindingV1 {
        plan,
        owners,
        opening,
    } = prepared;
    if !opening_currentness {
        return R57PublishOutcomeV1::Restored(R57RestoredThreeBindingV1 {
            reason: R57RestorationReasonV1::OpeningCurrentnessRejected,
            owners,
            opening,
        });
    }
    match script {
        R57PublicationScriptV1::Complete => {
            R57PublishOutcomeV1::Published(R57PublishedThreeBindingV1 {
                plan,
                owners,
                opening,
            })
        }
        R57PublicationScriptV1::RejectBeforePublication => {
            R57PublishOutcomeV1::Restored(R57RestoredThreeBindingV1 {
                reason: R57RestorationReasonV1::NativeRejectedBeforeEffect,
                owners,
                opening,
            })
        }
        R57PublicationScriptV1::AmbiguousAfterWaitPublication => {
            R57PublishOutcomeV1::Quarantined(R57QuarantinedThreeBindingV1 {
                reason: R57QuarantineReasonV1::AmbiguousAfterWaitPublication,
                published_packet_prefix: 1,
                plan,
                owners,
                opening,
            })
        }
        R57PublicationScriptV1::AmbiguousAfterDispatchPublication => {
            R57PublishOutcomeV1::Quarantined(R57QuarantinedThreeBindingV1 {
                reason: R57QuarantineReasonV1::AmbiguousAfterDispatchPublication,
                published_packet_prefix: R57_PACKET_COUNT_V1,
                plan,
                owners,
                opening,
            })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R57CompletionIdentityV1 {
    pub queue: u64,
    pub queue_generation: u64,
    pub dispatch: u64,
    pub transaction_generation: u64,
    pub completion_signal: u64,
    pub frontier_after: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R57CompletionObservationV1 {
    Timeout,
    Completed(R57CompletionIdentityV1),
}

#[derive(Debug, Eq, PartialEq)]
pub struct R57CompletedThreeBindingV1 {
    pub completion: R57CompletionIdentityV1,
    owners: [R57OwnedAllocationV1; R57_BINDING_COUNT_V1],
    opening: [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1],
}

impl R57CompletedThreeBindingV1 {
    pub fn owner_snapshots_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.owners
            .each_ref()
            .map(|owner| owner.snapshot_model_only())
    }

    pub const fn opening_model_only(&self) -> [R57OwnerSnapshotV1; R57_BINDING_COUNT_V1] {
        self.opening
    }

    pub fn into_owners_model_only(self) -> [R57OwnedAllocationV1; R57_BINDING_COUNT_V1] {
        self.owners
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum R57WaitOutcomeV1 {
    Pending(R57PublishedThreeBindingV1),
    Completed(R57CompletedThreeBindingV1),
    Quarantined(R57QuarantinedThreeBindingV1),
}

fn exact_completion(plan: R57ThreeBindingPlanV1, observed: R57CompletionIdentityV1) -> bool {
    plan.frontier_after_model_only()
        .is_some_and(|frontier_after| {
            observed
                == R57CompletionIdentityV1 {
                    queue: plan.queue,
                    queue_generation: plan.queue_generation,
                    dispatch: plan.dispatch,
                    transaction_generation: plan.transaction_generation,
                    completion_signal: plan.completion_signal,
                    frontier_after,
                }
        })
}

pub fn r57_wait_three_binding_compute_model_only(
    published: R57PublishedThreeBindingV1,
    closing_currentness: bool,
    observation: R57CompletionObservationV1,
) -> R57WaitOutcomeV1 {
    if !closing_currentness {
        let R57PublishedThreeBindingV1 {
            plan,
            owners,
            opening,
        } = published;
        return R57WaitOutcomeV1::Quarantined(R57QuarantinedThreeBindingV1 {
            reason: R57QuarantineReasonV1::ClosingCurrentnessRejected,
            published_packet_prefix: R57_PACKET_COUNT_V1,
            plan,
            owners,
            opening,
        });
    }
    match observation {
        R57CompletionObservationV1::Timeout => R57WaitOutcomeV1::Pending(published),
        R57CompletionObservationV1::Completed(completion) => {
            let R57PublishedThreeBindingV1 {
                plan,
                mut owners,
                opening,
            } = published;
            if !exact_completion(plan, completion) {
                return R57WaitOutcomeV1::Quarantined(R57QuarantinedThreeBindingV1 {
                    reason: R57QuarantineReasonV1::CompletionIdentityMismatch,
                    published_packet_prefix: R57_PACKET_COUNT_V1,
                    plan,
                    owners,
                    opening,
                });
            }
            owners[2].snapshot.initialized = true;
            owners[2].snapshot.content_generation += 1;
            R57WaitOutcomeV1::Completed(R57CompletedThreeBindingV1 {
                completion,
                owners,
                opening,
            })
        }
    }
}
