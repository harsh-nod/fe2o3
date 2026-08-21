//! Linear identity and failure foundation for device-content initialization.
//!
//! The pinned KFD UAPI has no admitted memcpy or SDMA submission primitive.
//! The compute queue can return an exact recycled C3 lease, but no authenticated
//! copy-kernel path connects that return to this state machine. Consequently
//! this module deliberately has no production completion constructor and
//! cannot mint initialized content. It does close the host-side composition
//! needed by that future bridge: exact source, destination, content, operation,
//! publication, and completion identities; fail-before-side-effect retry; and
//! terminal retention after any ambiguous side effect.

#![allow(dead_code)]

use core::fmt;

use fe2o3_runtime_model::{
    DeviceKeyV1, MemoryMappingKeyV1, MemoryPublicationKeyV1, QueueKeyV1, VmKeyV1,
};
use sha2::{Digest, Sha256};

/// Frozen claim boundary for the unbacked KFD copy-composition foundation.
pub const GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-mi300x-gfx942-device-content-copy-foundation-r2-v1\n",
    "target=gfx942:xnack-,one-selected-current-device-vm-and-queue-generation\n",
    "content=nonzero-semantic-role-identity,u32-ordinal,nonzero-byte-extent,sha256,canonical-content-identity\n",
    "source=actual-private-mapped-gtt-allocation-and-publication-generations,exact-logical-byte-extent,no-address-export\n",
    "destination=actual-private-c3-device-vm-allocation-generations,checked-requested-byte-extent,no-address-export\n",
    "copy=nonzero-operation-and-publication-identities,exact-queue-and-dispatch-generation,one-packet-publication\n",
    "completion=exact-copy-publication-queue-dispatch-batch-signal-and-packet-generations,completed-not-caller-boolean\n",
    "failure=all-substitution-and-size-validation-before-side-effects,no-effect-retains-for-retry,packet-body-or-later-retains-poisoned-for-teardown\n",
    "quiescence=initialized-authority-form-exists-only-after-exact-completed-identity,source-and-destination-retained-through-completion\n",
    "authority=crate-private-linear-states,no-public-mint,no-handle-gpu-address-pointer-fd-packet-or-signal-export\n",
    "proof=bounded-host-state-machine-and-hostile-tests-only,no-concrete-verus-or-machine-refinement\n",
    "queue-prerequisite=actual-mapped-c3-authority-return-with-owning-memory-session-only-after-exact-C4-recycle-and-confirmed-queue-destroy,not-yet-connected-to-content-state-machine\n",
    "hard-boundary=no-admitted-kfd-memcpy-or-sdma-packet,no-authenticated-copy-kernel,no-production-copy-token-constructor,no-linux-copy-backend-or-hardware-evidence\n",
    "excluded=actual-copy,initialized-content-mint,dispatch-read-premise,alias-or-effect-proof,cpu-gpu-coherence,firmware-effects,public-launch\n",
);

/// SHA-256 of [`GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_V1`].
pub const GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_SHA256_V1: &str =
    "2e52b3b210f36729fd309b4973fbcbbb1fe9e325e95ae62f4e567f544f79eceb";

/// Semantic role and ordinal for one exact byte image.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Gfx942DeviceContentRoleV1 {
    identity: [u8; 32],
    ordinal: u32,
}

impl Gfx942DeviceContentRoleV1 {
    pub fn new(
        identity: [u8; 32],
        ordinal: u32,
    ) -> Result<Self, Gfx942DeviceContentDescriptorErrorV1> {
        if identity == [0; 32] {
            return Err(Gfx942DeviceContentDescriptorErrorV1::ZeroRoleIdentity);
        }
        Ok(Self { identity, ordinal })
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }

    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

/// Exact expected content of one initialized device-memory extent.
///
/// A descriptor is data, not initialization authority. It has no public method
/// that can convert it into a device-memory lease.
///
/// ```compile_fail
/// use fe2o3_kfd::{Gfx942DeviceContentDescriptorV1, Gfx942DeviceContentRoleV1};
///
/// let role = Gfx942DeviceContentRoleV1::new([1; 32], 0).unwrap();
/// let descriptor = Gfx942DeviceContentDescriptorV1::from_bytes(role, &[1]).unwrap();
/// let _forged = descriptor.into_initialized_lease();
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Gfx942DeviceContentDescriptorV1 {
    role: Gfx942DeviceContentRoleV1,
    byte_len: u64,
    sha256: [u8; 32],
    identity: [u8; 32],
}

impl Gfx942DeviceContentDescriptorV1 {
    pub fn new(
        role: Gfx942DeviceContentRoleV1,
        byte_len: u64,
        sha256: [u8; 32],
    ) -> Result<Self, Gfx942DeviceContentDescriptorErrorV1> {
        if byte_len == 0 {
            return Err(Gfx942DeviceContentDescriptorErrorV1::ZeroByteExtent);
        }
        if sha256 == [0; 32] {
            return Err(Gfx942DeviceContentDescriptorErrorV1::ZeroContentSha256);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"fe2o3-gfx942-device-content-v1\0");
        hasher.update(role.identity);
        hasher.update(role.ordinal.to_le_bytes());
        hasher.update(byte_len.to_le_bytes());
        hasher.update(sha256);
        Ok(Self {
            role,
            byte_len,
            sha256,
            identity: hasher.finalize().into(),
        })
    }

    pub fn from_bytes(
        role: Gfx942DeviceContentRoleV1,
        bytes: &[u8],
    ) -> Result<Self, Gfx942DeviceContentDescriptorErrorV1> {
        let byte_len = u64::try_from(bytes.len())
            .map_err(|_| Gfx942DeviceContentDescriptorErrorV1::ByteExtentOverflow)?;
        Self::new(role, byte_len, Sha256::digest(bytes).into())
    }

    pub const fn role(self) -> Gfx942DeviceContentRoleV1 {
        self.role
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942DeviceContentDescriptorErrorV1 {
    ZeroRoleIdentity,
    ZeroByteExtent,
    ZeroContentSha256,
    ByteExtentOverflow,
}

impl fmt::Display for Gfx942DeviceContentDescriptorErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for Gfx942DeviceContentDescriptorErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeviceCopySourceFactsV1 {
    mapping: MemoryMappingKeyV1,
    publication: MemoryPublicationKeyV1,
    logical_bytes: u64,
    content: Gfx942DeviceContentDescriptorV1,
}

impl DeviceCopySourceFactsV1 {
    pub(crate) const fn new(
        mapping: MemoryMappingKeyV1,
        publication: MemoryPublicationKeyV1,
        logical_bytes: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> Self {
        Self {
            mapping,
            publication,
            logical_bytes,
            content,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeviceCopyDestinationFactsV1 {
    allocation_id: u64,
    allocation_generation: u64,
    device: DeviceKeyV1,
    vm: VmKeyV1,
    requested_bytes: u64,
}

impl DeviceCopyDestinationFactsV1 {
    pub(crate) const fn new(
        allocation_id: u64,
        allocation_generation: u64,
        device: DeviceKeyV1,
        vm: VmKeyV1,
        requested_bytes: u64,
    ) -> Self {
        Self {
            allocation_id,
            allocation_generation,
            device,
            vm,
            requested_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeviceCopyIntentV1 {
    destination: DeviceCopyDestinationFactsV1,
    content: Gfx942DeviceContentDescriptorV1,
    operation_identity: [u8; 32],
    publication_identity: [u8; 32],
    queue: QueueKeyV1,
    dispatch_generation: u64,
}

impl DeviceCopyIntentV1 {
    pub(crate) const fn new(
        destination: DeviceCopyDestinationFactsV1,
        content: Gfx942DeviceContentDescriptorV1,
        operation_identity: [u8; 32],
        publication_identity: [u8; 32],
        queue: QueueKeyV1,
        dispatch_generation: u64,
    ) -> Self {
        Self {
            destination,
            content,
            operation_identity,
            publication_identity,
            queue,
            dispatch_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DeviceCopyCompletionIdentityV1 {
    operation_identity: [u8; 32],
    publication_identity: [u8; 32],
    queue: QueueKeyV1,
    dispatch_generation: u64,
    completion_batch_id: u64,
    signal_slot: u32,
    signal_generation: u64,
    last_packet_id: u64,
}

/// Queue-owned proof object for one actual copy publication.
///
/// Production has deliberately no constructor. The required follow-up bridge
/// must consume an authenticated copy-kernel dispatch and its exact C2
/// publication to construct this token. Raw caller fields are insufficient.
pub(crate) struct AuthenticatedDeviceCopyPublicationV1 {
    identity: DeviceCopyCompletionIdentityV1,
}

impl AuthenticatedDeviceCopyPublicationV1 {
    #[cfg(test)]
    fn for_test(identity: DeviceCopyCompletionIdentityV1) -> Self {
        Self { identity }
    }
}

/// Queue-owned proof object for one actual completed copy.
///
/// Production has deliberately no constructor. The required follow-up bridge
/// must consume the exact C4 completed batch for the authenticated copy
/// publication and the exact recycled C3 return to construct this token. Raw
/// caller fields or a boolean are insufficient.
pub(crate) struct AuthenticatedDeviceCopyCompletionV1 {
    identity: DeviceCopyCompletionIdentityV1,
}

impl AuthenticatedDeviceCopyCompletionV1 {
    #[cfg(test)]
    fn for_test(identity: DeviceCopyCompletionIdentityV1) -> Self {
        Self { identity }
    }
}

impl DeviceCopyCompletionIdentityV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) const fn new(
        operation_identity: [u8; 32],
        publication_identity: [u8; 32],
        queue: QueueKeyV1,
        dispatch_generation: u64,
        completion_batch_id: u64,
        signal_slot: u32,
        signal_generation: u64,
        last_packet_id: u64,
    ) -> Self {
        Self {
            operation_identity,
            publication_identity,
            queue,
            dispatch_generation,
            completion_batch_id,
            signal_slot,
            signal_generation,
            last_packet_id,
        }
    }

    fn is_well_formed(self) -> bool {
        self.operation_identity != [0; 32]
            && self.publication_identity != [0; 32]
            && self.dispatch_generation != 0
            && self.completion_batch_id != 0
            && self.signal_generation != 0
            && self.last_packet_id != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeviceCopyPreflightErrorV1 {
    ZeroAllocationIdentity,
    StaleDestinationIdentity,
    CrossDevice,
    CrossVm,
    SourcePublicationSubstitution,
    ContentSubstitution,
    InvalidSourceExtent,
    InvalidDestinationExtent,
    ZeroOperationIdentity,
    ZeroPublicationIdentity,
    WrongQueueGeneration,
    ZeroDispatchGeneration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeviceCopyTransitionErrorV1 {
    PublicationSubstitution,
    CompletionSubstitution,
    CopyFailed,
}

/// Boundary reached by a failed native copy publication attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeviceCopySideEffectBoundaryV1 {
    NoSideEffects,
    PacketBodyWritten,
    PacketHeaderPublished,
    DoorbellRung,
    CompletionObservationStarted,
    SignalRecycleStarted,
}

#[derive(Debug)]
pub(crate) struct DeviceCopyPreflightFailureV1<S, D> {
    error: DeviceCopyPreflightErrorV1,
    source: S,
    destination: D,
}

impl<S, D> DeviceCopyPreflightFailureV1<S, D> {
    pub(crate) const fn error(&self) -> DeviceCopyPreflightErrorV1 {
        self.error
    }

    pub(crate) fn into_resources(self) -> (S, D) {
        (self.source, self.destination)
    }
}

pub(crate) struct PreparedDeviceContentCopyV1<S, D> {
    source: S,
    destination: D,
    source_facts: DeviceCopySourceFactsV1,
    intent: DeviceCopyIntentV1,
}

pub(crate) struct PublishedDeviceContentCopyV1<S, D> {
    source: S,
    destination: D,
    source_facts: DeviceCopySourceFactsV1,
    intent: DeviceCopyIntentV1,
    completion: DeviceCopyCompletionIdentityV1,
}

#[derive(Debug)]
pub(crate) struct PoisonedDeviceContentCopyV1<S, D> {
    source: S,
    destination: D,
    error: DeviceCopyTransitionErrorV1,
    boundary: DeviceCopySideEffectBoundaryV1,
}

pub(crate) struct InitializedDeviceContentAuthorityV1<D> {
    destination: D,
    destination_facts: DeviceCopyDestinationFactsV1,
    content: Gfx942DeviceContentDescriptorV1,
    completion: DeviceCopyCompletionIdentityV1,
}

pub(crate) enum DeviceCopyPublicationFailureV1<S, D> {
    Retryable(Box<PreparedDeviceContentCopyV1<S, D>>),
    Poisoned(PoisonedDeviceContentCopyV1<S, D>),
}

pub(crate) enum DeviceCopyCompletionPollV1<S, D> {
    Pending(Box<PublishedDeviceContentCopyV1<S, D>>),
    Completed {
        source: S,
        initialized: Box<InitializedDeviceContentAuthorityV1<D>>,
    },
    Poisoned(PoisonedDeviceContentCopyV1<S, D>),
}

pub(crate) fn prepare_device_content_copy<S, D>(
    source: S,
    destination: D,
    source_facts: DeviceCopySourceFactsV1,
    destination_facts: DeviceCopyDestinationFactsV1,
    intent: DeviceCopyIntentV1,
) -> Result<PreparedDeviceContentCopyV1<S, D>, DeviceCopyPreflightFailureV1<S, D>> {
    let result = validate_preflight(source_facts, destination_facts, intent);
    match result {
        Ok(()) => Ok(PreparedDeviceContentCopyV1 {
            source,
            destination,
            source_facts,
            intent,
        }),
        Err(error) => Err(DeviceCopyPreflightFailureV1 {
            error,
            source,
            destination,
        }),
    }
}

fn validate_preflight(
    source: DeviceCopySourceFactsV1,
    destination: DeviceCopyDestinationFactsV1,
    intent: DeviceCopyIntentV1,
) -> Result<(), DeviceCopyPreflightErrorV1> {
    if destination.allocation_id == 0 || destination.allocation_generation == 0 {
        return Err(DeviceCopyPreflightErrorV1::ZeroAllocationIdentity);
    }
    if destination != intent.destination {
        return Err(DeviceCopyPreflightErrorV1::StaleDestinationIdentity);
    }
    if destination.device != destination.vm.device {
        return Err(DeviceCopyPreflightErrorV1::CrossDevice);
    }
    if source.mapping.allocation.vm.device != destination.device {
        return Err(DeviceCopyPreflightErrorV1::CrossDevice);
    }
    if source.mapping.allocation.vm != destination.vm {
        return Err(DeviceCopyPreflightErrorV1::CrossVm);
    }
    if source.publication.mapping != source.mapping {
        return Err(DeviceCopyPreflightErrorV1::SourcePublicationSubstitution);
    }
    if source.content != intent.content {
        return Err(DeviceCopyPreflightErrorV1::ContentSubstitution);
    }
    if source.logical_bytes == 0 || source.logical_bytes != intent.content.byte_len {
        return Err(DeviceCopyPreflightErrorV1::InvalidSourceExtent);
    }
    if destination.requested_bytes == 0 || intent.content.byte_len > destination.requested_bytes {
        return Err(DeviceCopyPreflightErrorV1::InvalidDestinationExtent);
    }
    if intent.operation_identity == [0; 32] {
        return Err(DeviceCopyPreflightErrorV1::ZeroOperationIdentity);
    }
    if intent.publication_identity == [0; 32] {
        return Err(DeviceCopyPreflightErrorV1::ZeroPublicationIdentity);
    }
    if intent.queue.vm != destination.vm {
        return Err(DeviceCopyPreflightErrorV1::WrongQueueGeneration);
    }
    if intent.dispatch_generation == 0 {
        return Err(DeviceCopyPreflightErrorV1::ZeroDispatchGeneration);
    }
    Ok(())
}

impl<S, D> PreparedDeviceContentCopyV1<S, D> {
    pub(crate) fn publication_failed(
        self,
        boundary: DeviceCopySideEffectBoundaryV1,
    ) -> DeviceCopyPublicationFailureV1<S, D> {
        if boundary == DeviceCopySideEffectBoundaryV1::NoSideEffects {
            DeviceCopyPublicationFailureV1::Retryable(Box::new(self))
        } else {
            DeviceCopyPublicationFailureV1::Poisoned(PoisonedDeviceContentCopyV1 {
                source: self.source,
                destination: self.destination,
                error: DeviceCopyTransitionErrorV1::CopyFailed,
                boundary,
            })
        }
    }

    pub(crate) fn mark_published(
        self,
        authenticated: AuthenticatedDeviceCopyPublicationV1,
    ) -> Result<PublishedDeviceContentCopyV1<S, D>, PoisonedDeviceContentCopyV1<S, D>> {
        let completion = authenticated.identity;
        let exact = completion.is_well_formed()
            && completion.operation_identity == self.intent.operation_identity
            && completion.publication_identity == self.intent.publication_identity
            && completion.queue == self.intent.queue
            && completion.dispatch_generation == self.intent.dispatch_generation;
        if !exact {
            return Err(PoisonedDeviceContentCopyV1 {
                source: self.source,
                destination: self.destination,
                error: DeviceCopyTransitionErrorV1::PublicationSubstitution,
                boundary: DeviceCopySideEffectBoundaryV1::PacketHeaderPublished,
            });
        }
        Ok(PublishedDeviceContentCopyV1 {
            source: self.source,
            destination: self.destination,
            source_facts: self.source_facts,
            intent: self.intent,
            completion,
        })
    }
}

impl<S, D> PublishedDeviceContentCopyV1<S, D> {
    pub(crate) fn observe_pending(self) -> DeviceCopyCompletionPollV1<S, D> {
        DeviceCopyCompletionPollV1::Pending(Box::new(self))
    }

    pub(crate) fn observe_completed(
        self,
        authenticated: AuthenticatedDeviceCopyCompletionV1,
    ) -> DeviceCopyCompletionPollV1<S, D> {
        let observed = authenticated.identity;
        if observed != self.completion {
            return DeviceCopyCompletionPollV1::Poisoned(PoisonedDeviceContentCopyV1 {
                source: self.source,
                destination: self.destination,
                error: DeviceCopyTransitionErrorV1::CompletionSubstitution,
                boundary: DeviceCopySideEffectBoundaryV1::CompletionObservationStarted,
            });
        }
        DeviceCopyCompletionPollV1::Completed {
            source: self.source,
            initialized: Box::new(InitializedDeviceContentAuthorityV1 {
                destination: self.destination,
                destination_facts: self.intent.destination,
                content: self.source_facts.content,
                completion: self.completion,
            }),
        }
    }

    pub(crate) fn observe_copy_failure(self) -> PoisonedDeviceContentCopyV1<S, D> {
        PoisonedDeviceContentCopyV1 {
            source: self.source,
            destination: self.destination,
            error: DeviceCopyTransitionErrorV1::CopyFailed,
            boundary: DeviceCopySideEffectBoundaryV1::CompletionObservationStarted,
        }
    }
}

impl<S, D> PoisonedDeviceContentCopyV1<S, D> {
    pub(crate) const fn error(&self) -> DeviceCopyTransitionErrorV1 {
        self.error
    }

    pub(crate) const fn boundary(&self) -> DeviceCopySideEffectBoundaryV1 {
        self.boundary
    }

    pub(crate) fn into_resources_for_teardown(self) -> (S, D) {
        (self.source, self.destination)
    }
}

impl<D> InitializedDeviceContentAuthorityV1<D> {
    pub(crate) const fn content(&self) -> Gfx942DeviceContentDescriptorV1 {
        self.content
    }

    pub(crate) fn into_destination(self) -> D {
        self.destination
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_runtime_model::{
        AllocationGenerationV1, AllocationIdV1, DeviceGenerationV1, MappingIdV1,
        MemoryAllocationKeyV1, MemoryPublicationIdV1, PhysicalDeviceIdV1, QueueGenerationV1,
        QueueInstanceIdV1, VmIdV1,
    };

    fn device(physical: u64, generation: u64) -> DeviceKeyV1 {
        DeviceKeyV1 {
            physical: PhysicalDeviceIdV1(physical),
            generation: DeviceGenerationV1(generation),
        }
    }

    fn vm(device: DeviceKeyV1, id: u64) -> VmKeyV1 {
        VmKeyV1 {
            device,
            id: VmIdV1(id),
        }
    }

    fn mapping(vm: VmKeyV1, id: u64, generation: u64) -> MemoryMappingKeyV1 {
        MemoryMappingKeyV1 {
            allocation: MemoryAllocationKeyV1 {
                vm,
                id: AllocationIdV1(id),
                generation: AllocationGenerationV1(generation),
            },
            id: MappingIdV1(id),
        }
    }

    fn queue(vm: VmKeyV1) -> QueueKeyV1 {
        QueueKeyV1 {
            vm,
            id: QueueInstanceIdV1(31),
            generation: QueueGenerationV1(7),
        }
    }

    fn content(seed: u8) -> Gfx942DeviceContentDescriptorV1 {
        Gfx942DeviceContentDescriptorV1::from_bytes(
            Gfx942DeviceContentRoleV1::new([seed; 32], 4).unwrap(),
            &[seed; 4096],
        )
        .unwrap()
    }

    fn fixture() -> (
        DeviceCopySourceFactsV1,
        DeviceCopyDestinationFactsV1,
        DeviceCopyIntentV1,
        DeviceCopyCompletionIdentityV1,
    ) {
        let device = device(9, 3);
        let vm = vm(device, 11);
        let mapping = mapping(vm, 21, 5);
        let source = DeviceCopySourceFactsV1::new(
            mapping,
            MemoryPublicationKeyV1 {
                mapping,
                id: MemoryPublicationIdV1(23),
            },
            4096,
            content(0x51),
        );
        let destination = DeviceCopyDestinationFactsV1::new(41, 2, device, vm, 8192);
        let intent = DeviceCopyIntentV1::new(
            destination,
            source.content,
            [0x61; 32],
            [0x71; 32],
            queue(vm),
            13,
        );
        let completion = DeviceCopyCompletionIdentityV1::new(
            intent.operation_identity,
            intent.publication_identity,
            intent.queue,
            intent.dispatch_generation,
            17,
            2,
            19,
            29,
        );
        (source, destination, intent, completion)
    }

    fn prepared() -> PreparedDeviceContentCopyV1<&'static str, &'static str> {
        let (source, destination, intent, _) = fixture();
        prepare_device_content_copy("source", "destination", source, destination, intent).unwrap()
    }

    #[test]
    fn manifest_digest_is_frozen() {
        let digest = Sha256::digest(GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(
            rendered,
            GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_SHA256_V1
        );
    }

    #[test]
    fn content_identity_is_canonical_and_binds_role_ordinal_extent_and_sha() {
        let base = content(0x51);
        assert_eq!(base.byte_len(), 4096);
        assert_ne!(base.sha256(), [0; 32]);
        assert_ne!(base.identity(), [0; 32]);
        let other_role = Gfx942DeviceContentDescriptorV1::new(
            Gfx942DeviceContentRoleV1::new([0x52; 32], 4).unwrap(),
            base.byte_len(),
            base.sha256(),
        )
        .unwrap();
        let other_ordinal = Gfx942DeviceContentDescriptorV1::new(
            Gfx942DeviceContentRoleV1::new([0x51; 32], 5).unwrap(),
            base.byte_len(),
            base.sha256(),
        )
        .unwrap();
        let other_extent =
            Gfx942DeviceContentDescriptorV1::new(base.role(), base.byte_len() + 1, base.sha256())
                .unwrap();
        assert_ne!(base.identity(), other_role.identity());
        assert_ne!(base.identity(), other_ordinal.identity());
        assert_ne!(base.identity(), other_extent.identity());
        assert_eq!(
            Gfx942DeviceContentRoleV1::new([0; 32], 0),
            Err(Gfx942DeviceContentDescriptorErrorV1::ZeroRoleIdentity)
        );
        assert_eq!(
            Gfx942DeviceContentDescriptorV1::new(base.role(), 0, base.sha256()),
            Err(Gfx942DeviceContentDescriptorErrorV1::ZeroByteExtent)
        );
        assert_eq!(
            Gfx942DeviceContentDescriptorV1::new(base.role(), 1, [0; 32]),
            Err(Gfx942DeviceContentDescriptorErrorV1::ZeroContentSha256)
        );
    }

    #[test]
    fn preflight_rejects_every_identity_and_extent_substitution_without_losing_inputs() {
        let (source, destination, intent, _) = fixture();
        let mut cases = Vec::new();

        let mut value = destination;
        value.allocation_generation += 1;
        cases.push((
            source,
            value,
            intent,
            DeviceCopyPreflightErrorV1::StaleDestinationIdentity,
        ));

        let mut value = destination;
        value.allocation_id += 1;
        cases.push((
            source,
            value,
            intent,
            DeviceCopyPreflightErrorV1::StaleDestinationIdentity,
        ));

        let mut value = destination;
        value.device = device(10, 3);
        let mut request = intent;
        request.destination = value;
        cases.push((
            source,
            value,
            request,
            DeviceCopyPreflightErrorV1::CrossDevice,
        ));

        let mut source_vm = source;
        source_vm.mapping.allocation.vm.id = VmIdV1(12);
        source_vm.publication.mapping = source_vm.mapping;
        cases.push((
            source_vm,
            destination,
            intent,
            DeviceCopyPreflightErrorV1::CrossVm,
        ));

        let mut source_publication = source;
        source_publication.publication.mapping.id = MappingIdV1(99);
        cases.push((
            source_publication,
            destination,
            intent,
            DeviceCopyPreflightErrorV1::SourcePublicationSubstitution,
        ));

        let mut source_content = source;
        source_content.content = content(0x52);
        cases.push((
            source_content,
            destination,
            intent,
            DeviceCopyPreflightErrorV1::ContentSubstitution,
        ));

        let mut source_role = source;
        source_role.content = Gfx942DeviceContentDescriptorV1::new(
            Gfx942DeviceContentRoleV1::new([0x52; 32], source.content.role().ordinal()).unwrap(),
            source.content.byte_len(),
            source.content.sha256(),
        )
        .unwrap();
        cases.push((
            source_role,
            destination,
            intent,
            DeviceCopyPreflightErrorV1::ContentSubstitution,
        ));

        let mut source_ordinal = source;
        source_ordinal.content = Gfx942DeviceContentDescriptorV1::new(
            Gfx942DeviceContentRoleV1::new(
                source.content.role().identity(),
                source.content.role().ordinal() + 1,
            )
            .unwrap(),
            source.content.byte_len(),
            source.content.sha256(),
        )
        .unwrap();
        cases.push((
            source_ordinal,
            destination,
            intent,
            DeviceCopyPreflightErrorV1::ContentSubstitution,
        ));

        let mut source_sha = source;
        let mut substituted_sha = source.content.sha256();
        substituted_sha[0] ^= 1;
        source_sha.content = Gfx942DeviceContentDescriptorV1::new(
            source.content.role(),
            source.content.byte_len(),
            substituted_sha,
        )
        .unwrap();
        cases.push((
            source_sha,
            destination,
            intent,
            DeviceCopyPreflightErrorV1::ContentSubstitution,
        ));

        let mut source_extent = source;
        source_extent.logical_bytes -= 1;
        cases.push((
            source_extent,
            destination,
            intent,
            DeviceCopyPreflightErrorV1::InvalidSourceExtent,
        ));

        let mut destination_extent = destination;
        destination_extent.requested_bytes = 1024;
        let mut request = intent;
        request.destination = destination_extent;
        cases.push((
            source,
            destination_extent,
            request,
            DeviceCopyPreflightErrorV1::InvalidDestinationExtent,
        ));

        let mut request = intent;
        request.operation_identity = [0; 32];
        cases.push((
            source,
            destination,
            request,
            DeviceCopyPreflightErrorV1::ZeroOperationIdentity,
        ));

        let mut request = intent;
        request.publication_identity = [0; 32];
        cases.push((
            source,
            destination,
            request,
            DeviceCopyPreflightErrorV1::ZeroPublicationIdentity,
        ));

        let mut request = intent;
        request.queue.vm.id = VmIdV1(12);
        cases.push((
            source,
            destination,
            request,
            DeviceCopyPreflightErrorV1::WrongQueueGeneration,
        ));

        let mut request = intent;
        request.dispatch_generation = 0;
        cases.push((
            source,
            destination,
            request,
            DeviceCopyPreflightErrorV1::ZeroDispatchGeneration,
        ));

        for (source, destination, intent, expected) in cases {
            let failure =
                prepare_device_content_copy("source", "destination", source, destination, intent)
                    .err()
                    .unwrap();
            assert_eq!(failure.error(), expected);
            assert_eq!(failure.into_resources(), ("source", "destination"));
        }
    }

    #[test]
    fn only_no_effect_failure_is_retryable_and_every_later_boundary_is_poisoned() {
        let retry = prepared().publication_failed(DeviceCopySideEffectBoundaryV1::NoSideEffects);
        assert!(matches!(
            retry,
            DeviceCopyPublicationFailureV1::Retryable(_)
        ));

        for boundary in [
            DeviceCopySideEffectBoundaryV1::PacketBodyWritten,
            DeviceCopySideEffectBoundaryV1::PacketHeaderPublished,
            DeviceCopySideEffectBoundaryV1::DoorbellRung,
            DeviceCopySideEffectBoundaryV1::CompletionObservationStarted,
            DeviceCopySideEffectBoundaryV1::SignalRecycleStarted,
        ] {
            let DeviceCopyPublicationFailureV1::Poisoned(poisoned) =
                prepared().publication_failed(boundary)
            else {
                panic!("side-effect failure was retryable");
            };
            assert_eq!(poisoned.error(), DeviceCopyTransitionErrorV1::CopyFailed);
            assert_eq!(poisoned.boundary(), boundary);
            assert_eq!(
                poisoned.into_resources_for_teardown(),
                ("source", "destination")
            );
        }
    }

    #[test]
    fn publication_and_completion_are_exact_generation_bound_and_fail_closed() {
        let (_, _, _, completion) = fixture();
        let mut substitutions = Vec::new();
        let mut value = completion;
        value.operation_identity[0] ^= 1;
        substitutions.push(value);
        let mut value = completion;
        value.publication_identity[0] ^= 1;
        substitutions.push(value);
        let mut value = completion;
        value.queue.generation = QueueGenerationV1(8);
        substitutions.push(value);
        let mut value = completion;
        value.dispatch_generation += 1;
        substitutions.push(value);
        let mut value = completion;
        value.completion_batch_id = 0;
        substitutions.push(value);
        let mut value = completion;
        value.signal_generation = 0;
        substitutions.push(value);
        let mut value = completion;
        value.last_packet_id = 0;
        substitutions.push(value);

        for substituted in substitutions {
            let poisoned = prepared()
                .mark_published(AuthenticatedDeviceCopyPublicationV1::for_test(substituted))
                .err()
                .unwrap();
            assert_eq!(
                poisoned.error(),
                DeviceCopyTransitionErrorV1::PublicationSubstitution
            );
            assert_eq!(
                poisoned.into_resources_for_teardown(),
                ("source", "destination")
            );
        }

        let pending = prepared()
            .mark_published(AuthenticatedDeviceCopyPublicationV1::for_test(completion))
            .unwrap();
        let DeviceCopyCompletionPollV1::Pending(_) = pending.observe_pending() else {
            unreachable!();
        };

        let mut completion_substitutions = Vec::new();
        let mut wrong = completion;
        wrong.completion_batch_id += 1;
        completion_substitutions.push(wrong);
        let mut wrong = completion;
        wrong.signal_slot += 1;
        completion_substitutions.push(wrong);
        let mut wrong = completion;
        wrong.signal_generation += 1;
        completion_substitutions.push(wrong);
        let mut wrong = completion;
        wrong.last_packet_id += 1;
        completion_substitutions.push(wrong);
        for wrong in completion_substitutions {
            let published = prepared()
                .mark_published(AuthenticatedDeviceCopyPublicationV1::for_test(completion))
                .unwrap();
            let DeviceCopyCompletionPollV1::Poisoned(poisoned) =
                published.observe_completed(AuthenticatedDeviceCopyCompletionV1::for_test(wrong))
            else {
                panic!("wrong completion minted initialized content");
            };
            assert_eq!(
                poisoned.error(),
                DeviceCopyTransitionErrorV1::CompletionSubstitution
            );
            assert_eq!(
                poisoned.into_resources_for_teardown(),
                ("source", "destination")
            );
        }
    }

    #[test]
    fn exact_completion_is_the_only_initialized_authority_mint() {
        let (_, _, _, completion) = fixture();
        let published = prepared()
            .mark_published(AuthenticatedDeviceCopyPublicationV1::for_test(completion))
            .unwrap();
        let DeviceCopyCompletionPollV1::Completed {
            source,
            initialized,
        } = published.observe_completed(AuthenticatedDeviceCopyCompletionV1::for_test(completion))
        else {
            panic!("exact completion did not initialize");
        };
        assert_eq!(source, "source");
        assert_eq!(initialized.content(), content(0x51));
        assert_eq!(initialized.into_destination(), "destination");

        let (_, _, _, completion) = fixture();
        let poisoned = prepared()
            .mark_published(AuthenticatedDeviceCopyPublicationV1::for_test(completion))
            .unwrap()
            .observe_copy_failure();
        assert_eq!(poisoned.error(), DeviceCopyTransitionErrorV1::CopyFailed);
        assert_eq!(
            poisoned.into_resources_for_teardown(),
            ("source", "destination")
        );
    }
}
