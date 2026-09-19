//! Pure registration/accounting checks, not source authentication or GPU evidence.

use super::*;
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use fe2o3_mir_model::semantic_mir_v1::{SemanticMirLimitsV1, SemanticMirResourceV1};
use std::cell::Cell;
use std::cmp::Ordering;

fn root(name: &str, target: u32) -> KernelRoot<u32> {
    KernelRoot {
        target,
        logical_name: name.into(),
        export_name: name.into(),
        generated_host_contract_identity: None,
        kernel_binding: None,
        frontend_contract: None,
        kernel_context_contract: None,
        reference_effect_binding: None,
        reference_target: None,
    }
}

fn record(name: &str, path: &str) -> ReferenceBindingRegistrationRecord<u32> {
    ReferenceBindingRegistrationRecord {
        registration_path: path.into(),
        item_name: format!(
            "{}{name}",
            reserved_fe2o3_symbols::REFERENCE_BINDING_REGISTRATION_PREFIX_V1,
        ),
        logical_name: name.into(),
        kernel: 7,
        reference: 11,
    }
}

fn locate(
    roots: &[KernelRoot<u32>],
    records: Vec<ReferenceBindingRegistrationRecord<u32>>,
    work: &mut SourceClosureWorkV1,
) -> Result<Vec<usize>, RegistrationError> {
    let names = reference_registration_maps_v1(roots, &records, work)?;
    records
        .into_iter()
        .map(|record| reference_registration_root_v1(&names, record, work).map(|(index, _)| index))
        .collect()
}

fn work_with_remaining(remaining: u64) -> SourceClosureWorkV1 {
    let mut work = SourceClosureWorkV1::default();
    work.charge(17).unwrap();
    let limit = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    let debit = limit
        .checked_sub(work.validation_work_for_test())
        .unwrap()
        .checked_sub(remaining)
        .unwrap();
    work.charge(usize::try_from(debit).unwrap()).unwrap();
    work
}

fn check_boundaries(
    roots: &[KernelRoot<u32>],
    records: &[ReferenceBindingRegistrationRecord<u32>],
    expected: Result<Vec<usize>, RegistrationError>,
) {
    // Input copies prepare independent fixtures; the production binder takes
    // ownership of already decoded records before this accounting starts.
    let run = |work: &mut SourceClosureWorkV1| locate(roots, records.to_vec(), work);
    let mut measured = SourceClosureWorkV1::default();
    assert_eq!(run(&mut measured), expected);
    let cost = measured.validation_work_for_test();
    assert!(cost > 0);
    let mut inherited = SourceClosureWorkV1::default();
    inherited.charge(17).unwrap();
    assert_eq!(run(&mut inherited), expected);
    assert_eq!(
        inherited.validation_work_for_test(),
        cost.checked_add(17).unwrap()
    );

    let mut exact = work_with_remaining(cost);
    assert_eq!(run(&mut exact), expected);
    assert_eq!(
        exact.validation_work_for_test(),
        SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork),
    );
    assert!(exact.charge(1).is_err());
    for remaining in [cost.checked_sub(1).unwrap(), 0] {
        let mut work = work_with_remaining(remaining);
        let error = run(&mut work).unwrap_err();
        assert!(error.reason.contains("ValidationWork"), "{error}");
        assert_eq!(error.registration_path, "<reference binding work>");
        let after = work.validation_work_for_test();
        let error = run(&mut work).unwrap_err();
        assert!(error.reason.contains("ValidationWork"), "{error}");
        assert!(work.validation_work_for_test() > after);
    }
}

#[test]
fn registration_names_keep_root_indices_and_input_order_with_exact_work() {
    let roots = [root("beta", 2), root("alpha", 1)];
    let records = [record("alpha", "fixture::a"), record("beta", "fixture::b")];
    check_boundaries(&roots, &records, Ok(vec![1, 0]));
    check_boundaries(&[], &[], Ok(vec![]));
    check_boundaries(&roots, &[], Ok(vec![]));
}

#[test]
fn duplicate_refusal_is_metered_and_keeps_lexical_then_first_record_precedence() {
    let mut malformed_orphan = record("orphan", "fixture::bad");
    malformed_orphan.item_name = "malformed".into();
    let records = [
        malformed_orphan,
        record("beta", "fixture::first_beta"),
        record("alpha", "fixture::first_alpha"),
        record("beta", "fixture::second_beta"),
        record("alpha", "fixture::second_alpha"),
    ];
    check_boundaries(
        &[],
        &records,
        Err(RegistrationError::new(
            "fixture::first_alpha",
            "duplicate safe Rust reference binding for one kernel",
        )),
    );
}

#[test]
fn orphan_refusal_is_metered_with_and_without_registered_roots() {
    let records = [record("absent", "fixture::orphan")];
    for roots in [Vec::new(), vec![root("alpha", 1), root("beta", 2)]] {
        check_boundaries(
            &roots,
            &records,
            Err(RegistrationError::new(
                "fixture::orphan",
                "orphan safe Rust reference binding has no registered kernel",
            )),
        );
    }
}

#[test]
fn malformed_item_name_still_precedes_orphan_refusal() {
    let mut malformed = record("absent", "fixture::bad_name");
    malformed.item_name = "malformed".into();
    check_boundaries(
        &[],
        &[malformed],
        Err(RegistrationError::new(
            "fixture::bad_name",
            "reference-binding item name disagrees with its logical kernel name",
        )),
    );
}

#[test]
fn long_names_and_duplicate_diagnostic_paths_consume_work_before_copying() {
    let long_name = "n".repeat(8_192);
    let long_path = "p".repeat(16_384);
    let roots = [root(&long_name, 7)];
    check_boundaries(&roots, &[record(&long_name, &long_path)], Ok(vec![0]));
    check_boundaries(
        &[],
        &[
            record("same", &long_path),
            record("same", "fixture::second"),
        ],
        Err(RegistrationError::new(
            &long_path,
            "duplicate safe Rust reference binding for one kernel",
        )),
    );

    let mut short = SourceClosureWorkV1::default();
    let mut long = SourceClosureWorkV1::default();
    let short_records = [record("same", "s"), record("same", "second")];
    let long_records = [record("same", &long_path), record("same", "second")];
    assert!(reference_registration_maps_v1::<u32>(&[], &short_records, &mut short).is_err());
    assert!(reference_registration_maps_v1::<u32>(&[], &long_records, &mut long).is_err());
    assert!(long.validation_work_for_test() > short.validation_work_for_test());
}

#[test]
fn search_accounting_is_checked_and_logarithmic() {
    assert_eq!(
        reference_registration_search_work_v1(0, usize::MAX),
        Some(0)
    );
    assert_eq!(reference_registration_search_work_v1(1, usize::MAX), None);
    assert_eq!(
        reference_registration_search_work_v1(usize::MAX, usize::MAX),
        None
    );
    let maximum = reference_registration_search_work_v1(usize::MAX, 0).unwrap();
    assert!(
        maximum
            <= usize::try_from(usize::BITS)
                .unwrap()
                .checked_mul(11)
                .unwrap()
    );
    assert!(reference_registration_search_work_v1(4_096, 0).unwrap() < 4_096);
    let mut work = SourceClosureWorkV1::default();
    let error = charge_reference_registration_work_v1(&mut work, None).unwrap_err();
    assert_eq!(error.reason, "reference registration work overflow");
    assert_eq!(error.registration_path, "<reference binding work>");
    assert_eq!(work.validation_work_for_test(), 0);
}

struct CountedKey<'a>(usize, &'a Cell<usize>);

impl PartialEq for CountedKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for CountedKey<'_> {}

impl PartialOrd for CountedKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CountedKey<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.1.set(self.1.get().checked_add(1).unwrap());
        self.0.cmp(&other.0)
    }
}

#[test]
fn search_envelope_bounds_pinned_std_insert_entry_and_lookup_comparisons() {
    let comparisons = Cell::new(0);
    for order in 0..3 {
        #[expect(
            clippy::mutable_key_type,
            reason = "the external comparison counter does not affect key ordering"
        )]
        let mut map = BTreeMap::new();
        for ordinal in 0_usize..1_024 {
            let value = match order {
                0 => ordinal,
                1 => 1_023_usize.checked_sub(ordinal).unwrap(),
                _ => ordinal.checked_mul(73).unwrap() % 1_024,
            };
            comparisons.set(0);
            let bound = reference_registration_search_work_v1(map.len(), 0).unwrap();
            map.insert(CountedKey(value.checked_mul(2).unwrap(), &comparisons), ());
            assert!(comparisons.get() <= bound);
        }
        for value in 0..2_049 {
            let bound = reference_registration_search_work_v1(map.len(), 0).unwrap();
            comparisons.set(0);
            map.get(&CountedKey(value, &comparisons));
            assert!(comparisons.get() <= bound);
            comparisons.set(0);
            map.entry(CountedKey(value, &comparisons)).or_insert(());
            assert!(comparisons.get() <= bound);
        }
    }
}
