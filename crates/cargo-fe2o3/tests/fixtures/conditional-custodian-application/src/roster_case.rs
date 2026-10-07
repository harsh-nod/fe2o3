//! Bounded data/configuration oracles, never native admission or completion authority.
use std::ffi::OsString;

use crate::native_case::{self, ELEMENTS, GUARD_BYTES, HardwareCase};

pub const MAX_DEVICES: usize = 8;
pub const TAG_BYTES: usize = 24;
pub const MAX_FRAME_BYTES: usize = TAG_BYTES + (ELEMENTS + MAX_DEVICES - 1) * 4 + 2 * GUARD_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionCase {
    Pair(HardwareCase),
    Roster(RosterCase),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RosterCase {
    devices: [u64; MAX_DEVICES],
    len: usize,
}

pub fn parse_execution_case(
    args: impl IntoIterator<Item = OsString>,
) -> Result<Option<ExecutionCase>, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(None);
    };
    if first != "--roster" {
        return native_case::parse_case(std::iter::once(first).chain(args))
            .map(|case| case.map(ExecutionCase::Pair));
    }
    let mut devices = [0; MAX_DEVICES];
    let mut len = 0;
    for arg in args {
        if len == MAX_DEVICES {
            return Err("roster exceeds the eight-device qualification bound".into());
        }
        let text = arg.to_str().ok_or("GPU unique ID is not UTF-8")?;
        let digits = text
            .strip_prefix("0x")
            .ok_or("GPU unique ID requires 0x prefix")?;
        if digits.is_empty() || digits.len() > 16 || !digits.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("GPU unique ID must contain 1 to 16 hexadecimal digits".into());
        }
        let uid = u64::from_str_radix(digits, 16).map_err(|error| error.to_string())?;
        if uid == 0 || devices[..len].contains(&uid) {
            return Err("roster requires nonzero distinct GPU unique IDs".into());
        }
        devices[len] = uid;
        len += 1;
    }
    if len < 2 {
        return Err("roster requires two through eight GPU unique IDs".into());
    }
    Ok(Some(ExecutionCase::Roster(RosterCase { devices, len })))
}

impl RosterCase {
    pub fn devices(&self) -> &[u64] {
        &self.devices[..self.len]
    }

    pub fn count(self) -> usize {
        self.len
    }

    pub fn elements(self, shard: usize) -> usize {
        assert!(shard < self.len);
        ELEMENTS + shard
    }

    pub fn payload_bytes(self, shard: usize) -> usize {
        TAG_BYTES + self.elements(shard) * 4
    }

    pub fn result_budget_bytes(self) -> usize {
        (0..self.len)
            .map(|shard| self.elements(shard) * 4 * 2)
            .sum()
    }

    pub fn routes(self) -> impl Iterator<Item = (usize, usize)> {
        (0..self.len).flat_map(move |source| {
            (0..self.len)
                .filter(move |destination| *destination != source)
                .map(move |destination| (source, destination))
        })
    }

    pub fn check_observed_roster(self, observed: &[u64]) -> Result<(), String> {
        if observed != self.devices() {
            return Err(
                "admitted context roster differs from the exact requested UID order".into(),
            );
        }
        Ok(())
    }

    pub fn check_fill(self, shard: usize, values: &[u32]) -> Result<(), String> {
        if values.len() != self.elements(shard)
            || values
                .iter()
                .enumerate()
                .any(|(index, value)| *value != index as u32)
        {
            return Err(format!(
                "shard {shard} fill content or exact extent differs"
            ));
        }
        Ok(())
    }

    /// The tag is host-authored routing data, not kernel output or proof evidence.
    pub fn payload(self, shard: usize, completed_fill: &[u8]) -> Result<Vec<u8>, String> {
        if completed_fill.len() != self.elements(shard) * 4
            || completed_fill
                .chunks_exact(4)
                .enumerate()
                .any(|(index, bytes)| bytes != (index as u32).to_le_bytes())
        {
            return Err("encoded completed shard differs from its fill oracle".into());
        }
        let mut bytes = Vec::with_capacity(self.payload_bytes(shard));
        bytes.extend_from_slice(&self.devices[shard].to_le_bytes());
        bytes.extend_from_slice(&(shard as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.elements(shard) as u64).to_le_bytes());
        bytes.extend_from_slice(completed_fill);
        Ok(bytes)
    }

    pub fn destination_frame(self, source: usize, destination: usize) -> Vec<u8> {
        assert!(source < self.len && destination < self.len && source != destination);
        vec![0x80 + (source * MAX_DEVICES + destination) as u8; MAX_FRAME_BYTES]
    }

    pub fn check_route(
        self,
        source: usize,
        destination: usize,
        payload: &[u8],
        source_readback: &[u8],
        destination_readback: &[u8],
    ) -> Result<(), String> {
        let mut expected = self.destination_frame(source, destination);
        if payload.len() != self.payload_bytes(source)
            || payload[..8] != self.devices[source].to_le_bytes()
            || payload[8..16] != (source as u64).to_le_bytes()
            || payload[16..TAG_BYTES] != (self.elements(source) as u64).to_le_bytes()
            || source_readback != payload
        {
            return Err("peer source shard identity, data or extent differs".into());
        }
        expected[GUARD_BYTES..GUARD_BYTES + payload.len()].copy_from_slice(payload);
        if destination_readback != expected {
            return Err(format!(
                "directed route {source}->{destination} payload or guards differ"
            ));
        }
        Ok(())
    }
}

/// A data-only fixture ledger. Callers must supply actual checked runtime observations.
pub struct CampaignLedger {
    case: RosterCase,
    fills: usize,
    peers: usize,
}

impl CampaignLedger {
    pub fn new(case: RosterCase) -> Self {
        Self {
            case,
            fills: 0,
            peers: 0,
        }
    }

    pub fn record_fill(&mut self, shard: usize) -> Result<(), String> {
        if shard != self.fills || self.fills == self.case.count() || self.peers != 0 {
            return Err("missing, duplicate or out-of-order shard completion".into());
        }
        self.fills += 1;
        Ok(())
    }

    pub fn record_peer(&mut self, route: (usize, usize), native_count: u64) -> Result<(), String> {
        if self.fills != self.case.count()
            || self.case.routes().nth(self.peers) != Some(route)
            || native_count != (self.peers + 1) as u64
        {
            return Err(
                "directed route or native retirement count differs from complete campaign".into(),
            );
        }
        self.peers += 1;
        Ok(())
    }

    pub fn finish(self) -> Result<CampaignReport, String> {
        if self.fills != self.case.count() || self.peers != self.case.routes().count() {
            return Err("roster campaign is incomplete".into());
        }
        Ok(CampaignReport { case: self.case })
    }
}

/// Data-only report; emission follows separate owned shutdown and result-credit checks.
pub struct CampaignReport {
    case: RosterCase,
}

impl CampaignReport {
    pub fn json(&self) -> String {
        let devices = self
            .case
            .devices()
            .iter()
            .map(|uid| format!("\"{uid:#018x}\""))
            .collect::<Vec<_>>()
            .join(",");
        let shards = (0..self.case.count())
            .map(|shard| {
                format!(
                    "{{\"device\":\"{:#018x}\",\"elements\":{}}}",
                    self.case.devices[shard],
                    self.case.elements(shard)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let pairs = self
            .case
            .routes()
            .map(|(source, destination)| {
                format!(
                    "[\"{:#018x}\",\"{:#018x}\"]",
                    self.case.devices[source], self.case.devices[destination]
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            concat!(
                "{{\"schema\":\"fe2o3.genuine-device-roster.v1\",\"devices\":[{}],\"shards\":[{}],",
                "\"directed_peer_pairs\":[{}],\"native_peer_completions\":{},",
                "\"roster_scope\":\"all-admitted-context-devices\",",
                "\"ordering\":\"compute-complete-before-peer-submit\",",
                "\"copy_compute_overlap\":\"not-measured\",\"peer_tag_origin\":\"host\",",
                "\"shutdown\":\"released\"}}"
            ),
            devices,
            shards,
            pairs,
            self.case.routes().count()
        )
    }
}

#[cfg(test)]
#[path = "roster_case_tests.rs"]
mod tests;
