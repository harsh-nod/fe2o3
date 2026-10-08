//! Exact three-original composition. These identities remain descriptive.

use super::*;
use std::sync::Arc;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneratedProfileV1 {
    Singleton,
    IndependentArenaMember,
    NativeFillCohort3,
    NativeFillRegistry4,
    NativeFillRegistry4Repeat2,
    NativeFillRegistry16,
    NativeFillArena1024,
    IndependentFillArena1024,
    IndependentFillArena2048,
}

impl GeneratedProfileV1 {
    pub(crate) fn arena_slots(self) -> Option<usize> {
        match self {
            Self::NativeFillArena1024 | Self::IndependentFillArena1024 => Some(1024),
            Self::IndependentFillArena2048 => Some(2048),
            _ => None,
        }
    }

    pub(crate) fn independent_arena(self) -> bool {
        matches!(
            self,
            Self::IndependentFillArena1024 | Self::IndependentFillArena2048
        )
    }
}

#[derive(Clone)]
pub(crate) enum GeneratedSourceIdentityV1 {
    Singleton(Arc<()>),
    IndependentArenaMember(Arc<()>),
    Cohort3([Arc<()>; 3]),
    Registry4([Arc<()>; 4]),
    Registry4Repeat2([Arc<()>; 4]),
    Registry16([Arc<()>; 16]),
    Arena1024(Arc<()>),
    IndependentArena1024(Arc<()>),
    IndependentArena2048(Arc<()>),
}

impl From<Arc<()>> for GeneratedSourceIdentityV1 {
    fn from(value: Arc<()>) -> Self {
        Self::Singleton(value)
    }
}

impl GeneratedSourceIdentityV1 {
    pub(crate) fn profile(&self) -> GeneratedProfileV1 {
        match self {
            Self::Singleton(_) => GeneratedProfileV1::Singleton,
            Self::IndependentArenaMember(_) => GeneratedProfileV1::IndependentArenaMember,
            Self::Cohort3(_) => GeneratedProfileV1::NativeFillCohort3,
            Self::Registry4(_) => GeneratedProfileV1::NativeFillRegistry4,
            Self::Registry4Repeat2(_) => GeneratedProfileV1::NativeFillRegistry4Repeat2,
            Self::Registry16(_) => GeneratedProfileV1::NativeFillRegistry16,
            Self::Arena1024(_) => GeneratedProfileV1::NativeFillArena1024,
            Self::IndependentArena1024(_) => GeneratedProfileV1::IndependentFillArena1024,
            Self::IndependentArena2048(_) => GeneratedProfileV1::IndependentFillArena2048,
        }
    }
    #[cfg(test)]
    pub(crate) fn singleton_for_test(&self) -> &Arc<()> {
        let Self::Singleton(value) = self else {
            panic!("singleton fixture")
        };
        value
    }

    pub(crate) fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Singleton(a), Self::Singleton(b)) => Arc::ptr_eq(a, b),
            (Self::IndependentArenaMember(a), Self::IndependentArenaMember(b)) => Arc::ptr_eq(a, b),
            (Self::Cohort3(a), Self::Cohort3(b)) => a.iter().zip(b).all(|(a, b)| Arc::ptr_eq(a, b)),
            (Self::Registry4(a), Self::Registry4(b))
            | (Self::Registry4Repeat2(a), Self::Registry4Repeat2(b)) => {
                a.iter().zip(b).all(|(a, b)| Arc::ptr_eq(a, b))
            }
            (Self::Registry16(a), Self::Registry16(b)) => {
                a.iter().zip(b).all(|(a, b)| Arc::ptr_eq(a, b))
            }
            (Self::Arena1024(a), Self::Arena1024(b)) => Arc::ptr_eq(a, b),
            (Self::IndependentArena1024(a), Self::IndependentArena1024(b)) => Arc::ptr_eq(a, b),
            (Self::IndependentArena2048(a), Self::IndependentArena2048(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    pub(crate) fn matches_single(&self, other: &Arc<()>) -> bool {
        matches!(self, Self::Singleton(value) if Arc::ptr_eq(value, other))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Keep the bounded contract roster Copy; a boxed variant adds fallible storage.
#[allow(clippy::large_enum_variant)]
pub(crate) enum GeneratedContractsV1 {
    Singleton([u8; 32]),
    IndependentArenaMember([u8; 32]),
    Cohort3([[u8; 32]; 3]),
    Registry4([[u8; 32]; 4]),
    Registry4Repeat2([[u8; 32]; 4]),
    Registry16([[u8; 32]; 16]),
    Arena1024([u8; 32]),
    IndependentArena1024([u8; 32]),
    IndependentArena2048([u8; 32]),
}

impl From<[u8; 32]> for GeneratedContractsV1 {
    fn from(value: [u8; 32]) -> Self {
        Self::Singleton(value)
    }
}

impl GeneratedContractsV1 {
    #[cfg(test)]
    pub(crate) fn singleton_for_test(&self) -> &[u8; 32] {
        let Self::Singleton(value) = self else {
            panic!("singleton fixture")
        };
        value
    }

    #[cfg(test)]
    pub(crate) fn singleton_mut_for_test(&mut self) -> &mut [u8; 32] {
        let Self::Singleton(value) = self else {
            panic!("singleton fixture")
        };
        value
    }

    #[cfg(feature = "hardware-qualification")]
    pub(crate) fn update_qualification_hash(self, hash: &mut Sha256) {
        match self {
            Self::Singleton(value) => hash.update(value),
            Self::IndependentArenaMember(value) => {
                hash.update(b"fe2o3.generated.independent-fill-arena.member.v1\0");
                hash.update(value);
            }
            Self::Cohort3(values) => {
                hash.update(b"fe2o3.generated.native-fill-cohort3.contracts.v1\0");
                for value in values {
                    hash.update(value);
                }
            }
            Self::Registry4(values) => {
                hash.update(b"fe2o3.generated.native-fill-registry4.contracts.v1\0");
                for value in values {
                    hash.update(value);
                }
            }
            Self::Registry4Repeat2(values) => {
                hash.update(b"fe2o3.generated.native-fill-registry4-repeat2.contracts.v1\0");
                for value in values {
                    hash.update(value);
                }
            }
            Self::Registry16(values) => {
                hash.update(b"fe2o3.generated.native-fill-registry16.contracts.v1\0");
                for value in values {
                    hash.update(value);
                }
            }
            Self::Arena1024(value) => {
                hash.update(b"fe2o3.generated.native-fill-arena1024.commitment.v1\0");
                hash.update(value);
            }
            Self::IndependentArena1024(value) => {
                hash.update(b"fe2o3.generated.independent-fill-arena1024.commitment.v1\0");
                hash.update(value);
            }
            Self::IndependentArena2048(value) => {
                hash.update(b"fe2o3.generated.independent-fill-arena2048.commitment.v1\0");
                hash.update(value);
            }
        }
    }
}

/// Three original generated carriers, not a singleton carrier or a launch permit.
///
/// Construction is inert. The runtime must independently admit all original
/// sources, currentness owners and result gates before any native effect. There
/// is no per-member native receipt: publication and retirement are whole-cohort.
///
/// ```compile_fail
/// use fe2o3_runtime::{RuntimeGfx942GeneratedCarrierV1, RuntimeGfx942GeneratedCohort3V1};
/// fn scalar<T: RuntimeGfx942GeneratedCarrierV1>(_: T) {}
/// fn reject<P: RuntimeGfx942GeneratedCarrierV1>(members: [P; 3]) {
///     scalar(RuntimeGfx942GeneratedCohort3V1::new(members));
/// }
/// ```
#[must_use]
pub struct RuntimeGfx942GeneratedCohort3V1<P> {
    pub(crate) members: [P; 3],
}

impl<P> RuntimeGfx942GeneratedCohort3V1<P> {
    pub const fn new(members: [P; 3]) -> Self {
        Self { members }
    }
}

impl<P: RuntimeGfx942GeneratedCarrierV1> RuntimeGfx942GeneratedCohort3V1<P> {
    pub(crate) fn source_mut_v1(
        &mut self,
    ) -> Option<RuntimeGfx942Cohort3SourceMutV1<'_, P::CurrentnessError>> {
        let [a, b, c] = self.members.each_mut();
        Some(RuntimeGfx942Cohort3SourceMutV1 {
            sources: [a.source_mut()?, b.source_mut()?, c.source_mut()?],
        })
    }

    pub(crate) fn with_current_sources_v1<F>(
        &self,
        uid: u64,
        expected: &GeneratedHostRosterV1,
        callback: impl FnOnce() -> Result<(), F>,
    ) -> Result<Result<(), F>, RuntimeGfx942GeneratedReservationErrorV1> {
        use RuntimeGfx942GeneratedReservationErrorV1 as Error;
        if !self.validate_sources(uid)?.matches(expected) {
            return Err(Error::InvalidRoster);
        }
        for member in &self.members {
            member.source().revalidate()?;
        }
        let result = callback();
        for member in &self.members {
            member.source().revalidate()?;
        }
        Ok(result)
    }

    pub(crate) fn with_native_inputs_v1(
        &self,
        uid: u64,
        expected: &GeneratedHostRosterV1,
        callback: impl for<'b> FnOnce(
            [fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'b>; 3],
            [&'b [crate::Gfx942KfdDispatchBufferV1]; 3],
        ) -> Result<
            (),
            crate::RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
        >,
    ) -> Result<
        Result<(), crate::RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
        GeneratedNativeInputErrorV1,
    > {
        use GeneratedNativeInputErrorV1 as Error;
        if !self
            .validate_sources(uid)
            .map_err(Error::Source)?
            .matches(expected)
        {
            return Err(Error::Source(
                RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster,
            ));
        }
        let [a, b, c] = self.members.each_ref().map(|member| member.source());
        let ar = a.validate(uid).map_err(Error::Source)?;
        let br = b.validate(uid).map_err(Error::Source)?;
        let cr = c.validate(uid).map_err(Error::Source)?;
        let mut nested_error = None;
        // Each earlier envelope stays borrowed across every later callback.
        // No borrowed input or native owner can escape the `()` boundary.
        let result = a.with_native_inputs_v1(uid, &ar, |ap, ab| {
            let result = b.with_native_inputs_v1(uid, &br, |bp, bb| {
                match c
                    .with_native_inputs_v1(uid, &cr, |cp, cb| callback([ap, bp, cp], [ab, bb, cb]))
                {
                    Ok(result) => result,
                    Err(error) => {
                        nested_error = Some(error);
                        Ok(())
                    }
                }
            });
            match result {
                Ok(result) => result,
                Err(error) => {
                    nested_error = Some(error);
                    Ok(())
                }
            }
        })?;
        match nested_error {
            Some(error) => Err(error),
            None => Ok(result),
        }
    }

    pub(crate) fn validate_sources(
        &self,
        uid: u64,
    ) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
        use RuntimeGfx942GeneratedReservationErrorV1 as Error;
        let sources = self.members.each_ref().map(|member| member.source());
        for source in &sources {
            if !matches!(
                source.projection.invocation_binding(),
                crate::Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 { .. }
            ) {
                return Err(Error::AuthorityMismatch);
            }
        }
        let originals = [
            sources[0].validate(uid)?,
            sources[1].validate(uid)?,
            sources[2].validate(uid)?,
        ];
        combine_original_rosters(originals)
    }
}

pub(crate) struct RuntimeGfx942Cohort3SourceMutV1<'a, E> {
    sources: [RuntimeGfx942GeneratedSourceMutV1<'a, E>; 3],
}

impl<E> RuntimeGfx942Cohort3SourceMutV1<'_, E> {
    pub(crate) fn validate(
        &self,
        uid: u64,
    ) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
        for source in &self.sources {
            if !matches!(
                source.storage.data().invocation_binding(),
                crate::Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 { .. }
            ) {
                return Err(RuntimeGfx942GeneratedReservationErrorV1::AuthorityMismatch);
            }
        }
        combine_original_rosters([
            self.sources[0].validate(uid)?,
            self.sources[1].validate(uid)?,
            self.sources[2].validate(uid)?,
        ])
    }

    pub(crate) fn matches_roster(&self, expected: &GeneratedHostRosterV1) -> bool {
        if self.sources.iter().any(|source| {
            !source.storage.control_available()
                || !matches!(
                    source.storage.data().invocation_binding(),
                    crate::Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 { .. }
                )
        }) {
            return false;
        }
        let originals = self
            .sources
            .each_ref()
            .map(|source| GeneratedHostRosterV1::from_data(source.storage.data()));
        let [Ok(a), Ok(b), Ok(c)] = originals else {
            return false;
        };
        combine_original_rosters([a, b, c]).is_ok_and(|actual| actual.matches(expected))
    }

    pub(crate) fn transfer_controls_into(
        &mut self,
        destinations: &mut [Option<fe2o3_kfd::Gfx942FixedDispatchPacketV1>; 3],
    ) -> bool {
        if destinations.iter().any(Option::is_some)
            || self
                .sources
                .iter()
                .any(|source| !source.storage.control_available())
        {
            return false;
        }
        for (source, destination) in self.sources.iter_mut().zip(destinations) {
            if !source.transfer_control_into(destination) {
                return false;
            }
        }
        true
    }
}

pub(crate) fn combine_original_rosters(
    originals: [GeneratedHostRosterV1; 3],
) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
    use RuntimeGfx942GeneratedReservationErrorV1 as Error;
    let mut identities: [Option<Arc<()>>; 3] = [None, None, None];
    let mut contracts = [[0; 32]; 3];
    let mut buffers = [None; GFX942_MAX_FIXED_DISPATCH_DATA_V1];
    let mut readback_bytes = 0_u64;
    for (index, original) in originals.into_iter().enumerate() {
        let Some(slot) = original.buffers[0] else {
            return Err(Error::InvalidRoster);
        };
        if original.count != 1
            || original.fixup_count != 1
            || slot.ordinal != 0
            || slot.access != Gfx942RuntimeBufferAccessV1::WriteOnly
            || original.buffers[1..].iter().any(Option::is_some)
        {
            return Err(Error::InvalidRoster);
        }
        let GeneratedSourceIdentityV1::Singleton(identity) = original.source_identity else {
            return Err(Error::InvalidRoster);
        };
        if identities
            .iter()
            .flatten()
            .any(|prior| Arc::ptr_eq(prior, &identity))
        {
            return Err(Error::InvalidRoster);
        }
        let GeneratedContractsV1::Singleton(contract) = original.dispatch_contract_sha256 else {
            return Err(Error::InvalidRoster);
        };
        contracts[index] = contract;
        identities[index] = Some(identity);
        buffers[index] = Some(GeneratedBufferSlotV1 {
            ordinal: index,
            ..slot
        });
        readback_bytes = readback_bytes
            .checked_add(original.readback_bytes)
            .ok_or(Error::InvalidRoster)?;
    }
    // Exactly three inputs were checked before their inert identity moves.
    let [a, b, c] = identities;
    let identities = [
        a.ok_or(Error::InvalidRoster)?,
        b.ok_or(Error::InvalidRoster)?,
        c.ok_or(Error::InvalidRoster)?,
    ];
    Ok(GeneratedHostRosterV1 {
        source_identity: GeneratedSourceIdentityV1::Cohort3(identities),
        buffers,
        count: 3,
        readback_bytes,
        fixup_count: 3,
        dispatch_contract_sha256: GeneratedContractsV1::Cohort3(contracts),
    })
}
