//! Fixed V2/V3 envelope mechanics; V1's raw-body format does not use this codec.
use super::super::{HandoffEngineError, Resources};
use sha2::{Digest, Sha256};

pub(in crate::compiler_module_handoff) const SUBJECT_START: usize = 24;
pub(in crate::compiler_module_handoff) const SUBJECT_BYTES: usize = 690;
pub(in crate::compiler_module_handoff) const SUBJECT_END: usize = SUBJECT_START + SUBJECT_BYTES;
pub(in crate::compiler_module_handoff) const BODY_START: usize = SUBJECT_END + 8;
pub(in crate::compiler_module_handoff) const OVERHEAD: usize = BODY_START + 32;

// Implemented only by the two closed transport adapters, never a caller callback.
pub(in crate::compiler_module_handoff) trait Error:
    From<HandoffEngineError>
{
    fn invalid(reason: &'static str) -> Self;
    fn mismatch() -> Self;
}

pub(in crate::compiler_module_handoff) struct Schema {
    pub magic: [u8; 8],
    pub version: u16,
    pub domain: &'static [u8],
    pub maximum: usize,
}

impl Schema {
    pub(in crate::compiler_module_handoff) fn identity(&self, prefix: &[u8]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.domain);
        hash.update((prefix.len() as u64).to_le_bytes());
        hash.update(prefix);
        hash.finalize().into()
    }

    /// Body length is admitted by the version entry before this fixed codec.
    pub(in crate::compiler_module_handoff) fn encode<E: Error>(
        &self,
        subject: &[u8; SUBJECT_BYTES],
        body: &[u8],
        r: &mut Resources<'_, '_>,
    ) -> Result<Vec<u8>, E> {
        let n = OVERHEAD + body.len();
        r.work(4 * n + 4096)?;
        let mut wire = r.buffer(n)?;
        wire.resize(n, 0);
        wire[..8].copy_from_slice(&self.magic);
        wire[8..10].copy_from_slice(&self.version.to_le_bytes());
        wire[12..20].copy_from_slice(&(n as u64).to_le_bytes());
        wire[SUBJECT_START..SUBJECT_END].copy_from_slice(subject);
        wire[SUBJECT_END..BODY_START].copy_from_slice(&(body.len() as u64).to_le_bytes());
        wire[BODY_START..n - 32].copy_from_slice(body);
        let digest = self.identity(&wire[..n - 32]);
        wire[n - 32..].copy_from_slice(&digest);
        Ok(wire)
    }

    pub(in crate::compiler_module_handoff) fn inspect<E: Error>(
        &self,
        wire: &[u8],
        expected: &[u8; SUBJECT_BYTES],
        r: &mut Resources<'_, '_>,
    ) -> Result<([u8; 32], usize), E> {
        let n = wire.len();
        if !(OVERHEAD + 1..=self.maximum).contains(&n) {
            return Err(E::invalid("length"));
        }
        r.work(4 * n + 4096)?;
        let u64_at = |offset| -> Result<u64, E> {
            Ok(u64::from_le_bytes(
                wire[offset..offset + 8]
                    .try_into()
                    .map_err(|_| E::invalid("truncated"))?,
            ))
        };
        if wire[..8] != self.magic
            || wire[8..10] != self.version.to_le_bytes()
            || wire[10..12] != [0; 2]
            || wire[20..24] != [0; 4]
            || u64_at(12)? != n as u64
            || u64_at(SUBJECT_END)? != (n - OVERHEAD) as u64
        {
            return Err(E::invalid("header"));
        }
        if &wire[SUBJECT_START..SUBJECT_END] != expected {
            return Err(E::mismatch());
        }
        let digest = self.identity(&wire[..n - 32]);
        if wire[n - 32..] != digest {
            return Err(E::invalid("identity"));
        }
        Ok((digest, n - OVERHEAD))
    }
}
