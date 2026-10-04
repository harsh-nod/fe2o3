use super::*;
use sha2::{Digest, Sha256};

pub(super) const HEADER: usize = 96;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(super) enum Kind {
    Hello = 1,
    Welcome,
    Begin,
    Attach,
    Ready,
    Activate,
    Activated,
    Probe,
    Retained,
    Quarantined,
    Rejected,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Frame {
    pub kind: Kind,
    pub connection: [u8; 32],
    pub operation: [u8; 32],
    pub sequence: u64,
    pub deadline: u64,
    pub body: Vec<u8>,
}
impl Frame {
    pub fn rights(&self) -> usize {
        match self.kind {
            Kind::Hello | Kind::Welcome | Kind::Attach | Kind::Ready => 1,
            Kind::Begin => 2,
            _ => 0,
        }
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.body.len() > MAX_TRANSPORT_PACKET - HEADER {
            return Err(invalid("oversized manager body"));
        }
        let mut bytes = vec![0; HEADER + self.body.len()];
        bytes[..8].copy_from_slice(b"F3PMCH1\0");
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[10] = self.kind as u8;
        bytes[11] = self.rights() as u8;
        bytes[12..16].copy_from_slice(&((HEADER + self.body.len()) as u32).to_le_bytes());
        bytes[16..48].copy_from_slice(&self.connection);
        bytes[48..80].copy_from_slice(&self.operation);
        bytes[80..88].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[88..96].copy_from_slice(&self.deadline.to_le_bytes());
        bytes[96..].copy_from_slice(&self.body);
        Self::decode(&bytes)?;
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if !(HEADER..=MAX_TRANSPORT_PACKET).contains(&bytes.len())
            || &bytes[..8] != b"F3PMCH1\0"
            || bytes[8..10] != 1u16.to_le_bytes()
            || bytes[12..16] != (bytes.len() as u32).to_le_bytes()
        {
            return Err(invalid("noncanonical proof-manager frame"));
        }
        let kind = match bytes[10] {
            1 => Kind::Hello,
            2 => Kind::Welcome,
            3 => Kind::Begin,
            4 => Kind::Attach,
            5 => Kind::Ready,
            6 => Kind::Activate,
            7 => Kind::Activated,
            8 => Kind::Probe,
            9 => Kind::Retained,
            10 => Kind::Quarantined,
            11 => Kind::Rejected,
            _ => return Err(invalid("unknown proof-manager frame")),
        };
        let value = Self {
            kind,
            connection: bytes[16..48].try_into().unwrap(),
            operation: bytes[48..80].try_into().unwrap(),
            sequence: u64::from_le_bytes(bytes[80..88].try_into().unwrap()),
            deadline: u64::from_le_bytes(bytes[88..96].try_into().unwrap()),
            body: bytes[96..].to_vec(),
        };
        let n = value.body.len();
        let shape = match kind {
            Kind::Hello | Kind::Welcome => n == 32,
            Kind::Begin => n == fe2o3_runtime_protocol::WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1,
            Kind::Attach => n == 64,
            Kind::Ready => {
                n == fe2o3_runtime_protocol::WORKER_V3_APPLICATION_PROOF_SESSION_BYTES_V1
            }
            Kind::Activate | Kind::Activated => n == 32,
            Kind::Probe => n == 0,
            Kind::Retained | Kind::Quarantined => n > 0,
            Kind::Rejected => n > 0 && n <= 512,
        };
        let bootstrap = matches!(kind, Kind::Hello | Kind::Welcome);
        if !shape
            || bytes[11] != value.rights() as u8
            || value.connection == [0; 32]
            || bootstrap != (value.sequence == 0)
            || value.sequence == u64::MAX
            || (kind == Kind::Hello) != (value.operation == [0; 32])
            || (matches!(kind, Kind::Begin | Kind::Attach)) != (value.deadline > 0)
        {
            return Err(invalid("proof-manager phase shape differs"));
        }
        Ok(value)
    }
}
pub(super) fn connection_nonce(
    client: [u8; 32],
    manager: [u8; 32],
    deployment: [u8; 32],
) -> Result<[u8; 32]> {
    if client == [0; 32] || manager == [0; 32] || client == manager {
        return Err(invalid("manager challenge reused"));
    }
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/PROOF-MANAGER-CHANNEL/V1\0");
    hash.update(client);
    hash.update(manager);
    hash.update(deployment);
    Ok(hash.finalize().into())
}

pub(super) fn monotonic_ns() -> Result<u64> {
    let mut value = std::mem::MaybeUninit::<libc::timespec>::uninit();
    // SAFETY: clock_gettime initializes one writable timespec on success.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, value.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error().into());
    }
    // SAFETY: success initialized the complete structure above.
    let value = unsafe { value.assume_init() };
    u64::try_from(value.tv_sec)
        .ok()
        .and_then(|s| s.checked_mul(1_000_000_000))
        .and_then(|s| {
            u64::try_from(value.tv_nsec)
                .ok()
                .and_then(|n| s.checked_add(n))
        })
        .ok_or_else(|| invalid("invalid monotonic clock"))
}
pub(super) fn export_deadline(deadline: Instant) -> Result<u64> {
    let base = monotonic_ns()?;
    let remaining = deadline
        .checked_duration_since(Instant::now())
        .ok_or(CompilerExecutionObserverErrorV1::Timeout)?;
    base.checked_add(u64::try_from(remaining.as_nanos()).map_err(|_| invalid("deadline overflow"))?)
        .ok_or_else(|| invalid("deadline overflow"))
}
pub(super) fn import_deadline(value: u64) -> Result<Instant> {
    let base = Instant::now();
    let remaining = value
        .checked_sub(monotonic_ns()?)
        .ok_or(CompilerExecutionObserverErrorV1::Timeout)?;
    if remaining == 0 || remaining > 120_000_000_000 {
        return Err(invalid("manager startup deadline exceeds bound"));
    }
    Ok(base + Duration::from_nanos(remaining))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_frame_enforces_its_exact_phase_shape() {
        for (kind, size) in [
            (Kind::Hello, 32),
            (Kind::Welcome, 32),
            (Kind::Begin, 840),
            (Kind::Attach, 64),
            (Kind::Ready, 192),
            (Kind::Activate, 32),
            (Kind::Activated, 32),
            (Kind::Probe, 0),
            (Kind::Retained, 856),
            (Kind::Quarantined, 856),
            (Kind::Rejected, 512),
        ] {
            let good = Frame {
                kind,
                connection: [1; 32],
                operation: if kind == Kind::Hello {
                    [0; 32]
                } else {
                    [2; 32]
                },
                sequence: if matches!(kind, Kind::Hello | Kind::Welcome) {
                    0
                } else {
                    1
                },
                deadline: u64::from(matches!(kind, Kind::Begin | Kind::Attach)),
                body: vec![7; size],
            };
            let bytes = good.encode().unwrap();
            assert_eq!(Frame::decode(&bytes).unwrap(), good);
            for index in [0, 8, 11, 12] {
                let mut bytes = bytes.clone();
                bytes[index] ^= 1;
                assert!(Frame::decode(&bytes).is_err(), "{kind:?} byte {index}");
            }
            for mutation in 0..5 {
                let mut changed = good.clone();
                match mutation {
                    0 => changed.connection = [0; 32],
                    1 => {
                        changed.operation = if kind == Kind::Hello {
                            [1; 32]
                        } else {
                            [0; 32]
                        }
                    }
                    2 => changed.sequence = if changed.sequence == 0 { 1 } else { 0 },
                    3 => changed.deadline = if changed.deadline == 0 { 1 } else { 0 },
                    _ => changed.body = vec![0; MAX_TRANSPORT_PACKET - HEADER + 1],
                }
                assert!(changed.encode().is_err(), "{kind:?} mutation {mutation}");
            }
        }
    }
    #[test]
    fn challenge_and_deadline_are_not_restartable() {
        assert!(connection_nonce([0; 32], [2; 32], [3; 32]).is_err());
        assert!(connection_nonce([1; 32], [1; 32], [3; 32]).is_err());
        assert_ne!(
            connection_nonce([1; 32], [2; 32], [3; 32]).unwrap(),
            connection_nonce([2; 32], [1; 32], [3; 32]).unwrap()
        );
        assert!(export_deadline(Instant::now()).is_err());
        assert!(import_deadline(monotonic_ns().unwrap() - 1).is_err());
        assert!(import_deadline(monotonic_ns().unwrap() + 121_000_000_000).is_err());
        let original = Instant::now() + Duration::from_secs(10);
        let wire = export_deadline(original).unwrap();
        let imported = import_deadline(wire).unwrap();
        assert!(imported <= original);
        assert!(imported > Instant::now() + Duration::from_secs(9));
    }
}
