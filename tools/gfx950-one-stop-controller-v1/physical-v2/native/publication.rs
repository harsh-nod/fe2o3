//! Bounded awaited raw observation output. Only the separate family validator
//! may promote it after cleanup-only ACK and current manager-generation joins.
use super::{clock::Clock, wire::NativePeer};
use fe2o3_private_one_stop_protocol::{Cleanup, ProtocolObservation, Refusal};
use std::io::{self, Write};
const CAP: usize = 18 * 1024 * 1024;
pub(super) struct Buffer {
    bytes: Vec<u8>,
}
impl Buffer {
    pub(super) fn reserve() -> Result<Self, Refusal> {
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(CAP).map_err(|_| Refusal::Bound)?;
        Ok(Self { bytes })
    }
}
impl Write for Buffer {
    fn write(&mut self, v: &[u8]) -> io::Result<usize> {
        if self
            .bytes
            .len()
            .checked_add(v.len())
            .is_none_or(|n| n > CAP)
            || self.bytes.capacity() - self.bytes.len() < v.len()
        {
            return Err(io::Error::other("one-stop raw report cap"));
        }
        self.bytes.extend_from_slice(v);
        Ok(v.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn publish(
    mut buffer: Buffer,
    peer: &NativePeer<'_>,
    result: &Result<ProtocolObservation, (Refusal, Cleanup)>,
    clock: Clock,
) -> Result<(), Refusal> {
    clock.check()?;
    buffer
        .write_all(b"{\"schema\":\"fe2o3-one-stop-native-peer-observation-v2\",\"result\":")
        .map_err(|_| Refusal::Incomplete)?;
    let result_value = match result {
        Ok(value) => serde_json::json!({"status":"observed","observation":value}),
        Err((reason, cleanup)) => {
            serde_json::json!({"status":"refused","reason":format!("{reason:?}"),"cleanup":cleanup})
        }
    };
    serde_json::to_writer(&mut buffer, &result_value).map_err(|_| Refusal::Bound)?;
    buffer
        .write_all(b",\"identity\":")
        .map_err(|_| Refusal::Incomplete)?;
    serde_json::to_writer(&mut buffer, &peer.identity()).map_err(|_| Refusal::Bound)?;
    buffer
        .write_all(b",\"transcript\":")
        .map_err(|_| Refusal::Incomplete)?;
    peer.encode_transcript(&mut buffer)?;
    buffer.write_all(b",\"accepted_by_family\":false,\"native_tuple_independently_replayed\":false,\"general_host_exclusion_proved\":false,\"whole_family_cleanup_proved\":false,\"operational_qualification\":false}\n")
  .map_err(|_|Refusal::Incomplete)?;
    clock.check()?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&buffer.bytes)
        .map_err(|_| Refusal::Incomplete)?;
    stdout.flush().map_err(|_| Refusal::Incomplete)?;
    // A full stdout prefix is not acceptance. Parent MUST require exit 0, timely
    // complete stream, ACK, and current family cleanup; quarantine on any failure.
    clock.check()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_budget_refuses_before_growth_and_keeps_prior_bytes() {
        let mut b = Buffer::reserve().unwrap();
        b.bytes.resize(CAP - 1, 0);
        b.write_all(b"x").unwrap();
        let old = b.bytes.len();
        assert!(b.write_all(b"y").is_err());
        assert_eq!(b.bytes.len(), old);
    }
}
