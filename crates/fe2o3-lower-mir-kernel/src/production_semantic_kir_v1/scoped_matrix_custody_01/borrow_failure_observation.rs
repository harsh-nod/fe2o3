use std::io::{self, Write};
use super::Role;

// Fixed-size metadata of the original failed Borrow query, never another query.
pub(super) struct Observation {
    pub(super) body: [u8; 32],
    pub(super) root: u32,
    pub(super) block: u32,
    pub(super) statement: Option<u32>,
    pub(super) local: u32,
    pub(super) projections: usize,
    pub(super) role: Role,
    pub(super) owner_type: Option<u32>,
    pub(super) promoted: bool,
    pub(super) source_block: Option<(u32, u32, u32)>,
    pub(super) source_local: Option<(u32, u32, u32)>,
}

pub(super) fn emit(observation: Observation) {
    let _ = write_observation(&mut io::stderr().lock(), &observation);
}

fn write_observation(out: &mut impl Write, observation: &Observation) -> io::Result<()> {
    write!(out, "capability-ssa-absent-borrow body=")?;
    for byte in observation.body {
        write!(out, "{byte:02x}")?;
    }
    writeln!(out,
        " root={} block={} statement={:?} local={} projections={} source_block={:?} source_local={:?} role={:?} owner_type={:?} promoted={} query=original-borrow-place error=NoPromotedUse diagnostic_only=true",
        observation.root, observation.block, observation.statement, observation.local,
        observation.projections, observation.source_block, observation.source_local,
        observation.role, observation.owner_type, observation.promoted)
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("borrow_failure_observation_tests.rs");
}
