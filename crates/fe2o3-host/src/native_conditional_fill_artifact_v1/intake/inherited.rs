//! Native-only, one-shot startup slot ownership. Environment bytes are not authority.
//! Value copying is fixed-capacity. libc environment lookup/removal is a separate
//! startup substrate, not an operation bounded by the canonical work account.

use super::*;
use crate::application_descriptor_handoff::{
    claim_inherited_descriptor, close_available_handoff_descriptors, valid_application_descriptor,
};
use fe2o3_runtime_protocol::{
    MAX_WORKER_V3_APPLICATION_OCCURRENCE_BYTES_V1 as MAX_OCCURRENCE,
    WORKER_V3_APPLICATION_ARTIFACT_DIR_FD_ENV_V1 as DIRECTORY,
    WORKER_V3_APPLICATION_ENVELOPE_FD_ENV_V1 as ENVELOPE,
    WORKER_V3_APPLICATION_HANDOFF_ACK_FD_ENV_V1 as ACK,
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_BYTES_V1 as CHALLENGE_BYTES,
    WORKER_V3_APPLICATION_HANDOFF_CHALLENGE_ENV_V1 as CHALLENGE,
    WORKER_V3_APPLICATION_HANDOFF_COMMITMENT_ENV_V1 as COMMITMENT,
    WORKER_V3_APPLICATION_OCCURRENCE_ENV_V1 as OCCURRENCE,
    WORKER_V3_APPLICATION_PROOF_FD_ENV_V1 as PROOF,
};
use std::{
    os::fd::RawFd,
    sync::atomic::{AtomicBool, Ordering},
};

const MARKER: &str = "FE2O3_NATIVE_CONDITIONAL_APPLICATION_V1";
const SLOTS: [&str; 4] = [ENVELOPE, DIRECTORY, ACK, PROOF];
const FORBIDDEN: [&str; 6] = [
    COMMITMENT,
    "FE2O3_APPLICATION_ENVELOPE_FD_V1",
    "FE2O3_APPLICATION_ARTIFACT_DIR_FD_V1",
    "FE2O3_APPLICATION_ENVELOPE_COMMITMENT_V1",
    "FE2O3_APPLICATION_HANDOFF_ACK_FD_V1",
    "FE2O3_APPLICATION_HANDOFF_CHALLENGE_V1",
];
static CLAIMED: AtomicBool = AtomicBool::new(false);

struct Field<const N: usize> {
    bytes: [u8; N],
    len: Option<usize>,
    oversized: bool,
}

impl<const N: usize> Field<N> {
    fn missing() -> Self {
        Self {
            bytes: [0; N],
            len: None,
            oversized: false,
        }
    }

    fn bytes(&self) -> Result<&[u8]> {
        require(!self.oversized, "oversized native environment field")?;
        self.len
            .map(|n| &self.bytes[..n])
            .ok_or_else(|| failure("missing native environment field"))
    }

    // The caller supplies the process-startup no-mutation guarantee. Read at most
    // N+1 bytes, stopping at the first NUL, before any environment mutation.
    #[allow(unsafe_code)]
    unsafe fn capture(name: &'static str) -> Self {
        let mut result = Self::missing();
        let mut key = [0u8; 96];
        assert!(name.len() < key.len());
        key[..name.len()].copy_from_slice(name.as_bytes());
        // SAFETY: key is NUL-terminated; the inherited environment is immutable
        // during this bounded startup snapshot under the entrypoint contract.
        let pointer = unsafe { libc::getenv(key.as_ptr().cast()) }.cast::<u8>();
        if pointer.is_null() {
            return result;
        }
        for index in 0..=N {
            // SAFETY: getenv returns a NUL-terminated allocation. Stop at its
            // first NUL, never reading beyond it, and never retain this pointer.
            let byte = unsafe { pointer.add(index).read() };
            if byte == 0 {
                result.len = Some(index);
                return result;
            }
            if index == N {
                result.len = Some(N);
                result.oversized = true;
                return result;
            }
            result.bytes[index] = byte;
        }
        unreachable!()
    }
}

struct Snapshot {
    marker: Field<1>,
    slots: [Field<10>; 4],
    occurrence: Field<{ MAX_OCCURRENCE * 2 }>,
    challenge: Field<{ CHALLENGE_BYTES * 2 }>,
    forbidden: [bool; FORBIDDEN.len()],
}

impl Snapshot {
    #[allow(unsafe_code)]
    unsafe fn take() -> Self {
        // SAFETY: the public entrypoint requires exclusive startup access to the
        // environment and inherited descriptors until admission returns.
        let snapshot = unsafe {
            Self {
                marker: Field::capture(MARKER),
                slots: SLOTS.map(|name| Field::capture(name)),
                occurrence: Field::capture(OCCURRENCE),
                challenge: Field::capture(CHALLENGE),
                forbidden: FORBIDDEN.map(|name| Field::<0>::capture(name).len.is_some()),
            }
        };
        for name in [MARKER, OCCURRENCE, CHALLENGE]
            .into_iter()
            .chain(SLOTS)
            .chain(FORBIDDEN)
        {
            // SAFETY: no other thread, signal handler or descendant accesses
            // the process environment under the explicit startup contract.
            unsafe { std::env::remove_var(name) };
        }
        snapshot
    }
}

struct OriginalSlots([Option<RawFd>; 4]);

impl Drop for OriginalSlots {
    fn drop(&mut self) {
        close_available_handoff_descriptors(std::mem::take(&mut self.0));
    }
}

impl OriginalSlots {
    fn claim(mut self) -> Result<[OwnedFd; 4]> {
        let raw = self
            .0
            .map(|n| n.ok_or_else(|| failure("native descriptor number")));
        let [envelope, directory, ack, proof] = raw;
        let raw = [envelope?, directory?, ack?, proof?];
        Descriptors::new(raw[0], raw[1], raw[2], raw[3]).map_err(failure)?;
        let mut take = |index: usize| {
            let descriptor = self.0[index]
                .take()
                .ok_or_else(|| failure("native descriptor already claimed"))?;
            claim_inherited_descriptor(descriptor, "native application input").map_err(failure)
        };
        Ok([take(0)?, take(1)?, take(2)?, take(3)?])
    }
}

fn descriptor(field: &Field<10>) -> Option<RawFd> {
    let bytes = field.bytes().ok()?;
    if !bytes.first().is_some_and(|b| matches!(b, b'1'..=b'9'))
        || !bytes.iter().all(u8::is_ascii_digit)
    {
        return None;
    }
    std::str::from_utf8(bytes)
        .ok()?
        .parse::<RawFd>()
        .ok()
        .filter(|fd| valid_application_descriptor(*fd))
}

fn decode_hex<const N: usize>(field: &[u8], output: &mut [u8; N]) -> Result<usize> {
    require(
        !field.is_empty() && field.len().is_multiple_of(2) && field.len() / 2 <= N,
        "native environment wire length",
    )?;
    let nibble = |b| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err(failure("noncanonical native environment hex")),
    };
    for (index, pair) in field.chunks_exact(2).enumerate() {
        output[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(field.len() / 2)
}

struct ClaimedInputs {
    descriptors: [OwnedFd; 4],
    occurrence: Occurrence,
    challenge: Challenge,
    scratch: usize,
}

fn claim_snapshot(snapshot: Snapshot, budget: &mut Budget<'_>) -> Result<ClaimedInputs> {
    // Cleanup of the finite transferred roster must not depend on an available
    // caller budget. It creates no authority and never includes reserved FD195.
    let slots = OriginalSlots(snapshot.slots.each_ref().map(descriptor));
    require(snapshot.marker.bytes()? == b"1", "native handoff marker")?;
    require(
        !snapshot.forbidden.into_iter().any(|present| present),
        "mixed native/legacy application environment",
    )?;
    let descriptors = slots.claim()?;
    let scratch = size_of::<Snapshot>()
        + MAX_OCCURRENCE
        + CHALLENGE_BYTES
        + size_of::<ClaimedInputs>()
        + fe2o3_runtime_protocol::MAX_WORKER_V3_APPLICATION_HANDOFF_ALLOCATION_BYTES_V1;
    budget.charge_work(scratch)?;
    budget.reserve_storage(scratch)?;
    let mut occurrence_wire = [0u8; MAX_OCCURRENCE];
    let n = decode_hex(snapshot.occurrence.bytes()?, &mut occurrence_wire)?;
    let occurrence = Occurrence::decode_canonical(&occurrence_wire[..n]).map_err(failure)?;
    require(
        occurrence.inputs().len() == 4,
        "native four-input occurrence",
    )?;
    let mut challenge_wire = [0u8; CHALLENGE_BYTES];
    let n = decode_hex(snapshot.challenge.bytes()?, &mut challenge_wire)?;
    require(n == CHALLENGE_BYTES, "native challenge exact length")?;
    let challenge = Challenge::decode_canonical(&challenge_wire).map_err(failure)?;
    Ok(ClaimedInputs {
        descriptors,
        occurrence,
        challenge,
        scratch,
    })
}

impl<'work> PreparedNativeConditionalFillApplicationV1<'work> {
    /// Consume the native Cargo runner's original four inherited descriptors.
    /// Requires marker `FE2O3_NATIVE_CONDITIONAL_APPLICATION_V1=1`, canonical
    /// descriptor/occurrence/challenge variables, and no legacy handoff fields.
    /// All recognized variables are removed exactly once. Refusal closes every
    /// distinct valid declared input; reserved service FD195 remains untouched.
    ///
    /// Environment fields authenticate nothing. Actual independent deployment,
    /// publication, source, occurrence and later native custody checks still run.
    /// The returned logical owner charge is unreserved, as in `admit_descriptors`.
    /// libc's environment lookup/removal is outside that canonical accounting;
    /// Cargo's native runner supplies an env-cleared, fixed application roster.
    ///
    /// # Safety
    /// Call exactly once at cooperative application startup, before creating any
    /// other thread or descendant. No signal handler may read or mutate the
    /// environment or descriptor table. The four declared slots must be exclusively
    /// transferred raw descriptors with no other Rust owner; no concurrent owner
    /// may close, reuse, duplicate or mutate them until this call returns. This
    /// function does not claim FD195 or relax its separate unsafe admission rules.
    #[allow(unsafe_code)]
    pub unsafe fn admit_inherited(
        producer: &ProducerIdentity,
        compiler: &Compiler<'work>,
        profile: &Profile<'work>,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, NativeConditionalFillIntakeStorageV1)> {
        require(
            !CLAIMED.swap(true, Ordering::AcqRel),
            "native handoff already claimed",
        )?;
        // SAFETY: forwarded from this function's cooperative startup contract.
        let snapshot = unsafe { Snapshot::take() };
        let ClaimedInputs {
            descriptors,
            occurrence,
            challenge,
            scratch,
        } = claim_snapshot(snapshot, budget)?;
        let result = Self::admit_descriptors(
            descriptors,
            occurrence,
            challenge,
            producer,
            compiler,
            profile,
            budget,
        )?;
        budget.release_storage(scratch)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests;
