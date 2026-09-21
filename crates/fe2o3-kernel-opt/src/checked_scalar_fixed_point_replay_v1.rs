//! Adjacent semantic replay only. No optimizer invocation or producer recreation.
use super::*;
use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, check_canonical_kir_transition_v1};
use fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1;
use fe2o3_pliron::{PlironOptimizationReportV1, read_unauthenticated_policy3_execution_claim_v1};

fn roster(
    report: &PlironOptimizationReportV1,
    expected: &[Pass],
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    meter.work(
        expected
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if report.passes().len() != expected.len()
        || !report
            .passes()
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual.pass() == *expected)
    {
        return Err(Error::History);
    }
    Ok(())
}

pub(super) fn headers(
    input: &Owner,
    integer: &Integer,
    scalar: &Scalar,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    if !equal_bytes(
        input.canonical().canonical_bytes(),
        integer.native_input_audit_bytes(),
        meter,
    )? || !equal_bytes(
        integer.owner().canonical().canonical_bytes(),
        scalar.native_input_audit_bytes(),
        meter,
    )? {
        return Err(Error::History);
    }
    roster(integer.report(), &INTEGER_PASSES, meter)?;
    roster(scalar.report(), &SCALAR_PASSES, meter)?;
    meter.work(2 * (INTEGER_PASSES.len() + SCALAR_PASSES.len()) + 4 + 160)?;
    if !integer.map().matches_execution(integer.report())
        || !scalar.map().matches_execution(scalar.report())
        || integer.map().input_identity() != input.canonical().identity()
        || integer.map().output_identity() != integer.owner().canonical().identity()
        || scalar.map().input_identity() != integer.owner().canonical().identity()
        || scalar.map().output_identity() != scalar.owner().canonical().identity()
    {
        return Err(Error::History);
    }
    meter.derive(|b| {
        integer.execution().check_against(
            input,
            integer.owner(),
            integer.report(),
            integer.map(),
            b,
        )?;
        // This reads the genuine retained witness's framing/subjects. It does
        // not authenticate arbitrary bytes or convert a claim into execution.
        let _claim = read_unauthenticated_policy3_execution_claim_v1(
            integer.owner(),
            scalar.owner(),
            scalar.execution().canonical_bytes(),
            b,
        )
        .map_err(Error::ScalarExecution)?;
        Ok(())
    })
}

pub(super) fn pair(
    input: &Owner,
    output: &Owner,
    rows: CanonicalKirTransitionCandidateV1<'_>,
    meter: &mut Meter<'_, '_>,
) -> Result<()> {
    let (before, bs) =
        meter.derive(|b| CanonicalKirInventoryV1::derive(input, b).map_err(Error::Inventory))?;
    meter.reserve(bs.retained_storage())?;
    let (after, after_s) =
        meter.derive(|b| CanonicalKirInventoryV1::derive(output, b).map_err(Error::Inventory))?;
    meter.reserve(after_s.retained_storage())?;
    let retained = {
        let (_checked, receipt) = meter.derive(|b| {
            check_canonical_kir_transition_v1(&before, &after, rows, b).map_err(Error::Transition)
        })?;
        meter.reserve(receipt.retained_storage())?;
        receipt.retained_storage()
    };
    meter.release(retained)?;
    drop(after);
    meter.release(after_s.retained_storage())?;
    drop(before);
    meter.release(bs.retained_storage())?;
    Ok(())
}

pub(super) fn check(
    history: &CheckedScalarFixedPointOwnerV1,
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<()> {
    resources::scoped(budget, |meter| {
        meter.work(4)?;
        if history.rounds.is_empty() || history.rounds.len() > SCALAR_FIXED_POINT_MAX_ROUNDS_V1 {
            return Err(Error::History);
        }
        meter.work(
            history
                .rounds
                .len()
                .checked_mul(3)
                .ok_or(Resource::Arithmetic)?,
        )?;
        if retained(history.rounds.capacity(), &history.rounds)? != history.retained {
            return Err(Error::History);
        }
        meter.reserve(CHECK_SCRATCH)?;
        let expected = record(input, history.output(), history.rounds.len(), meter)?;
        if !equal_bytes(&expected, &history.execution.bytes, meter)? {
            return Err(Error::History);
        }
        let mut source = input;
        for (index, round) in history.rounds.iter().enumerate() {
            meter.work(1)?;
            if usize::from(round.ordinal) != index {
                return Err(Error::History);
            }
            headers(source, &round.integer, &round.scalar, meter)?;
            meter.derive(|b| {
                round
                    .integer
                    .map()
                    .check_against(source, round.integer.owner(), b)
                    .map_err(Error::Map)
            })?;
            pair(
                source,
                round.integer.owner(),
                round.integer.occurrences().candidate(),
                meter,
            )?;
            meter.derive(|b| {
                round
                    .scalar
                    .map()
                    .check_against(round.integer.owner(), round.output(), b)
                    .map_err(Error::Map)
            })?;
            pair(
                round.integer.owner(),
                round.output(),
                round.scalar.occurrences().candidate(),
                meter,
            )?;
            let terminal = equal_bytes(
                source.canonical().canonical_bytes(),
                round.output().canonical().canonical_bytes(),
                meter,
            )?;
            if terminal != (index + 1 == history.rounds.len()) {
                return Err(Error::History);
            }
            source = round.output();
        }
        meter.work(1)?;
        Ok(())
    })
}
