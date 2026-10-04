//! Bounded read-only mixed state snapshots under one fresh group fence pair.

use super::*;

const MAX_STATES: usize = 144;

/// A borrowed typed state identity, never a pointer or a reusable fence token.
/// Entry order is preserved in the returned snapshots.
#[derive(Clone, Copy, Debug)]
pub enum Gfx950EngineeringPeerStateBankEntryV1<'a> {
    Prefix(&'a Gfx950EngineeringPeerWaveQkvAttentionOutputTilesStateV6),
    Mlp(&'a Gfx950EngineeringPeerWaveMlpTilesStateV2),
}

impl Gfx950EngineeringPeerStateBankEntryV1<'_> {
    fn token(self) -> Gfx950EngineeringPeerBufferV1 {
        match self {
            Self::Prefix(state) => state.buffer,
            Self::Mlp(state) => state.buffer,
        }
    }
}

/// Owned Acquire-load results. A snapshot grants no rearm or dispatch authority.
#[derive(Debug, Eq, PartialEq)]
pub enum Gfx950EngineeringPeerStateBankSnapshotV1 {
    Prefix([u32; 284]),
    Mlp([u32; 548]),
}

trait BankBackend {
    fn check(&mut self) -> Result<()>;
    fn validate(&mut self, entry: Gfx950EngineeringPeerStateBankEntryV1<'_>) -> Result<()>;
    fn observe(
        &mut self,
        entry: Gfx950EngineeringPeerStateBankEntryV1<'_>,
    ) -> Result<Gfx950EngineeringPeerStateBankSnapshotV1>;
}

fn observe_bank(
    backend: &mut impl BankBackend,
    entries: &[Gfx950EngineeringPeerStateBankEntryV1<'_>],
) -> Result<Vec<Gfx950EngineeringPeerStateBankSnapshotV1>> {
    if entries.is_empty() || entries.len() > MAX_STATES {
        return Err("state bank requires 1..=144 entries".into());
    }
    backend.check()?;
    // Validate the whole roster before the first atomic load. IDs are unique
    // within a group; each entry's full token is independently authenticated.
    let mut ids = Vec::with_capacity(entries.len());
    for &entry in entries {
        backend.validate(entry)?;
        let id = entry.token().id;
        if ids.contains(&id) {
            return Err("duplicate state bank identity".into());
        }
        ids.push(id);
    }
    let mut snapshots = Vec::with_capacity(entries.len());
    for &entry in entries {
        snapshots.push(backend.observe(entry)?);
    }
    backend.check()?;
    Ok(snapshots)
}

struct NativeBank<'a> {
    group: &'a mut Gfx950EngineeringPeerGroupV1,
}

impl BankBackend for NativeBank<'_> {
    fn check(&mut self) -> Result<()> {
        check_contexts(&mut self.group.contexts, self.group.shared_full_currentness)
    }

    fn validate(&mut self, entry: Gfx950EngineeringPeerStateBankEntryV1<'_>) -> Result<()> {
        match entry {
            Gfx950EngineeringPeerStateBankEntryV1::Prefix(state) => {
                wave_qkv_attention_output_tiles_state_v6::validate_idle_bank_state(
                    self.group, state,
                )
            }
            Gfx950EngineeringPeerStateBankEntryV1::Mlp(state) => {
                wave_mlp_tiles_state_v2::validate_idle_bank_state(self.group, state)
            }
        }
    }

    fn observe(
        &mut self,
        entry: Gfx950EngineeringPeerStateBankEntryV1<'_>,
    ) -> Result<Gfx950EngineeringPeerStateBankSnapshotV1> {
        // SAFETY: observe_bank checks the whole group before reading, retains
        // this exclusive borrow, and checks again before returning any snapshot.
        // It never dispatches, mutates state, or changes mappings in between.
        unsafe {
            match entry {
                Gfx950EngineeringPeerStateBankEntryV1::Prefix(state) => {
                    wave_qkv_attention_output_tiles_state_v6::observe_within_idle_bank_fence(
                        self.group, state,
                    )
                    .map(Gfx950EngineeringPeerStateBankSnapshotV1::Prefix)
                }
                Gfx950EngineeringPeerStateBankEntryV1::Mlp(state) => {
                    wave_mlp_tiles_state_v2::observe_within_idle_bank_fence(self.group, state)
                        .map(Gfx950EngineeringPeerStateBankSnapshotV1::Mlp)
                }
            }
        }
    }
}

impl Gfx950EngineeringPeerGroupV1 {
    /// Acquire snapshots of 1..=144 distinct typed prefix/MLP states.
    /// Every token, owner, allocation, atomic lifetime and owner-only mapping
    /// is checked before the first read, under fresh full-group entry/exit
    /// fences. Ready, Submitted and Completed states may be observed; their
    /// activation is not changed. Failure quarantines the group and no partial
    /// snapshot vector escapes. This does not grant completion or reuse rights.
    pub fn observe_state_bank_v1(
        &mut self,
        entries: &[Gfx950EngineeringPeerStateBankEntryV1<'_>],
    ) -> Result<Vec<Gfx950EngineeringPeerStateBankSnapshotV1>> {
        self.require_active()?;
        let result = observe_bank(&mut NativeBank { group: self }, entries);
        self.finish(result)
    }
}

#[cfg(test)]
#[path = "engineering_gfx950_peer_state_bank_v1_tests.rs"]
mod tests;
