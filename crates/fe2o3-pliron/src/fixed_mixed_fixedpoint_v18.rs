//! Nominal Policy11 evidence, including the complete terminal unchanged round.
use super::*;

/// Move-only actual execution evidence. Bytes are not a constructor or permit.
pub struct MixedFixedpointExecutionWitnessV18 {
    canonical: Vec<u8>,
    rounds: usize,
}

impl MixedFixedpointExecutionWitnessV18 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }
    pub const fn policy_version(&self) -> u16 {
        11
    }
    pub const fn graph_schema(&self) -> u16 {
        18
    }
    pub const fn rounds(&self) -> usize {
        self.rounds
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub(crate) fn retained_storage(&self) -> usize {
        self.canonical.capacity()
    }

    pub(crate) fn from_execution(
        input: &Owner18,
        output: &Owner18,
        table: CanonicalStorageTableIdentityV18,
        report: &PlironOptimizationReportV1,
        map: &crate::KirOptimizationMapMixedFixedpointV18,
        execution: ExecutionProfileV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Resource> {
        budget.charge_work(
            4usize
                .checked_add(
                    report
                        .passes()
                        .len()
                        .checked_mul(4)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?,
        )?;
        let rounds = validate_fixedpoint_report(report)?;
        if !map.matches_execution(report)
            || map.input_identity() != input.identity()
            || map.output_identity() != output.identity()
        {
            return Err(Resource::Accounting);
        }
        // Existing V18 header/tail, two explicit round fields, actual rows only.
        let bytes = report
            .passes()
            .len()
            .checked_mul(60)
            .and_then(|rows| rows.checked_add(352))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(bytes)?;
        budget.reserve_storage(bytes)?;
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(bytes)
            .map_err(|_| Resource::Allocation)?;
        budget.reserve_storage(
            canonical
                .capacity()
                .checked_sub(bytes)
                .ok_or(Resource::Accounting)?,
        )?;
        canonical.resize(bytes, 0);
        let mut writer = RecordWriter {
            bytes: &mut canonical,
            cursor: 0,
        };
        for control in [
            11,
            1,
            u16::try_from(report.passes().len()).map_err(|_| Resource::Arithmetic)?,
            18,
        ] {
            writer.u16(control);
        }
        for owner in [input, output] {
            writer.raw(owner.identity().digest());
            writer.u64(owner.identity().canonical_length());
        }
        writer.raw(table.digest());
        writer.u64(table.encoded_length());
        writer.usize(rounds)?;
        writer.usize(POLICY11_MAX_ROUNDS)?;
        write_execution_tail_for_pass_count(
            &mut writer,
            report,
            map.digest(),
            execution,
            report.passes().len(),
        )?;
        if writer.cursor != bytes {
            return Err(Resource::Accounting);
        }
        Ok(Self { canonical, rounds })
    }
}

pub(crate) fn validate_fixedpoint_report(
    report: &PlironOptimizationReportV1,
) -> Result<usize, Resource> {
    let policy = FixedPolicy::MixedFixedpoint11;
    let rows = report.passes();
    if !policy.complete_pass_count(rows.len()) {
        return Err(Resource::Accounting);
    }
    let rounds = rows.len() / POLICY11_PASSES.len();
    let mut work = report.initial_graph_work();
    let mut epoch = rows[0].input_epoch().sequence();
    if epoch == 0 {
        return Err(Resource::Accounting);
    }
    for (index, round) in rows.chunks_exact(POLICY11_PASSES.len()).enumerate() {
        let changed = round.iter().any(|row| row.changed());
        if changed == (index + 1 == rounds) {
            return Err(Resource::Accounting);
        }
        for (row, expected) in round.iter().zip(POLICY11_PASSES) {
            let output_epoch = epoch
                .checked_add(usize::from(row.changed()) as u64)
                .ok_or(Resource::Arithmetic)?;
            if row.pass() != expected
                || row.input_graph_work() != work
                || row.input_epoch().sequence() != epoch
                || row.output_epoch().sequence() != output_epoch
                || (row.changed() && row.output_graph_work() > work)
                || (!row.changed() && row.output_graph_work() != work)
                || row.work_units()
                    != row
                        .output_graph_work()
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(work))
                        .ok_or(Resource::Arithmetic)?
            {
                return Err(Resource::Accounting);
            }
            work = row.output_graph_work();
            epoch = output_epoch;
        }
    }
    if work != report.final_graph_work()
        || work != report.final_graph_identity().tree_work()
        || epoch != report.final_graph_identity().epoch().sequence()
    {
        return Err(Resource::Accounting);
    }
    Ok(rounds)
}
