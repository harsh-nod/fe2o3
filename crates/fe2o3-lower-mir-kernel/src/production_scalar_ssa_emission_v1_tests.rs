use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{cell::Cell, rc::Rc};

#[path = "production_scalar_ssa_bound_snapshot_query_v1_tests.rs"]
pub(super) mod bound_snapshot_tests;
#[path = "production_scalar_ssa_emission_fixture_v1_tests.rs"]
pub(super) mod fixture;
#[path = "production_scalar_ssa_emission_resources_v1_tests.rs"]
mod resource_tests;

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 17;
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

enum SetupReplayFault {
    Error,
    Panic,
    PanicPayload(std::sync::Arc<std::sync::atomic::AtomicUsize>),
}
std::thread_local! {
    static SETUP_REPLAY_FAULT: std::cell::RefCell<Option<SetupReplayFault>> =
        const { std::cell::RefCell::new(None) };
    static SETUP_REPLAY_OBSERVED: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

pub(super) fn after_replay_scratch_reserved_v1(budget: &Budget<'_>, capacity: usize) -> Result<()> {
    let Some(fault) = SETUP_REPLAY_FAULT.with(|slot| slot.replace(None)) else {
        return Ok(());
    };
    SETUP_REPLAY_OBSERVED.set(Some((budget.storage(), capacity)));
    assert!(
        capacity > 0,
        "the injected failure requires actual live replay scratch"
    );
    match fault {
        SetupReplayFault::Error => Err(Error::Mismatch("injected setup replay failure")),
        SetupReplayFault::Panic => panic!("injected setup replay panic"),
        SetupReplayFault::PanicPayload(count) => {
            struct Payload(std::sync::Arc<std::sync::atomic::AtomicUsize>);
            impl Drop for Payload {
                fn drop(&mut self) {
                    self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    panic!("setup panic payload destructor");
                }
            }
            std::panic::panic_any(Payload(count))
        }
    }
}

fn materialize(budget: &mut Budget<'_>) -> (ProductionScalarSsaEmissionOwnerV1, Certificate) {
    let (ssa, launch, certificate) = fixture::source(30);
    let owner = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
        ssa,
        launch,
        Default::default(),
        budget,
    )
    .unwrap();
    (owner, certificate)
}
fn query<'s>(
    owner: &'s ProductionScalarSsaEmissionOwnerV1,
    certificate: &Certificate,
    budget: &mut Budget<'_>,
) -> Result<ProductionU32RecurrenceConsistencyV1<'s>> {
    owner.with_u32_recurrences_v1(Default::default(), budget, |query, budget| {
        query.check_u32_certificate_v1(ROOT, certificate, budget)
    })
}
fn joined(
    value: ProductionU32RecurrenceConsistencyV1<'_>,
) -> ProductionU32RecurrenceConsistencyFactV1<'_> {
    match value {
        ProductionU32RecurrenceConsistencyV1::Joined(fact) => fact,
        other => panic!("expected exact genuine component recurrence, got {other:?}"),
    }
}

#[test]
fn actual_checked_u32_emission_joins_source_ssa_n_edges_and_inert_fact_can_escape() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, certificate) = materialize(&mut budget);
    assert_eq!(budget.storage(), FLOOR);
    budget
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    let before = budget.storage();
    let fact = joined(query(&owner, &certificate, &mut budget).unwrap());
    assert_eq!(budget.storage(), before);
    assert!(std::ptr::eq(fact.source(), owner.original()));
    assert_eq!(fact.certificate(), certificate);
    assert_eq!(fact.root(), ROOT);
    assert_eq!(fact.recurrence().scalar(), ScalarType::U32);
    assert_eq!(fact.recurrence().step_bits(), 1);
    assert!(fact.recurrence().overflow().is_some());
    assert!(!fact.authorizes_compiler_transform());
    assert!(!owner.grants_artifact_or_launch_authority());
    let retained = owner.retained_analysis_storage_v1();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn legacy_constructor_is_unchanged_and_explicitly_has_no_emission_attachment() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (ssa, launch, _) = fixture::source(30);
    let original = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        Default::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(
        original.scalar_ssa_emission_unavailable_v1(),
        ProductionScalarSsaEmissionUnavailableV1::NotCaptured
    );
    assert!(original.semantic_ssa().occurrences_v1().is_none());
}

#[test]
fn incoming_occurrence_receipt_is_not_recharged_or_transferred_twice() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (fresh, _) = materialize(&mut budget);
    let combined = fresh.retained_analysis_storage_v1();
    let captured = fresh.captured_occurrences.unwrap().retained_storage();
    drop(fresh);
    let (mut ssa, launch, certificate) = fixture::source(30);
    let receipt = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(receipt.retained_storage(), captured);
    budget.reserve_storage(captured).unwrap();
    let owner = ProductionScalarSsaEmissionOwnerV1::try_materialize_with_budget_v1(
        ssa,
        launch,
        Default::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR + captured);
    assert!(owner.captured_occurrences.is_none());
    assert_eq!(owner.retained_analysis_storage_v1() + captured, combined);
    budget
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    joined(query(&owner, &certificate, &mut budget).unwrap());
    let retained = owner.retained_analysis_storage_v1();
    drop(owner);
    budget.release_storage(retained + captured).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn missing_caller_reservation_is_an_accounting_error_not_unavailable() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, certificate) = materialize(&mut budget);
    assert!(matches!(
        query(&owner, &certificate, &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn foreign_source_certificate_and_root_are_errors_not_unavailable() {
    let (_, _, foreign) = fixture::source(31);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, certificate) = materialize(&mut budget);
    budget
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    let floor = budget.storage();
    assert!(matches!(
        query(&owner, &foreign, &mut budget),
        Err(Error::Mismatch("certificate actual source identity"))
    ));
    assert_eq!(budget.storage(), floor);
    let outcome =
        owner.with_u32_recurrences_v1(Default::default(), &mut budget, |query, budget| {
            query.check_u32_certificate_v1(
                SemanticFunctionIdV1::from_index(99),
                &certificate,
                budget,
            )
        });
    assert!(matches!(
        outcome,
        Err(Error::Mismatch("certificate actual root/body alias"))
    ));
    assert_eq!(budget.storage(), floor);
}

/// Hostiles alter private fixture rows only, never expose a constructor. Keep
/// capacity receipts accurate so extra-row checks cannot pass on accounting alone.
fn hostile(mut change: impl FnMut(&mut Sealed, &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (mut owner, _) = materialize(&mut budget);
    budget
        .reserve_storage(owner.retained_analysis_storage_v1())
        .unwrap();
    let previous = budget.storage();
    change(&mut owner.emission, &mut budget);
    let added = budget.storage() - previous;
    owner.emission.retained += added;
    owner.retained += added;
    let floor = budget.storage();
    let result = owner.with_u32_recurrences_v1(Default::default(), &mut budget, |_, _| Ok(()));
    assert!(
        matches!(result, Err(Error::Mismatch(_))),
        "hostile must fail coverage/binding: {result:?}"
    );
    assert_eq!(budget.storage(), floor);
}

#[test]
fn omitted_changed_duplicate_and_extra_source_rows_fail_complete_replay() {
    hostile(|sealed, _| {
        sealed.capture.expected.pop().unwrap();
    });
    hostile(|sealed, _| {
        sealed.capture.expected[0].variable = SsaVariableIdV1::new(99);
    });
    hostile(|sealed, _| {
        sealed.capture.expected[1] = sealed.capture.expected[0];
    });
    hostile(|sealed, budget| {
        let row = sealed.capture.expected[0];
        append(&mut sealed.capture.expected, row, budget).unwrap();
    });
}

#[test]
fn omitted_extra_reordered_and_wrong_physical_definition_rows_fail_replay() {
    hostile(|sealed, _| {
        sealed.capture.definitions.pop().unwrap();
    });
    hostile(|sealed, _| {
        sealed.capture.definitions.swap(0, 1);
    });
    hostile(|sealed, _| {
        sealed.capture.definitions[0].values[0] = ValueId(u32::MAX);
    });
    hostile(|sealed, _| {
        sealed.capture.definitions[0].definitions[0] = None;
    });
    hostile(|sealed, budget| {
        let row = sealed.capture.definitions[0];
        append(&mut sealed.capture.definitions, row, budget).unwrap();
        sealed.capture.functions[0].definitions.end += 1;
    });
}

#[test]
fn wrong_checked_field_pair_is_not_a_valid_scalar_mapping() {
    hostile(|sealed, _| {
        let row = sealed
            .capture
            .definitions
            .iter_mut()
            .find(|row| {
                matches!(
                    sealed.capture.expected[row.expected].shape,
                    Shape::CheckedAdd(_)
                )
            })
            .unwrap();
        row.definitions.swap(0, 1);
        row.values.swap(0, 1);
    });
}

#[test]
fn claiming_unsupported_placement_cannot_hide_existing_supported_rows() {
    hostile(|sealed, _| {
        sealed.capture.functions[0].unsupported_placement = true;
    });
}

#[test]
fn missing_duplicate_and_misdirected_edge_occurrences_fail_replay() {
    hostile(|sealed, _| {
        sealed.capture.edges.pop().unwrap();
    });
    hostile(|sealed, _| {
        sealed.capture.edges[0].argument = u32::MAX;
    });
    hostile(|sealed, _| {
        sealed.capture.edges[0].target = 4;
    });
    hostile(|sealed, _| {
        sealed.capture.edges[0].incoming = sealed.capture.edges[1].incoming;
    });
    hostile(|sealed, budget| {
        let row = sealed.capture.edges[0];
        append(&mut sealed.capture.edges, row, budget).unwrap();
        sealed.capture.functions[0].edges.end += 1;
    });
}

#[test]
fn missing_duplicate_alias_statement_and_site_index_rows_fail_replay() {
    hostile(|sealed, _| {
        sealed.aliases.pop().unwrap();
    });
    hostile(|sealed, _| {
        sealed.aliases[0].root = SemanticFunctionIdV1::from_index(99);
    });
    hostile(|sealed, budget| {
        let row = sealed.aliases[0];
        append(&mut sealed.aliases, row, budget).unwrap();
    });
    hostile(|sealed, _| {
        sealed.statements.pop().unwrap();
    });
    hostile(|sealed, _| {
        sealed.statements[0].ordinal = usize::MAX;
    });
    hostile(|sealed, budget| {
        let row = sealed.statements[0];
        append(&mut sealed.statements, row, budget).unwrap();
    });
    hostile(|sealed, _| {
        sealed.sites.pop().unwrap();
    });
    hostile(|sealed, _| {
        sealed.sites[0].expected = usize::MAX;
    });
    hostile(|sealed, budget| {
        let row = sealed.sites[0];
        append(&mut sealed.sites, row, budget).unwrap();
    });
}

#[test]
fn first_operand_index_replays_missing_wrong_duplicate_and_extra_occurrences() {
    hostile(|sealed, _| {
        sealed.first_operands.pop().unwrap();
    });
    hostile(|sealed, _| {
        sealed.first_operands[0].ordinal = usize::MAX;
    });
    hostile(|sealed, _| {
        sealed.first_operands[0].key.1 = u32::MAX;
    });
    hostile(|sealed, budget| {
        let row = sealed.first_operands[0];
        append(&mut sealed.first_operands, row, budget).unwrap();
    });
}

#[test]
fn foreign_inventory_cannot_replay_an_attachment_from_an_equal_source() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, _) = materialize(&mut budget);
    let (other, _) = materialize(&mut budget);
    budget
        .reserve_storage(owner.retained + other.retained)
        .unwrap();
    let (inventory, receipt) = Inventory::derive(other.original.executable(), &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(matches!(
        owner
            .emission
            .replay(&owner.original, &inventory, &mut budget),
        Err(Error::Mismatch("foreign inventory owner"))
    ));
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
}
