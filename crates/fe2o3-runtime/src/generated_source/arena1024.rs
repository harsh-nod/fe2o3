//! Original carriers remain independent; only their closed WO DATA is aggregated.

use super::*;
use fe2o3_resource_accounting::{HostMetadataTableV1, ResourceCreditAccountV1};
use std::sync::Arc;

mod inputs;

pub(crate) const SLOTS: usize = fe2o3_kfd::GFX942_NATIVE_FILL_ARENA_SLOTS_V1;

/// Refusal during effect-free preparation of the exact original carrier roster.
#[derive(Debug)]
pub enum RuntimeGfx942ArenaPreparationErrorV1<E> {
    Capacity,
    Member { index: usize, error: E },
    Source(RuntimeGfx942GeneratedReservationErrorV1),
}

/// Closed 1024-original, single-use fill arena. This is not a scalar carrier,
/// graph result, native admission, independent scheduling or common retirement.
/// Original source/authority loans and result residuals remain owned until the
/// runtime has destroyed the actual common native backing.
///
/// Carrier headers, original identity records and packet headers are separately
/// prepaid in `metadata`. Existing carrier source/result storage retains its own
/// original accounts. Allocator bookkeeping is not included in these payloads.
///
/// ```compile_fail
/// use fe2o3_runtime::{RuntimeGfx942GeneratedArena1024V1, RuntimeGfx942GeneratedCarrierV1};
/// fn scalar<T: RuntimeGfx942GeneratedCarrierV1>(_: T) {}
/// fn refuse<P>(arena: RuntimeGfx942GeneratedArena1024V1<P>) { scalar(arena); }
/// ```
#[must_use]
pub struct RuntimeGfx942GeneratedArena1024V1<P> {
    pub(crate) members: HostMetadataTableV1<Option<P>>,
    originals: HostMetadataTableV1<Option<Original>>,
    pub(crate) packets: Option<fe2o3_kfd::Gfx942NativeFillArenaPacketsV1>,
    roster: GeneratedHostRosterV1,
    metadata: ResourceCreditAccountV1,
}

/// Separately branded independent-disjoint-WO roster. It cannot be submitted
/// through a scalar or the ordered arena API and carries no native authority.
///
/// ```compile_fail
/// use fe2o3_runtime::{RuntimeGfx942GeneratedArena1024V1 as Ordered,
///                    RuntimeGfx942GeneratedIndependentArena1024V1 as Independent};
/// fn refuse<P>(source: Independent<P>) -> Ordered<P> { source }
/// ```
/// ```compile_fail
/// use fe2o3_runtime::{RuntimeGfx942GeneratedCarrierV1,
///                    RuntimeGfx942GeneratedIndependentArena1024V1 as Independent};
/// fn scalar<P: RuntimeGfx942GeneratedCarrierV1>(_: P) {}
/// fn refuse<P>(source: Independent<P>) { scalar(source); }
/// ```
pub struct RuntimeGfx942GeneratedIndependentArena1024V1<P>(
    pub(crate) RuntimeGfx942GeneratedArena1024V1<P>,
);

impl<P: RuntimeGfx942GeneratedCarrierV1> RuntimeGfx942GeneratedIndependentArena1024V1<P> {
    pub fn try_new<E>(
        metadata: &ResourceCreditAccountV1,
        device_unique_id: u64,
        prepare: impl FnMut(usize) -> Result<P, E>,
    ) -> Result<Self, RuntimeGfx942ArenaPreparationErrorV1<E>> {
        RuntimeGfx942GeneratedArena1024V1::try_new_with_order(
            metadata,
            device_unique_id,
            prepare,
            true,
        )
        .map(Self)
    }
}

#[derive(Clone)]
pub(crate) struct Original {
    source: Arc<()>,
    contract: [u8; 32],
    offset: u64,
    bytes: u64,
    independent: bool,
}

impl Original {
    pub(crate) fn matches(&self, roster: &GeneratedHostRosterV1) -> bool {
        (match (&roster.source_identity, self.independent) {
            (GeneratedSourceIdentityV1::Singleton(actual), false)
            | (GeneratedSourceIdentityV1::IndependentArenaMember(actual), true) => {
                Arc::ptr_eq(actual, &self.source)
            }
            _ => false,
        }) && roster.dispatch_contract_sha256
            == if self.independent {
                GeneratedContractsV1::IndependentArenaMember(self.contract)
            } else {
                GeneratedContractsV1::Singleton(self.contract)
            }
            && roster.count == 1
            && roster.fixup_count == 1
            && roster.readback_bytes == self.bytes
            && roster.buffers[0]
                == Some(GeneratedBufferSlotV1 {
                    ordinal: 0,
                    bytes: self.bytes,
                    access: Gfx942RuntimeBufferAccessV1::WriteOnly,
                })
            && roster.buffers[1..].iter().all(Option::is_none)
    }
}

impl<P: RuntimeGfx942GeneratedCarrierV1> RuntimeGfx942GeneratedArena1024V1<P> {
    /// Calls `prepare` in order, at most once per member. Any refusal disposes
    /// only effect-free preparations; this constructor cannot enter native work.
    pub fn try_new<E>(
        metadata: &ResourceCreditAccountV1,
        device_unique_id: u64,
        prepare: impl FnMut(usize) -> Result<P, E>,
    ) -> Result<Self, RuntimeGfx942ArenaPreparationErrorV1<E>> {
        Self::try_new_with_order(metadata, device_unique_id, prepare, false)
    }

    fn try_new_with_order<E>(
        metadata: &ResourceCreditAccountV1,
        device_unique_id: u64,
        mut prepare: impl FnMut(usize) -> Result<P, E>,
        independent: bool,
    ) -> Result<Self, RuntimeGfx942ArenaPreparationErrorV1<E>> {
        use RuntimeGfx942ArenaPreparationErrorV1 as Error;
        let mut failure = None;
        let mut next = 0;
        let mut members = HostMetadataTableV1::try_new(SLOTS, Some(metadata), || {
            if failure.is_some() {
                return None;
            }
            let index = next;
            next += 1;
            match prepare(index) {
                Ok(member) => Some(member),
                Err(error) => {
                    failure = Some(Error::Member { index, error });
                    None
                }
            }
        })
        .map_err(|_| Error::Capacity)?;
        if let Some(error) = failure {
            return Err(error);
        }
        let mut originals = HostMetadataTableV1::try_new(SLOTS, Some(metadata), || None)
            .map_err(|_| Error::Capacity)?;
        let mut addresses = HostMetadataTableV1::try_new(SLOTS, Some(metadata), || 0usize)
            .map_err(|_| Error::Capacity)?;
        let mut offset = 0u64;
        let mut hash = Sha256::new();
        hash.update(if independent {
            b"fe2o3.generated.independent-fill-arena1024.contracts.v1\0".as_slice()
        } else {
            b"fe2o3.generated.native-fill-arena1024.contracts.v1\0".as_slice()
        });
        for index in 0..SLOTS {
            let member = members[index]
                .as_ref()
                .unwrap_or_else(|| std::process::abort());
            let source = member.source();
            if !matches!(
                (source.projection.invocation_binding(), independent),
                (
                    crate::Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 { .. },
                    false
                ) | (
                    crate::Gfx942RuntimeInvocationBindingV1::NativeIndependentFill64V1 { .. },
                    true
                )
            ) {
                return Err(Error::Source(
                    RuntimeGfx942GeneratedReservationErrorV1::AuthorityMismatch,
                ));
            }
            let actual = source.validate(device_unique_id).map_err(Error::Source)?;
            let identity = match (&actual.source_identity, independent) {
                (GeneratedSourceIdentityV1::Singleton(identity), false)
                | (GeneratedSourceIdentityV1::IndependentArenaMember(identity), true) => identity,
                _ => {
                    return Err(Error::Source(
                        RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
                    ));
                }
            };
            let contract = match (actual.dispatch_contract_sha256, independent) {
                (GeneratedContractsV1::Singleton(contract), false)
                | (GeneratedContractsV1::IndependentArenaMember(contract), true) => contract,
                _ => {
                    return Err(Error::Source(
                        RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
                    ));
                }
            };
            let entry = Original {
                source: Arc::clone(identity),
                contract,
                offset,
                bytes: actual.readback_bytes,
                independent,
            };
            if entry.bytes == 0
                || !entry.matches(&actual)
                || (index != 0
                    && source.hsaco
                        != members[0]
                            .as_ref()
                            .unwrap_or_else(|| std::process::abort())
                            .source()
                            .hsaco)
            {
                return Err(Error::Source(
                    RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
                ));
            }
            offset = offset.checked_add(entry.bytes).ok_or(Error::Source(
                RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
            ))?;
            hash.update(contract);
            hash.update(entry.offset.to_le_bytes());
            hash.update(entry.bytes.to_le_bytes());
            addresses[index] = Arc::as_ptr(identity) as usize;
            originals[index] = Some(entry);
        }
        // Addresses only reject aliases while all original Arc owners remain
        // retained. They are never authority or a persisted identity.
        addresses.sort_unstable();
        if addresses.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::Source(
                RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
            ));
        }
        drop(addresses);
        if usize::try_from(offset).is_err() {
            return Err(Error::Source(
                RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
            ));
        }
        // Refuse unsupported mutable carriers before moving any original control.
        for (member, original) in members.iter_mut().zip(originals.iter()) {
            let source = member
                .as_mut()
                .unwrap_or_else(|| std::process::abort())
                .source_mut()
                .ok_or(Error::Source(
                    RuntimeGfx942GeneratedReservationErrorV1::UnsupportedPreparation,
                ))?;
            if !original
                .as_ref()
                .unwrap_or_else(|| std::process::abort())
                .matches(&source.validate(device_unique_id).map_err(Error::Source)?)
            {
                return Err(Error::Source(
                    RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
                ));
            }
        }
        let packets = fe2o3_kfd::Gfx942NativeFillArenaPacketsV1::try_new(metadata, |index| {
            let mut source = members[index]
                .as_mut()
                .unwrap_or_else(|| std::process::abort())
                .source_mut()
                .unwrap_or_else(|| std::process::abort());
            let mut packet = None;
            if !source.transfer_control_into(&mut packet) {
                std::process::abort();
            }
            packet.unwrap_or_else(|| std::process::abort())
        })
        .map_err(|_| Error::Capacity)?;
        let mut buffers = [None; GFX942_MAX_FIXED_DISPATCH_DATA_V1];
        buffers[0] = Some(GeneratedBufferSlotV1 {
            ordinal: 0,
            bytes: offset,
            access: Gfx942RuntimeBufferAccessV1::WriteOnly,
        });
        let result = Self {
            members,
            originals,
            packets: Some(packets),
            metadata: metadata.clone(),
            roster: GeneratedHostRosterV1 {
                source_identity: if independent {
                    GeneratedSourceIdentityV1::IndependentArena1024(Arc::new(()))
                } else {
                    GeneratedSourceIdentityV1::Arena1024(Arc::new(()))
                },
                buffers,
                count: 1,
                readback_bytes: offset,
                fixup_count: 1,
                dispatch_contract_sha256: if independent {
                    GeneratedContractsV1::IndependentArena1024(hash.finalize().into())
                } else {
                    GeneratedContractsV1::Arena1024(hash.finalize().into())
                },
            },
        };
        result
            .validate_sources(device_unique_id)
            .map_err(Error::Source)?;
        Ok(result)
    }

    pub(crate) fn metadata(&self) -> &ResourceCreditAccountV1 {
        &self.metadata
    }

    pub(crate) fn validate_sources(
        &self,
        uid: u64,
    ) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
        for (member, original) in self.members.iter().zip(self.originals.iter()) {
            let source = member
                .as_ref()
                .unwrap_or_else(|| std::process::abort())
                .source();
            if !original
                .as_ref()
                .unwrap_or_else(|| std::process::abort())
                .matches(&source.validate(uid)?)
            {
                return Err(RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster);
            }
        }
        Ok(self.roster.clone())
    }

    pub(crate) fn revalidate_sources(
        &self,
    ) -> Result<(), RuntimeGfx942GeneratedReservationErrorV1> {
        for member in self.members.iter().flatten() {
            member.source().revalidate()?;
        }
        Ok(())
    }

    pub(crate) fn matches_member(&self, index: usize, actual: &GeneratedHostRosterV1) -> bool {
        self.originals
            .get(index)
            .and_then(Option::as_ref)
            .is_some_and(|original| original.matches(actual))
    }

    pub(crate) fn member_original(&self, index: usize) -> Option<Original> {
        self.originals.get(index).and_then(Option::as_ref).cloned()
    }

    pub(crate) fn original_roster(&self) -> &GeneratedHostRosterV1 {
        &self.roster
    }
}
