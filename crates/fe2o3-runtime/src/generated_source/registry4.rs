//! Four retained originals, distinct from both scalar and whole-cohort identity.

use super::*;
use std::sync::Arc;
mod inputs;

/// Inert original carrier roster for the closed, single-use four-recipe registry.
/// It is neither a scalar carrier nor a Cohort3 owner. The native queue and every
/// original source remain retained until actual common backing destruction.
///
/// ```compile_fail
/// use fe2o3_runtime::{RuntimeGfx942GeneratedRegistry4V1, RuntimeGfx942GeneratedCarrierV1};
/// fn scalar<T: RuntimeGfx942GeneratedCarrierV1>(_: T) {}
/// fn reject<P>(originals: [P; 4]) { scalar(RuntimeGfx942GeneratedRegistry4V1::new(originals)); }
/// ```
#[must_use]
pub struct RuntimeGfx942GeneratedResidentRegistryV1<P, const N: usize> {
    pub(crate) members: [P; N],
    pub(crate) repeat2: bool,
}

/// Existing four-original profile, unchanged by the distinct N16 entrypoint.
pub type RuntimeGfx942GeneratedRegistry4V1<P> = RuntimeGfx942GeneratedResidentRegistryV1<P, 4>;

impl<P, const N: usize> RuntimeGfx942GeneratedResidentRegistryV1<P, N> {
    pub const fn new(members: [P; N]) -> Self {
        Self {
            members,
            repeat2: false,
        }
    }
}

/// Sixteen original carriers with separate result gates and native receipts.
/// Construction is inert; it supplies no authority, graph completion or OOD proof.
#[must_use]
pub struct RuntimeGfx942GeneratedRegistry16V1<P>(
    pub(crate) RuntimeGfx942GeneratedResidentRegistryV1<P, 16>,
);
impl<P: crate::RuntimeGfx942RegistryCompletionCarrierV1> RuntimeGfx942GeneratedRegistry16V1<P> {
    pub const fn new(members: [P; 16]) -> Self {
        Self(RuntimeGfx942GeneratedResidentRegistryV1::new(members))
    }
}

/// The same four original sources with two independently prepaid result frames.
/// This distinct inert profile permits at most eight publications and never
/// converts copied-result readiness into common native retirement.
#[must_use]
pub struct RuntimeGfx942GeneratedRegistry4Repeat2V1<P>(
    pub(crate) RuntimeGfx942GeneratedRegistry4V1<P>,
);

impl<P: crate::RuntimeGfx942RegistryRepeat2CarrierV1> RuntimeGfx942GeneratedRegistry4Repeat2V1<P> {
    pub const fn new(members: [P; 4]) -> Self {
        Self(RuntimeGfx942GeneratedRegistry4V1 {
            members,
            repeat2: true,
        })
    }
}

impl<P: RuntimeGfx942GeneratedCarrierV1, const N: usize>
    RuntimeGfx942GeneratedResidentRegistryV1<P, N>
{
    pub(crate) fn source_mut_v1(
        &mut self,
    ) -> Option<RuntimeGfx942Registry4SourceMutV1<'_, P::CurrentnessError, N>> {
        let mut sources = [const { None }; N];
        for (target, member) in sources.iter_mut().zip(&mut self.members) {
            *target = Some(member.source_mut()?);
        }
        Some(RuntimeGfx942Registry4SourceMutV1 {
            repeat2: self.repeat2,
            sources: all_some(sources)?,
        })
    }

    pub(crate) fn validate_sources(
        &self,
        uid: u64,
    ) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
        let sources = self.members.each_ref().map(|member| member.source());
        for source in &sources {
            if !matches!(
                source.projection.invocation_binding(),
                crate::Gfx942RuntimeInvocationBindingV1::NativeConditionalFill64V1 { .. }
            ) {
                return Err(RuntimeGfx942GeneratedReservationErrorV1::AuthorityMismatch);
            }
        }
        combine(
            validate_all(&sources, |source| source.validate(uid))?,
            self.repeat2,
        )
    }

    pub(crate) fn with_current_sources_v1<F>(
        &self,
        uid: u64,
        expected: &GeneratedHostRosterV1,
        callback: impl FnOnce() -> Result<(), F>,
    ) -> Result<Result<(), F>, RuntimeGfx942GeneratedReservationErrorV1> {
        if !self.validate_sources(uid)?.matches(expected) {
            return Err(RuntimeGfx942GeneratedReservationErrorV1::InvalidRoster);
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
        callback: impl for<'a> FnOnce(
            [fe2o3_amdhsa_loader::ValidatedKernelEnvelope<'a>; N],
            [&'a [crate::Gfx942KfdDispatchBufferV1]; N],
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
        inputs::lend(
            self.members.each_ref().map(|member| member.source()),
            uid,
            callback,
        )
    }
}

pub(crate) struct RuntimeGfx942Registry4SourceMutV1<'a, E, const N: usize = 4> {
    sources: [RuntimeGfx942GeneratedSourceMutV1<'a, E>; N],
    repeat2: bool,
}

impl<E, const N: usize> RuntimeGfx942Registry4SourceMutV1<'_, E, N> {
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
        combine(
            validate_all(&self.sources, |source| source.validate(uid))?,
            self.repeat2,
        )
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
        if originals.iter().any(Result::is_err) {
            return false;
        }
        let originals = originals.map(|item| item.unwrap_or_else(|_| std::process::abort()));
        combine(originals, self.repeat2).is_ok_and(|actual| actual.matches(expected))
    }

    pub(crate) fn transfer_controls_into(
        &mut self,
        destinations: &mut [Option<fe2o3_kfd::Gfx942FixedDispatchPacketV1>],
    ) -> bool {
        if destinations.len() != N
            || destinations.iter().any(Option::is_some)
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

fn combine<const N: usize>(
    originals: [GeneratedHostRosterV1; N],
    repeat2: bool,
) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
    use RuntimeGfx942GeneratedReservationErrorV1 as Error;
    if !(N == 4 || (N == 16 && !repeat2)) {
        return Err(Error::InvalidRoster);
    }
    let mut identities: [Option<Arc<()>>; N] = [const { None }; N];
    let mut contracts = [[0; 32]; N];
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
    let identities = all_some(identities).ok_or(Error::InvalidRoster)?;
    let (identity, contracts) = if N == 16 {
        (
            GeneratedSourceIdentityV1::Registry16(exact_array(identities)),
            GeneratedContractsV1::Registry16(exact_array(contracts)),
        )
    } else if repeat2 {
        (
            GeneratedSourceIdentityV1::Registry4Repeat2(exact_array(identities)),
            GeneratedContractsV1::Registry4Repeat2(exact_array(contracts)),
        )
    } else {
        (
            GeneratedSourceIdentityV1::Registry4(exact_array(identities)),
            GeneratedContractsV1::Registry4(exact_array(contracts)),
        )
    };
    Ok(GeneratedHostRosterV1 {
        source_identity: identity,
        buffers,
        count: N,
        readback_bytes,
        fixup_count: N,
        dispatch_contract_sha256: contracts,
    })
}

fn all_some<T, const N: usize>(items: [Option<T>; N]) -> Option<[T; N]> {
    if items.iter().any(Option::is_none) {
        return None;
    }
    Some(items.map(|item| item.unwrap_or_else(|| std::process::abort())))
}

fn validate_all<T, U, E, const N: usize>(
    items: &[T; N],
    mut validate: impl FnMut(&T) -> Result<U, E>,
) -> Result<[U; N], E> {
    let mut checked = [const { None }; N];
    for (target, item) in checked.iter_mut().zip(items) {
        *target = Some(validate(item)?);
    }
    Ok(checked.map(|item| item.unwrap_or_else(|| std::process::abort())))
}

fn exact_array<T, const IN: usize, const OUT: usize>(items: [T; IN]) -> [T; OUT] {
    if IN != OUT {
        std::process::abort();
    }
    let mut items = items.into_iter();
    core::array::from_fn(|_| items.next().unwrap_or_else(|| std::process::abort()))
}
