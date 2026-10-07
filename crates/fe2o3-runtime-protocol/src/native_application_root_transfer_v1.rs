//! Fixed inert registration transfer frames; OS sender/custody checks are separate.
use crate::{
    NATIVE_APPLICATION_REGISTRATION_BYTES_V1, NativeApplicationRegistrationBindingV1 as Binding,
};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionServiceReadyErrorV3 as ReadyError, CompilerExecutionServiceReadyV3 as Ready,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, mem::size_of};

const MAGIC: &[u8; 8] = b"F3NART1\0";
const NONCE_MAGIC: &[u8; 8] = b"F3NARN1\0";
const COMPLETE_MAGIC: &[u8; 8] = b"F3NARC1\0";
const PUBLICATION_MAGIC: &[u8; 8] = b"F3NARP1\0";
const PUBLISHED_MAGIC: &[u8; 8] = b"F3NARA1\0";
const ACK_READER_MAGIC: &[u8; 8] = b"F3NAAR1\0";
const ACK_CLOSED_MAGIC: &[u8; 8] = b"F3NAAC1\0";
const ROOT_ACK_READER_MAGIC: &[u8; 8] = b"F3NRAR1\0";
const SUPERVISOR_GATE_MAGIC: &[u8; 8] = b"F3NASG1\0";
pub const NATIVE_APPLICATION_ROOT_NONCE_BYTES_V1: usize = 40;
pub const NATIVE_APPLICATION_ROOT_TRANSFER_BYTES_V1: usize =
    48 + NATIVE_APPLICATION_REGISTRATION_BYTES_V1;
const BYTES: usize = NATIVE_APPLICATION_ROOT_TRANSFER_BYTES_V1;

/// Exactly four ordered one-descriptor packets, never a substitute for the
/// original authenticated supervisor. Slot order is service/app/proof/Cargo.
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationRootTransferV1 {
    bytes: [u8; BYTES],
}
type Frame = NativeApplicationRootTransferV1;
#[derive(Debug)]
pub enum NativeApplicationRootTransferErrorV1 {
    Resource(Resource),
    Ready(ReadyError),
    Invalid,
}
type Error = NativeApplicationRootTransferErrorV1;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native root transfer: {self:?}")
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod gate_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[test]
    fn supervisor_gate_requires_exact_native_policy_deployment_and_nonzero_nonce() {
        let mut work = Work::new(1_000_000);
        let mut b = Budget::new(&mut work, 1_000_000);
        let packet = Frame::supervisor_gate_packet([1; 32], [2; 32], [3; 32], &mut b).unwrap();
        assert_eq!(
            Frame::check_supervisor_gate(&packet, [1; 32], [2; 32], &mut b).unwrap(),
            [3; 32]
        );
        assert!(Frame::check_supervisor_gate(&packet, [4; 32], [2; 32], &mut b).is_err());
        assert!(Frame::check_supervisor_gate(&packet, [1; 32], [4; 32], &mut b).is_err());
        for index in [0, 7, 8, 40] {
            let mut changed = packet;
            changed[index] ^= 1;
            assert!(Frame::check_supervisor_gate(&changed, [1; 32], [2; 32], &mut b).is_err());
        }
        let mut zero = packet;
        zero[72..].fill(0);
        assert!(Frame::check_supervisor_gate(&zero, [1; 32], [2; 32], &mut b).is_err());
        assert!(Frame::supervisor_gate_packet([0; 32], [2; 32], [3; 32], &mut b).is_err());
        assert!(Frame::supervisor_gate_packet([1; 32], [0; 32], [3; 32], &mut b).is_err());
        assert_eq!(b.storage(), 0);
    }
}

impl Frame {
    pub const WORK: usize = 8 + 4 * BYTES;
    pub const SCRATCH: usize = 4 * size_of::<Self>() + 4096;
    pub const STORAGE: usize = size_of::<Self>();
    /// Inert closed-role prefix; matching it never authenticates a root sender.
    pub const SUPERVISOR_GATE_PREFIX: [u8; 8] = *SUPERVISOR_GATE_MAGIC;

    /// Fixed application-supervisor selection packet, sent by the actual root
    /// before releasing its original child. The receiver must independently bind
    /// original-parent credentials, policy, deployment, socket and running image.
    pub fn supervisor_gate_packet(
        policy: [u8; 32],
        deployment: [u8; 32],
        nonce: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<[u8; 104]> {
        budget.with_prepaid_scope(0, 8, 512, 512, |_| {
            if policy == [0; 32] || deployment == [0; 32] || nonce == [0; 32] {
                return Err(Error::Invalid);
            }
            let mut packet = [0; 104];
            packet[..8].copy_from_slice(SUPERVISOR_GATE_MAGIC);
            packet[8..40].copy_from_slice(&policy);
            packet[40..72].copy_from_slice(&deployment);
            packet[72..].copy_from_slice(&nonce);
            Ok(packet)
        })
    }

    /// Checks only the exact inert gate framing and independently supplied native
    /// identities. The nonce is not child, parentage or service-role authority.
    pub fn check_supervisor_gate(
        packet: &[u8; 104],
        policy: [u8; 32],
        deployment: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<[u8; 32]> {
        budget.with_prepaid_scope(0, 8, 512, 512, |budget| {
            let nonce = packet[72..].try_into().map_err(|_| Error::Invalid)?;
            if packet != &Self::supervisor_gate_packet(policy, deployment, nonce, budget)? {
                return Err(Error::Invalid);
            }
            Ok(nonce)
        })
    }
    /// Original Cargo sends this exact frame with one ACK reader descriptor.
    pub fn ack_reader_packet(binding: [u8; 32], budget: &mut Budget<'_>) -> Result<[u8; 40]> {
        Self::ack_packet(ACK_READER_MAGIC, binding, budget)
    }
    /// Sent only after Cargo closes its sole original ACK reader alias.
    pub fn ack_reader_closed_packet(
        binding: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<[u8; 40]> {
        Self::ack_packet(ACK_CLOSED_MAGIC, binding, budget)
    }
    fn ack_packet(magic: &[u8; 8], binding: [u8; 32], budget: &mut Budget<'_>) -> Result<[u8; 40]> {
        budget.with_prepaid_scope(0, 8, 128, 128, |_| {
            if binding == [0; 32] {
                return Err(Error::Invalid);
            }
            let mut packet = [0; 40];
            packet[..8].copy_from_slice(magic);
            packet[8..].copy_from_slice(&binding);
            Ok(packet)
        })
    }
    /// The measured supervisor forwards the original reader on the root nonce.
    pub fn root_ack_reader_packet(
        nonce: [u8; 32],
        binding: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<[u8; 72]> {
        budget.with_prepaid_scope(0, 8, 256, 256, |_| {
            if nonce == [0; 32] || binding == [0; 32] {
                return Err(Error::Invalid);
            }
            let mut packet = [0; 72];
            packet[..8].copy_from_slice(ROOT_ACK_READER_MAGIC);
            packet[8..40].copy_from_slice(&nonce);
            packet[40..].copy_from_slice(&binding);
            Ok(packet)
        })
    }
    /// Inert exact root publication framing, not root/issuer authentication.
    pub fn publication_packet(
        ready: &Ready,
        nonce: [u8; 32],
        binding: [u8; 32],
        gate: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<[u8; 224]> {
        budget.with_prepaid_scope(ready.retained_storage(), 8, 1024, 4096, |_| {
            if nonce == [0; 32] || binding == [0; 32] || gate == [0; 32] {
                return Err(Error::Invalid);
            }
            let mut packet = [0; 224];
            packet[..8].copy_from_slice(PUBLICATION_MAGIC);
            packet[8..40].copy_from_slice(&nonce);
            packet[40..72].copy_from_slice(&binding);
            packet[72..104].copy_from_slice(&gate);
            packet[104..].copy_from_slice(ready.canonical_bytes());
            Ok(packet)
        })
    }
    /// Decode exact native Ready framing only. The return charge is unreserved;
    /// actual root SCM, original pidfd and retained native policy are separate.
    pub fn decode_publication(
        packet: &[u8; 224],
        nonce: [u8; 32],
        binding: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(Ready, [u8; 32], usize)> {
        budget.with_prepaid_scope(224, 8, 1024, 4096, |budget| {
            if nonce == [0; 32]
                || binding == [0; 32]
                || &packet[..8] != PUBLICATION_MAGIC
                || packet[8..40] != nonce
                || packet[40..72] != binding
                || packet[72..104] == [0; 32]
            {
                return Err(Error::Invalid);
            }
            let (ready, charge) = Ready::decode(&packet[104..], budget).map_err(Error::Ready)?;
            Ok((
                ready,
                packet[72..104].try_into().unwrap(),
                charge.additional_storage(),
            ))
        })
    }
    /// Inert completion sent only after the supervisor closes Cargo's control.
    pub fn publication_completion(
        nonce: [u8; 32],
        binding: [u8; 32],
        gate: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<[u8; 104]> {
        budget.with_prepaid_scope(0, 8, 512, 512, |_| {
            if nonce == [0; 32] || binding == [0; 32] || gate == [0; 32] {
                return Err(Error::Invalid);
            }
            let mut packet = [0; 104];
            packet[..8].copy_from_slice(PUBLISHED_MAGIC);
            packet[8..40].copy_from_slice(&nonce);
            packet[40..72].copy_from_slice(&binding);
            packet[72..].copy_from_slice(&gate);
            Ok(packet)
        })
    }
    pub fn completion_packet(
        nonce: [u8; 32],
        binding: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<[u8; 72]> {
        budget.with_prepaid_scope(0, 8, 256, 256, |_| {
            if nonce == [0; 32] || binding == [0; 32] {
                return Err(Error::Invalid);
            }
            let mut packet = [0; 72];
            packet[..8].copy_from_slice(COMPLETE_MAGIC);
            packet[8..40].copy_from_slice(&nonce);
            packet[40..].copy_from_slice(&binding);
            Ok(packet)
        })
    }
    pub fn new(
        binding: &Binding,
        slot: u8,
        nonce: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.with_prepaid_scope(
            binding.retained_storage(),
            8,
            Self::WORK,
            Self::SCRATCH,
            |_| {
                if slot >= 4 || nonce == [0; 32] {
                    return Err(Error::Invalid);
                }
                let mut bytes = [0; BYTES];
                bytes[..8].copy_from_slice(MAGIC);
                bytes[8] = slot;
                bytes[16..48].copy_from_slice(&nonce);
                bytes[48..].copy_from_slice(binding.canonical_bytes());
                Ok(Self { bytes })
            },
        )
    }
    /// Validates transfer framing only. The root decodes/adopts the single exact
    /// native binding independently, then compares it across every packet.
    pub fn decode(
        bytes: &[u8],
        slot: u8,
        nonce: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.with_prepaid_scope(bytes.len().min(BYTES), 8, Self::WORK, Self::SCRATCH, |_| {
            let bytes: &[u8; BYTES] = bytes.try_into().map_err(|_| Error::Invalid)?;
            if slot >= 4
                || nonce == [0; 32]
                || &bytes[..8] != MAGIC
                || bytes[8] != slot
                || bytes[9..16] != [0; 7]
                || bytes[16..48] != nonce
            {
                return Err(Error::Invalid);
            }
            Ok(Self { bytes: *bytes })
        })
    }
    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        &self.bytes
    }
    pub fn binding_bytes(&self) -> &[u8] {
        &self.bytes[48..]
    }
    pub fn nonce_packet(nonce: [u8; 32], budget: &mut Budget<'_>) -> Result<[u8; 40]> {
        budget.with_prepaid_scope(0, 8, 128, 128, |_| {
            if nonce == [0; 32] {
                return Err(Error::Invalid);
            }
            let mut packet = [0; 40];
            packet[..8].copy_from_slice(NONCE_MAGIC);
            packet[8..].copy_from_slice(&nonce);
            Ok(packet)
        })
    }
    pub fn decode_nonce(packet: &[u8; 40], budget: &mut Budget<'_>) -> Result<[u8; 32]> {
        budget.with_prepaid_scope(40, 8, 128, 128, |_| {
            let nonce = packet[8..].try_into().unwrap();
            if &packet[..8] != NONCE_MAGIC || nonce == [0; 32] {
                return Err(Error::Invalid);
            }
            Ok(nonce)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    #[test]
    fn transfer_rejects_wrong_slot_nonce_reserved_bytes_and_role_magic() {
        let mut work = Work::new(1_000_000);
        let mut b = Budget::new(&mut work, 100_000);
        b.reserve_storage(10_000).unwrap();
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[16..48].copy_from_slice(&[1; 32]);
        assert!(Frame::decode(&bytes, 0, [1; 32], &mut b).is_ok());
        assert!(Frame::decode(&bytes, 1, [1; 32], &mut b).is_err());
        assert!(Frame::decode(&bytes, 0, [2; 32], &mut b).is_err());
        bytes[9] = 1;
        assert!(Frame::decode(&bytes, 0, [1; 32], &mut b).is_err());
        bytes[9] = 0;
        bytes[..8].copy_from_slice(NONCE_MAGIC);
        assert!(Frame::decode(&bytes, 0, [1; 32], &mut b).is_err());
        let packet = Frame::nonce_packet([1; 32], &mut b).unwrap();
        assert_eq!(Frame::decode_nonce(&packet, &mut b).unwrap(), [1; 32]);
        assert!(Frame::nonce_packet([0; 32], &mut b).is_err());
    }
}
