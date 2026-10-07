//! Pure validation and raw tick serialization; no clock conversion or authority.

use std::collections::BTreeMap;

pub(super) fn validate_identity(identity: [u64; 3], count: usize) -> Result<u64, String> {
    if !(1..=1024).contains(&count) {
        return Err("native timestamp count".into());
    }
    identity[2]
        .checked_add(count as u64)
        .ok_or_else(|| "native timestamp frontier overflow".into())
}

#[derive(serde::Serialize)]
pub(super) struct Record<'a> {
    schema: &'static str,
    clock: &'static str,
    device_unique_id: u64,
    queue_epoch: u64,
    first_write: u64,
    next_write: u64,
    symbols: &'a BTreeMap<u64, String>,
    // [kernel handle, start tick, end tick], in exact packet order.
    packets: Vec<[u64; 3]>,
}

impl<'a> Record<'a> {
    pub(super) fn new(
        identity: [u64; 3],
        kernels: &[u64],
        symbols: &'a BTreeMap<u64, String>,
        ticks: Vec<(u64, u64)>,
    ) -> Result<Self, String> {
        let next_write = validate_identity(identity, kernels.len())?;
        if ticks.len() != kernels.len()
            || symbols.len() > kernels.len()
            || symbols.keys().any(|key| !kernels.contains(key))
            || symbols
                .values()
                .any(|name| name.is_empty() || name.len() > 1024)
        {
            return Err("native timestamp roster".into());
        }
        let packets = kernels
            .iter()
            .zip(ticks)
            .map(|(&kernel, (start, end))| {
                if !symbols.contains_key(&kernel) || start == 0 || end <= start {
                    return Err("unqualified native dispatch timestamps".into());
                }
                Ok([kernel, start, end])
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            schema: "Fe2o3NativePacketObservationV1",
            clock: "raw_gpu_clock_ticks",
            device_unique_id: identity[0],
            queue_epoch: identity[1],
            first_write: identity[2],
            next_write,
            symbols,
            packets,
        })
    }
}

#[cfg(test)]
#[path = "engineering_native_packet_observation_tests.rs"]
mod tests;
