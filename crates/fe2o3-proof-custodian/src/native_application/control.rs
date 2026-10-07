//! One-attempt native root/controller transport. No legacy frame is accepted.
use super::*;
use rustix::net;

const BYTES: usize = 120;
const MAGIC: &[u8; 8] = b"F3NACP1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION/CUSTODIAN-CONTROL/V1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub(crate) enum Kind {
    Ready = 1,
    Activate = 2,
    Activated = 3,
    Probe = 4,
    Retained = 5,
    Quarantined = 6,
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn encode(kind: Kind, nonce: [u8; 32], session: [u8; 32]) -> io::Result<[u8; BYTES]> {
    require(
        nonce != [0; 32] && session != [0; 32],
        "zero native control identity",
    )?;
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(MAGIC);
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[10] = kind as u8;
    bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
    bytes[24..56].copy_from_slice(&nonce);
    bytes[56..88].copy_from_slice(&session);
    let identity = hash(&bytes[..88]);
    bytes[88..].copy_from_slice(&identity);
    Ok(bytes)
}
fn decode(bytes: &[u8], nonce: [u8; 32], session: [u8; 32]) -> io::Result<Kind> {
    require(bytes.len() == BYTES, "native control frame length")?;
    require(
        nonce != [0; 32]
            && session != [0; 32]
            && &bytes[..8] == MAGIC
            && bytes[8..10] == 1u16.to_le_bytes()
            && bytes[11] == 0
            && bytes[12..16] == (BYTES as u32).to_le_bytes()
            && bytes[16..24] == [0; 8]
            && bytes[24..56] == nonce
            && bytes[56..88] == session
            && bytes[88..] == hash(&bytes[..88]),
        "native control frame identity",
    )?;
    match bytes[10] {
        1 => Ok(Kind::Ready),
        2 => Ok(Kind::Activate),
        3 => Ok(Kind::Activated),
        4 => Ok(Kind::Probe),
        5 => Ok(Kind::Retained),
        6 => Ok(Kind::Quarantined),
        _ => Err(io::Error::other("native control kind")),
    }
}
pub(crate) fn try_send(
    peer: &wire::ControlEndpoint,
    kind: Kind,
    nonce: [u8; 32],
    session: [u8; 32],
    budget: &mut Budget<'_>,
) -> io::Result<bool> {
    budget.charge_work(IO_WORK).map_err(other)?;
    require(
        budget.storage() >= size_of::<wire::ControlEndpoint>(),
        "native control endpoint not prepaid",
    )?;
    budget.reserve_storage(IO_STORAGE).map_err(other)?;
    peer.revalidate()?;
    let bytes = encode(kind, nonce, session)?;
    let result = match net::send(
        peer,
        &bytes,
        net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
    ) {
        Ok(n) => {
            require(n == BYTES, "partial native control send")?;
            true
        }
        Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => false,
        Err(e) => return Err(e.into()),
    };
    budget.release_storage(IO_STORAGE).map_err(other)?;
    Ok(result)
}
pub(crate) fn try_receive(
    peer: &wire::ControlEndpoint,
    sender: (i32, u32, u32),
    nonce: [u8; 32],
    session: [u8; 32],
    budget: &mut Budget<'_>,
) -> io::Result<Option<Kind>> {
    budget.charge_work(IO_WORK).map_err(other)?;
    require(
        budget.storage() >= size_of::<wire::ControlEndpoint>(),
        "native control endpoint not prepaid",
    )?;
    budget.reserve_storage(IO_STORAGE).map_err(other)?;
    peer.revalidate()?;
    let result = wire::try_receive_with::<BYTES, _>(peer.as_fd(), sender, |bytes| {
        decode(bytes, nonce, session)
    })?;
    peer.revalidate()?;
    budget.release_storage(IO_STORAGE).map_err(other)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_control_frames_bind_phase_nonce_session_and_every_byte() {
        for kind in [
            Kind::Ready,
            Kind::Activate,
            Kind::Activated,
            Kind::Probe,
            Kind::Retained,
            Kind::Quarantined,
        ] {
            let bytes = encode(kind, [1; 32], [2; 32]).unwrap();
            assert_eq!(decode(&bytes, [1; 32], [2; 32]).unwrap(), kind);
            for offset in 0..BYTES {
                let mut changed = bytes;
                changed[offset] ^= 1;
                assert!(decode(&changed, [1; 32], [2; 32]).is_err());
            }
            assert!(decode(&bytes[..BYTES - 1], [1; 32], [2; 32]).is_err());
            assert!(decode(&bytes, [3; 32], [2; 32]).is_err());
            assert!(decode(&bytes, [1; 32], [3; 32]).is_err());
        }
    }
    #[test]
    fn native_control_unknown_and_legacy_frames_reject_even_resealed() {
        for kind in [0, 7, 255] {
            let mut bytes = encode(Kind::Activate, [1; 32], [2; 32]).unwrap();
            bytes[10] = kind;
            let identity = hash(&bytes[..88]);
            bytes[88..].copy_from_slice(&identity);
            assert!(decode(&bytes, [1; 32], [2; 32]).is_err());
        }
        let mut bytes = encode(Kind::Activate, [1; 32], [2; 32]).unwrap();
        bytes[..8].copy_from_slice(b"F3PCMS1\0");
        let identity = hash(&bytes[..88]);
        bytes[88..].copy_from_slice(&identity);
        assert!(decode(&bytes, [1; 32], [2; 32]).is_err());
    }
    #[test]
    fn native_control_real_credential_exchange_preserves_prepaid_floor() {
        let (a, b) = wire::control_pair().unwrap();
        let a = wire::ControlEndpoint::admit(a).unwrap();
        let b = wire::ControlEndpoint::admit(b).unwrap();
        let mut work = Work::new(4 * IO_WORK);
        let floor = 2 * size_of::<wire::ControlEndpoint>();
        let mut budget = Budget::new(&mut work, floor + IO_STORAGE);
        budget.reserve_storage(floor).unwrap();
        let sender = (
            std::process::id() as i32,
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        assert_eq!(
            try_receive(&b, sender, [1; 32], [2; 32], &mut budget).unwrap(),
            None
        );
        assert!(try_send(&a, Kind::Activate, [1; 32], [2; 32], &mut budget).unwrap());
        assert_eq!(
            try_receive(&b, sender, [1; 32], [2; 32], &mut budget).unwrap(),
            Some(Kind::Activate)
        );
        assert_eq!(budget.storage(), floor);
    }
}
