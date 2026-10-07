//! Closed inert case/report oracle, never native admission or qualification.
use super::native_registry4_case::{Case, parse_profile};
use std::ffi::OsString;

#[path = "native_independent_arena_case/observation.rs"]
pub mod observation;

pub const MEMBERS: usize = 1024;
pub const SHORT: usize = 64;
pub const LONG: usize = 65_536;
pub const SOURCE_BYTES: u64 = (MEMBERS / 2 * (SHORT + LONG) * 4) as u64;
pub const METADATA_BYTES: u64 = SOURCE_BYTES + 64 * 1024 * 1024;
pub const METADATA_RECORDS: usize = 64;
pub const RESULT_BYTES: u64 = 4 * SOURCE_BYTES;
pub const RESULT_RECORDS: usize = 3 * MEMBERS;

pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Case, String> {
    parse_profile(args, "--native-v5-independent-arena1024")
}

pub fn count(member: usize) -> Result<usize, String> {
    if member >= MEMBERS {
        return Err("unknown independent arena member".into());
    }
    // Long members precede short members in each pair. Counts alone make no
    // statement about actual device duration or hardware completion order.
    Ok(if member.is_multiple_of(2) {
        LONG
    } else {
        SHORT
    })
}

pub fn check_output(member: usize, output: &[u32]) -> Result<(), String> {
    if output.len() != count(member)?
        || output
            .iter()
            .enumerate()
            .any(|(i, value)| *value != i as u32)
    {
        return Err(format!(
            "independent arena original output {member} mismatch"
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Witness {
    pub earlier: usize,
    pub later: usize,
    pub earlier_published: u64,
    pub later_published: u64,
    pub later_completed: u64,
    pub earlier_pending: u64,
}

impl Witness {
    pub fn validate(self, events: u64) -> Result<(), String> {
        if self.earlier >= self.later
            || self.later >= MEMBERS
            || self.earlier_published == 0
            || self.earlier_published >= self.later_published
            || self.later_published >= self.later_completed
            || self.later_completed >= self.earlier_pending
            || self.earlier_pending > events
        {
            return Err("invalid original receipt observation chronology".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Report {
    pub events: u64,
    pub publications: usize,
    pub completions: usize,
    pub pending: u64,
    pub maximum_unobserved: usize,
    pub pre_metadata_bytes: u64,
    pub pre_metadata_records: usize,
    pub residual_metadata_bytes: u64,
    pub residual_metadata_records: usize,
    pub pre_result_bytes: u64,
    pub pre_result_records: usize,
    pub residual_source_bytes: u64,
    pub residual_source_records: usize,
    pub witness: Option<Witness>,
}

impl Report {
    pub fn validate(self) -> Result<(), String> {
        if self.publications != MEMBERS
            || self.completions != MEMBERS
            || self.pending.checked_add((2 * MEMBERS) as u64) != Some(self.events)
            || self.maximum_unobserved == 0
            || self.maximum_unobserved > MEMBERS
            || self.pre_metadata_bytes <= SOURCE_BYTES
            || self.pre_metadata_bytes > METADATA_BYTES
            || self.pre_metadata_records == 0
            || self.pre_metadata_records > METADATA_RECORDS
            || self.residual_metadata_bytes == 0
            || self.residual_metadata_bytes >= self.pre_metadata_bytes
            || self.residual_metadata_records == 0
            || self.residual_metadata_records > self.pre_metadata_records
            || self.pre_result_bytes != 3 * SOURCE_BYTES
            || self.pre_result_records != 2 * MEMBERS
            || self.residual_source_bytes != SOURCE_BYTES
            || self.residual_source_records != MEMBERS
        {
            return Err(
                "independent arena report lacks exact original receipt/resource observations"
                    .into(),
            );
        }
        if let Some(witness) = self.witness {
            if self.pending == 0 {
                return Err("no original Pending observation".into());
            }
            witness.validate(self.events)?;
        }
        Ok(())
    }

    pub fn fields(self, device: u64) -> Result<String, String> {
        self.validate()?;
        let witness = self.witness.map_or_else(|| "null".to_string(), |w| format!(
            "{{\"earlier_member\":{},\"later_member\":{},\"earlier_published\":{},\"later_published\":{},\"later_completed\":{},\"earlier_pending\":{}}}",
            w.earlier,w.later,w.earlier_published,w.later_published,w.later_completed,w.earlier_pending));
        let qualification = if self.witness.is_some() {
            "receipt-order-observed-only"
        } else {
            "unqualified-no-receipt-order-witness"
        };
        Ok(format!(
            concat!(
                "\"device\":\"0x{device:016x}\",\"target\":\"gfx942:xnack-\",",
                "\"members\":1024,\"counts\":{{\"even\":65536,\"odd\":64}},\"grid_equals_count\":true,",
                "\"source_bytes\":{SOURCE_BYTES},\"metadata_capacity_bytes\":{METADATA_BYTES},\"metadata_capacity_records\":{METADATA_RECORDS},",
                "\"result_capacity_bytes\":{RESULT_BYTES},\"result_capacity_records\":{RESULT_RECORDS},",
                "\"prelaunch_metadata_bytes\":{},\"prelaunch_metadata_records\":{},\"prelaunch_result_bytes\":{},\"prelaunch_result_records\":{},",
                "\"residual_metadata_bytes\":{},\"residual_metadata_records\":{},\"residual_source_bytes\":{},\"residual_source_records\":{},",
                "\"original_receipt_events\":{},\"publications\":{},\"pending_observations\":{},\"completed_observations\":{},",
                "\"maximum_published_without_observed_completion\":{},\"out_of_order_witness\":{},\"native_out_of_order_measured\":{},",
                "\"copied_results\":1024,\"results_dropped_before_common_destroy\":1024,\"common_arena_destroyed\":true,",
                "\"metadata_credits\":\"refunded\",\"native_durations_measured\":false,\"thousands_inflight_qualified\":false,\"rolling_rearm\":false,",
                "\"qualification_status\":\"{qualification}\""
            ),
            self.pre_metadata_bytes,
            self.pre_metadata_records,
            self.pre_result_bytes,
            self.pre_result_records,
            self.residual_metadata_bytes,
            self.residual_metadata_records,
            self.residual_source_bytes,
            self.residual_source_records,
            self.events,
            self.publications,
            self.pending,
            self.completions,
            self.maximum_unobserved,
            witness,
            self.witness.is_some(),
            device = device,
            qualification = qualification,
            SOURCE_BYTES = SOURCE_BYTES,
            METADATA_BYTES = METADATA_BYTES,
            METADATA_RECORDS = METADATA_RECORDS,
            RESULT_BYTES = RESULT_BYTES,
            RESULT_RECORDS = RESULT_RECORDS
        ))
    }
}

#[cfg(test)]
#[path = "native_independent_arena_case/tests.rs"]
mod tests;
