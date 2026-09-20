//! Additional inert guard observations in the existing D child, never admission.
use super::*;
use crate::production_ranked_projection_v1::scalar_emission_capture_v1::CapturedBoundSnapshotSourceV1;
use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
use fe2o3_lower_mir_kernel::{
    ProductionU32BoundSnapshotGuardRequestV1 as GuardRequest,
    ProductionU32GuardConsistencyV1 as GuardConsistency, ProductionU32GuardReportV1 as GuardReport,
    ProductionU32GuardRowV1 as GuardRow, ProductionU32GuardStorageV1 as GuardStorage,
};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) enum Outcome {
    NoCertificate,
    Unavailable(String),
    Joined {
        header: [u32; 2],
        bound: [u32; 2],
        condition: [u32; 4],
        body: [u32; 2],
        exit: [u32; 2],
        then_edge: [u32; 3],
        else_edge: [u32; 3],
    },
}

/// Existing D observations already borrow the genuine reports. This stages only
/// inert requests, performs no second import, and uses the new-family component
/// to independently rederive each requested actual source function. Persistent
/// report capacities remain reserved in the captured attachment throughout.
/// The component returns its unreserved receipt on the unchanged caller ledger.
pub(super) fn analyze<'s>(
    stage: &'s CapturedBoundSnapshotSourceV1,
    slots: &[Option<SourceLoopObservationV1<'s>>],
    budget: &mut Budget<'_>,
) -> Result<(GuardReport<'s>, GuardStorage), Failure> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let limits = fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1::default();
        budget.charge_work(1).map_err(|e| fail(Stage::Guard, e))?;
        if slots.len() > limits.rows || floor < stage.retained_storage() {
            return Err(fail(Stage::Guard, "request count or retained D floor"));
        }
        budget
            .reserve_storage(size_of::<Vec<GuardRequest<'_>>>())
            .map_err(|e| fail(Stage::Guard, e))?;
        let requested = slots
            .len()
            .checked_mul(size_of::<GuardRequest<'_>>())
            .ok_or_else(|| fail(Stage::Guard, "request byte overflow"))?;
        budget
            .reserve_storage(requested)
            .map_err(|e| fail(Stage::Guard, e))?;
        let mut requests = Vec::new();
        requests
            .try_reserve_exact(slots.len())
            .map_err(|e| fail(Stage::Guard, e))?;
        let capacity = requests
            .capacity()
            .checked_mul(size_of::<GuardRequest<'_>>())
            .ok_or_else(|| fail(Stage::Guard, "request capacity overflow"))?;
        budget
            .reserve_storage(
                capacity
                    .checked_sub(requested)
                    .ok_or_else(|| fail(Stage::Guard, "request capacity underflow"))?,
            )
            .map_err(|e| fail(Stage::Guard, e))?;
        for slot in slots {
            budget.charge_work(4).map_err(|e| fail(Stage::Guard, e))?;
            let observed = slot
                .as_ref()
                .ok_or_else(|| fail(Stage::Guard, "missing actual observation"))?;
            match observed.certificate_ordinal() {
                Some(ordinal) => {
                    if observed.report().certificates().get(ordinal).is_none()
                        || observed.outcome().is_none()
                    {
                        return Err(fail(
                            Stage::Guard,
                            "actual requested certificate occurrence",
                        ));
                    }
                    requests.push(GuardRequest::new(
                        observed.root().semantic_root(),
                        observed.report(),
                        ordinal,
                    ));
                }
                None => {
                    if !observed.report().certificates().is_empty() || observed.outcome().is_some()
                    {
                        return Err(fail(
                            Stage::Guard,
                            "missing-certificate observation mismatch",
                        ));
                    }
                }
            }
        }
        stage
            .capture()
            .analyze_u32_bound_snapshot_guard_consistency_v1(&requests, limits, budget)
            .map_err(|e| fail(Stage::Guard, e))
    }));
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < floor {
        drop(result);
        return Err(fail(Stage::Guard, "guard observation ledger/floor changed"));
    }
    // Request staging has dropped. Only its accepted scratch is released; the
    // component's returned report remains owned with an UNRESERVED receipt.
    budget
        .release_storage(budget.storage() - floor)
        .map_err(|e| fail(Stage::Guard, e))?;
    match result {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(fail(Stage::Guard, "guard observation panicked"))
        }
    }
}

pub(super) fn observe(
    stage: &CapturedBoundSnapshotSourceV1,
    actual: &SourceLoopObservationV1<'_>,
    rows: &[GuardRow<'_>],
    next: &mut usize,
    budget: &mut Budget<'_>,
) -> Result<Outcome, Failure> {
    budget.charge_work(8).map_err(|e| fail(Stage::Guard, e))?;
    let Some(ordinal) = actual.certificate_ordinal() else {
        if actual.outcome().is_some() || !actual.report().certificates().is_empty() {
            return Err(fail(Stage::Guard, "no-certificate guard row"));
        }
        return Ok(Outcome::NoCertificate);
    };
    let row = rows
        .get(*next)
        .ok_or_else(|| fail(Stage::Guard, "missing requested guard row"))?;
    *next = next
        .checked_add(1)
        .ok_or_else(|| fail(Stage::Guard, "guard row ordinal overflow"))?;
    if row.root() != actual.root().semantic_root()
        || row.function() != actual.report().function()
        || row.certificate_ordinal() != ordinal
    {
        return Err(fail(
            Stage::Guard,
            "guard row actual root/report/ordinal join",
        ));
    }
    match row.outcome() {
        GuardConsistency::Unavailable(reason) => Ok(Outcome::Unavailable(format!("{reason:?}"))),
        GuardConsistency::Joined(fact) => {
            let Some(Consistency::Joined(recurrence)) = actual.outcome() else {
                return Err(fail(
                    Stage::Guard,
                    "guard joined without original C recurrence",
                ));
            };
            budget.charge_work(12).map_err(|e| fail(Stage::Guard, e))?;
            if !std::ptr::eq(fact.source(), stage.capture().original())
                || fact.root() != row.root()
                || fact.function() != row.function()
                || fact.certificate_ordinal() != ordinal
                || fact.recurrence() != recurrence.recurrence()
                || fact.authorizes_compiler_transform()
            {
                return Err(fail(Stage::Guard, "actual N guard fact custody"));
            }
            let Definition::FunctionArgument { function, argument } = fact.bound() else {
                return Err(fail(Stage::Guard, "guard entry bound definition"));
            };
            let Definition::Result { operation, result } = fact.condition() else {
                return Err(fail(Stage::Guard, "guard Compare definition"));
            };
            Ok(Outcome::Joined {
                header: [
                    fact.then_edge().source.function.0,
                    fact.then_edge().source.block,
                ],
                bound: [function.0, argument],
                condition: [
                    operation.block.function.0,
                    operation.block.block,
                    operation.operation,
                    result,
                ],
                body: [fact.body().function.0, fact.body().block],
                exit: [fact.exit().function.0, fact.exit().block],
                then_edge: [
                    fact.then_edge().source.function.0,
                    fact.then_edge().source.block,
                    fact.then_edge().successor,
                ],
                else_edge: [
                    fact.else_edge().source.function.0,
                    fact.else_edge().source.block,
                    fact.else_edge().successor,
                ],
            })
        }
    }
}

pub(super) fn validate(row: &Row) -> Result<(), String> {
    match (&row.outcome, &row.guard) {
        (super::Outcome::NoCertificate, Outcome::NoCertificate) => Ok(()),
        (super::Outcome::Unavailable(_), Outcome::Unavailable(_)) => Ok(()),
        (
            super::Outcome::Joined {
                header: recurrence_header,
                ..
            },
            Outcome::Joined {
                header,
                bound,
                condition,
                body,
                exit,
                then_edge,
                else_edge,
            },
        ) if header == recurrence_header
            && bound[0] == header[0]
            && condition[0..2] == header[..]
            && condition[3] == 0
            && body[0] == header[0]
            && exit[0] == header[0]
            && body != exit
            && body != header
            && exit != header
            && then_edge == &[header[0], header[1], 0]
            && else_edge == &[header[0], header[1], 1] =>
        {
            Ok(())
        }
        _ => Err("guard outcome is not the exact observed source/N recurrence and branch".into()),
    }
}

#[test]
fn guard_protocol_rejects_wrong_axes_polarity_absent_and_unavailable_outcomes() -> Result<(), String>
{
    // Inert protocol data, not an admitted source/native owner or signed proof.
    let mut row = Row {
        root: 7,
        body: 9,
        root_identity: [4; 32],
        body_identity: [5; 32],
        report_semantic: [2; 32],
        ordinal: Some(0),
        certificates: 1,
        checked_additions: 1,
        dynamic_u32_bound: true,
        outcome: super::Outcome::Joined {
            header: [2, 3],
            parameter: "p".into(),
            initial: "i".into(),
            update: "u".into(),
            step: "s".into(),
            overflow: "o".into(),
            initial_edge: "e".into(),
            backedge: "b".into(),
            source_initialization: "i".into(),
            source_update: "u".into(),
            source_guard: "g".into(),
        },
        guard: Outcome::Joined {
            header: [2, 3],
            bound: [2, 0],
            condition: [2, 3, 4, 0],
            body: [2, 5],
            exit: [2, 6],
            then_edge: [2, 3, 0],
            else_edge: [2, 3, 1],
        },
    };
    validate(&row).unwrap();
    let valid = row.guard.clone();
    for mode in 0..11 {
        row.guard = valid.clone();
        let Outcome::Joined {
            header,
            bound,
            condition,
            body,
            exit,
            then_edge,
            else_edge,
        } = &mut row.guard
        else {
            return Err("synthetic protocol fixture lost its Joined variant".into());
        };
        match mode {
            0 => header[1] += 1,
            1 => bound[0] += 1,
            2 => condition[1] += 1,
            3 => condition[3] = 1,
            4 => body[0] += 1,
            5 => *exit = *body,
            6 => *body = *header,
            7 => *exit = *header,
            8 => then_edge[2] = 1,
            9 => else_edge[2] = 0,
            _ => else_edge[1] += 1,
        }
        assert!(validate(&row).is_err());
    }
    for replacement in [
        Outcome::NoCertificate,
        Outcome::Unavailable("GuardRecipe".into()),
    ] {
        row.guard = replacement;
        assert!(validate(&row).is_err());
    }
    row.outcome = super::Outcome::NoCertificate;
    row.guard = Outcome::NoCertificate;
    validate(&row).unwrap();
    row.guard = Outcome::Unavailable("GuardRecipe".into());
    assert!(validate(&row).is_err());
    Ok(())
}
