//! Actual-source qualification hook for the reservation-only context.
//! No strict origins, assertion mask, root recipe, ranked placement or admission.
use super::{
    CanonicalAssertionErrorV1, CanonicalAssertionSessionV1, CanonicalSourceAssertionFactsV1,
    bf16_nominal_recipe_resources_v1::with_nominal_recipe_resources_v1,
    with_nominal_canonical_facts_observation_v1,
};
use crate::production_ranked_projection_v1::{
    ProductionRankedProjectionErrorV1 as ProjectionError, SemanticFunctionIdV1,
    SemanticTerminatorKindV1,
    bf16_nominal_source_preparation_v1::{
        RichNominalSourceTablesV1, with_nominal_rich_source_preparation_v1,
    },
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as Error, CheckedBf16CallInstanceV1, ProductionPreRankedKirOwnerV1,
};
use std::{
    cell::Cell,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Result<T> = std::result::Result<T, Error>;
type ProjectionResult<T> = std::result::Result<T, ProjectionError>;
const HEADERS: usize = 8192 + 4 * 8192;
const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;
const REFUSAL: &str = "genuine recipe resource callback refusal";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Observe,
    Surplus,
    Error,
    Panic,
    ForeignFunction,
    MissingFunction,
    ForeignCorrespondence,
    Masked,
}
#[derive(Default)]
struct Events {
    rich: Cell<usize>,
    facts: Cell<usize>,
    context: Cell<usize>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    blocks: usize,
    materialized: usize,
    owned_credits: usize,
}
#[derive(Clone, Copy)]
struct Custody {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    work: usize,
    peak: usize,
}
impl Custody {
    fn take(budget: &Budget<'_>) -> Self {
        Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            work: budget.work(),
            peak: budget.peak_storage(),
        }
    }
    fn check(self, budget: &Budget<'_>, owned: usize) -> Result<()> {
        let floor = self.floor.checked_add(owned).ok_or(Resource::Arithmetic)?;
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < floor
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Resource::Arithmetic.into())
}
fn projection(error: ProjectionError) -> Error {
    // The source error is consumed/dropped here before any owning refund.
    match error {
        ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error)) => {
            Error::Resource(error)
        }
        ProjectionError::Incomplete(reason) | ProjectionError::Unsupported(reason) => {
            Error::Unavailable(reason)
        }
        ProjectionError::CanonicalAssertions(CanonicalAssertionErrorV1::Origin(_)) => {
            Error::Unavailable("genuine recipe entry correspondence refused")
        }
        _ => Error::Unavailable("genuine recipe canonical query refused"),
    }
}

#[allow(clippy::too_many_arguments)]
fn enter_context(
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    rich: &RichNominalSourceTablesV1<'_>,
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    mode: Mode,
    owned: &mut usize,
    pending: &mut Option<Vec<bool>>,
    events: &Events,
) -> ProjectionResult<Observation> {
    let payload = [7_u8; 8192];
    with_nominal_recipe_resources_v1(facts, rich, owned, move |context| {
        events.context.set(events.context.get() + 1);
        assert!(matches!(
            mode,
            Mode::Observe | Mode::Surplus | Mode::Error | Mode::Panic
        ));
        assert_eq!(payload[8191], 7);
        let actual =
            &owner.semantic_ssa().source_semantic().functions()[source.root().index() as usize];
        assert!(std::ptr::eq(context.function(), actual));
        assert!(std::ptr::eq(rich.function(), actual));
        let block = &actual.blocks()[source.call_block().index() as usize];
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            panic!("actual recipe source call terminal absent");
        };
        assert!(std::ptr::eq(call, source.source_call()));
        assert!(!actual.blocks().is_empty() && actual.blocks().len() <= 32);
        let blocks = actual.blocks().len();
        assert!(pending.is_none());
        context.with_resources(|resources| {
            resources.work(128)?;
            *pending = Some(resources.filled(blocks, false)?);
            Ok(())
        })?;
        // This is a materialization observation, NOT an assertion-decision mask
        // or checked-origin inventory. Query and resource borrows alternate.
        let mut materialized = 0usize;
        for index in 0..blocks {
            context.with_resources(|resources| resources.work(32))?;
            let present = context.with_facts(|facts| facts.is_materialized_block(index))?;
            context.with_resources(|resources| {
                resources.work(8)?;
                pending.as_mut().expect("paid pending bitmap")[index] = present;
                Ok(())
            })?;
            materialized += usize::from(present);
        }
        let bitmap = pending.as_ref().expect("paid pending bitmap");
        assert_eq!(bitmap.len(), blocks);
        assert_eq!(bitmap.capacity(), blocks);
        assert!(bitmap[actual.entry().index() as usize]);
        assert!(bitmap[source.call_block().index() as usize]);
        if mode != Mode::Observe {
            // Separate caller surplus, deliberately NOT charged to our counter.
            context.with_facts(|facts| {
                facts.reserve_scalar_private_storage_v1(23)?;
                facts.charge_private_array_work(17)
            })?;
        }
        match mode {
            Mode::Error => Err(ProjectionError::Incomplete(REFUSAL)),
            Mode::Panic => panic!("genuine recipe resource unwind"),
            _ => Ok(Observation {
                blocks,
                materialized,
                owned_credits: 0,
            }),
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn inspect_facts(
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    rich: &RichNominalSourceTablesV1<'_>,
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    mode: Mode,
    owned: &mut usize,
    pending: &mut Option<Vec<bool>>,
    events: &Events,
    original: Custody,
) -> Result<Observation> {
    events.facts.set(events.facts.get() + 1);
    assert_eq!(facts.budget as *const Budget<'_> as usize, original.slot);
    assert!(facts.budget.work_ledger_identity_v1() == original.ledger);
    assert!(std::ptr::eq(facts.owner, owner));
    assert!(facts.report.belongs_to(inventory));
    assert_eq!(facts.correspondence_owner, source.root());
    assert_eq!(facts.semantic_function, source.root());
    let result = match mode {
        Mode::ForeignFunction | Mode::MissingFunction | Mode::ForeignCorrespondence => {
            // Negative-only shadow of real sealed fields. It borrows the SAME
            // Budget exclusively, is disposed here, and never mutates real facts.
            let mut shadow = CanonicalSourceAssertionFactsV1 {
                owner: facts.owner,
                origins: facts.origins,
                report: facts.report,
                budget: &mut *facts.budget,
                correspondence_owner: if mode == Mode::ForeignCorrespondence {
                    source.helper()
                } else {
                    facts.correspondence_owner
                },
                semantic_function: match mode {
                    Mode::ForeignFunction => source.helper(),
                    Mode::MissingFunction => SemanticFunctionIdV1::from_index(u32::MAX),
                    _ => facts.semantic_function,
                },
                masked: None,
            };
            enter_context(
                &mut shadow,
                rich,
                owner,
                source,
                mode,
                owned,
                pending,
                events,
            )
        }
        Mode::Masked => {
            // Use the existing real masked-table constructor on the original
            // ledger, not an invented bool table or detached authority object.
            let mut session = CanonicalAssertionSessionV1 {
                owner: facts.owner,
                origins: facts.origins,
                report: facts.report,
                budget: &mut *facts.budget,
            };
            session.with_source_masked_assertions_v1(source.root(), source.root(), |masked| {
                enter_context(masked, rich, owner, source, mode, owned, pending, events)
            })
        }
        _ => enter_context(facts, rich, owner, source, mode, owned, pending, events),
    };
    result.map_err(projection)
}

#[allow(clippy::too_many_arguments)]
fn run(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
    mode: Mode,
    events: &Events,
) -> Result<Observation> {
    // The genuine owner floor is checked BEFORE adding our observation header.
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget);
        // Mandatory on EVERY run, including standalone Observe: admit the
        // fixed 8192-byte payload initialization and bounded closure transfers
        // before enter_context constructs it. External control prepayments
        // remain separate conservative harness charges, not a substitute.
        let run_work = 4usize
            .checked_mul(8192)
            .and_then(|bytes| bytes.checked_add(512))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(run_work)?;
        budget.reserve_storage(HEADERS)?;
        let mut owned = HEADERS;
        // Physical owner and credit counter exist outside BOTH nested factories.
        let mut pending: Option<Vec<bool>> = None;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_nominal_rich_source_preparation_v1(
                owner,
                inventory,
                source.root(),
                source.root(),
                source.call_block(),
                source.source_call(),
                budget,
                |rich, budget| {
                    events.rich.set(events.rich.get() + 1);
                    with_nominal_canonical_facts_observation_v1(
                        owner,
                        inventory,
                        source.root(),
                        source.root(),
                        source.call_block(),
                        source.source_call(),
                        budget,
                        |facts| {
                            inspect_facts(
                                facts,
                                rich,
                                owner,
                                source,
                                inventory,
                                mode,
                                &mut owned,
                                &mut pending,
                                events,
                                before,
                            )
                        },
                    )
                },
            )
        }));
        // All rich/facts/context scopes and their internal postflights are over.
        // A parked bitmap may still be alive; its credit has NEVER been refunded.
        let mut result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(Error::CallbackPanicked)
            }
        };
        drop(pending.take());
        // Query errors are fixed Copy values; projection errors and all panic
        // payloads were consumed before this point. Refuse custody loss without
        // attempting to restore a foreign ledger or undercut floor.
        before.check(budget, owned)?;
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            result = Err(Resource::Accounting.into());
        }
        if let Ok(observation) = &mut result {
            observation.owned_credits = owned;
        }
        budget.release_storage(owned)?;
        result
    })
}

pub(in crate::production_ranked_projection_v1) fn observe_nominal_recipe_resources_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let before = Custody::take(budget);
    let events = Events::default();
    let observed = run(owner, source, inventory, budget, Mode::Observe, &events)?;
    before.check(budget, 0)?;
    assert_eq!(budget.storage(), before.floor);
    assert_eq!(
        (events.rich.get(), events.facts.get(), events.context.get()),
        (1, 1, 1)
    );
    assert!(observed.materialized > 0 && observed.materialized <= observed.blocks);
    // Capped test-harness telemetry only; not compiler-ledger formatting/I/O,
    // an assertion proof, checked-origin completeness, or a recipe admission.
    eprintln!(
        "fe2o3-recipe-resource-observation-v1 root={} call_block={} permutation={:?} blocks={} materialized={} owned_credits={} contexts=1",
        source.root().index(),
        source.call_block().index(),
        source.return_permutation(),
        observed.blocks,
        observed.materialized,
        observed.owned_credits,
    );
    Ok(())
}

fn with_headers(
    budget: &mut Budget<'_>,
    bytes: usize,
    body: impl FnOnce(&mut Budget<'_>) -> Result<()>,
) -> Result<()> {
    budget.reserve_storage(bytes)?;
    let before = Custody::take(budget);
    let outcome = catch_unwind(AssertUnwindSafe(|| body(budget)));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    before.check(budget, 0)?;
    budget.release_storage(bytes)?;
    result
}

pub(in crate::production_ranked_projection_v1) fn nominal_recipe_resources_controls_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, original, |original| {
        with_headers(original, HEADERS, |original| {
            for mode in [Mode::Surplus, Mode::Error, Mode::Panic] {
                original.charge_work(4 * 8192 + 256)?;
                let before = Custody::take(original);
                let events = Events::default();
                let result = run(owner, source, inventory, original, mode, &events);
                assert_eq!(
                    (events.rich.get(), events.facts.get(), events.context.get()),
                    (1, 1, 1)
                );
                match mode {
                    Mode::Surplus => assert!(result.is_ok()),
                    Mode::Error => assert_eq!(result, Err(Error::Unavailable(REFUSAL))),
                    _ => assert_eq!(result, Err(Error::CallbackPanicked)),
                }
                before.check(original, 0)?;
                assert_eq!(original.storage(), before.floor + 23);
                assert_eq!(
                    (original.failed_work(), original.failed_storage()),
                    (None, None)
                );
                // The run released only its own credits, not this caller surplus.
                original.release_storage(23)?;
            }
            for (mode, expected) in [
                (
                    Mode::ForeignFunction,
                    "nominal recipe tables do not borrow the actual source function",
                ),
                (
                    Mode::MissingFunction,
                    "nominal recipe source function absent",
                ),
                (
                    Mode::ForeignCorrespondence,
                    "genuine recipe entry correspondence refused",
                ),
                (
                    Mode::Masked,
                    "nominal recipe resources require unmasked real facts",
                ),
            ] {
                original.charge_work(4 * 8192 + 256)?;
                let before = Custody::take(original);
                let events = Events::default();
                let result = run(owner, source, inventory, original, mode, &events);
                assert_eq!(
                    (events.rich.get(), events.facts.get(), events.context.get()),
                    (1, 1, 0)
                );
                assert_eq!(result, Err(Error::Unavailable(expected)));
                before.check(original, 0)?;
                assert_eq!(original.storage(), before.floor);
                assert_eq!(
                    (original.failed_work(), original.failed_storage()),
                    (None, None)
                );
            }
            let occurrence = owner
                .semantic_ssa()
                .occurrence_storage()
                .ok_or(Error::Unavailable(
                    "genuine recipe occurrence storage absent",
                ))?
                .retained_storage();
            let floor = add(
                add(owner.retained_analysis_storage_v1(), occurrence)?,
                inventory_storage,
            )?;
            let short = floor.checked_sub(1).ok_or(Resource::Arithmetic)?;
            // Independent ledgers are NEGATIVE ONLY. Prepay the complete finite
            // probe budget and scratch on ORIGINAL; unchanged 8 MiB caps.
            with_headers(original, PROBE_SCRATCH, |original| {
                for mode in 0..3 {
                    original.charge_work(PROBE_WORK + 4 * 8192 + 256)?;
                    let mut work = Work::new(if mode == 1 { 0 } else { PROBE_WORK });
                    let mut probe = Budget::new(
                        &mut work,
                        if mode == 2 {
                            floor
                        } else {
                            add(floor, PROBE_SCRATCH)?
                        },
                    );
                    let incoming = if mode == 0 { short } else { floor };
                    probe.reserve_storage(incoming)?;
                    let events = Events::default();
                    let result = run(owner, source, inventory, &mut probe, Mode::Observe, &events);
                    assert_eq!(
                        (events.rich.get(), events.facts.get(), events.context.get()),
                        (0, 0, 0)
                    );
                    assert_eq!(probe.storage(), incoming);
                    match mode {
                        0 => {
                            assert_eq!(result, Err(Resource::Accounting.into()));
                            assert_eq!((probe.failed_work(), probe.failed_storage()), (None, None));
                        }
                        1 => {
                            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                            assert!(probe.failed_work().is_some());
                        }
                        _ => {
                            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                            assert!(probe.failed_storage().is_some());
                        }
                    }
                }
                Ok(())
            })
        })
    })
}

#[test]
fn genuine_recipe_resource_control_header_covers_fixed_stack_payloads() {
    assert!(
        4 * 8192
            + 4 * size_of::<Custody>()
            + 4 * size_of::<Events>()
            + 4 * size_of::<Observation>()
            + 4 * size_of::<Option<Vec<bool>>>()
            + 8 * size_of::<Result<Observation>>()
            + 2 * size_of::<Budget<'static>>()
            + 2 * size_of::<Work>()
            + 2048
            <= HEADERS
    );
}
