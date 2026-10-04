//! Inert synthetic model controls. No backend source/ledger/admission authority.
use super::*;
use crate::semantic_mir_v1::*;
use crate::semantic_option_dominance::enum_payload_admission_tests::payload_fixture;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[path = "semantic_retained_preparation_oracle_v1_tests.rs"]
mod oracle;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Denied {
    Work,
    Storage,
    Injected,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Work(usize),
    Storage(usize),
}
#[derive(Default, Debug)]
struct Meter {
    work: usize,
    storage: usize,
    events: Vec<Event>,
    work_limit: Option<usize>,
    storage_limit: Option<usize>,
    deny: Option<usize>,
    panic: Option<usize>,
    failed: bool,
}
impl SemanticEnumPayloadMeterV1 for Meter {
    type Error = Denied;
    fn charge_work(&mut self, amount: usize) -> Result<(), Denied> {
        assert!(!self.failed, "work after denial");
        self.events.push(Event::Work(amount));
        if self.panic == Some(self.events.len()) {
            panic!("injected meter unwind");
        }
        let next = self.work.checked_add(amount).unwrap();
        let error = if self.deny == Some(self.events.len()) {
            Some(Denied::Injected)
        } else if self.work_limit.is_some_and(|limit| next > limit) {
            Some(Denied::Work)
        } else {
            None
        };
        if let Some(error) = error {
            self.failed = true;
            return Err(error);
        }
        self.work = next;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Denied> {
        assert!(!self.failed, "storage after denial");
        self.events.push(Event::Storage(amount));
        if self.panic == Some(self.events.len()) {
            panic!("injected meter unwind");
        }
        let next = self.storage.checked_add(amount).unwrap();
        let error = if self.deny == Some(self.events.len()) {
            Some(Denied::Injected)
        } else if self.storage_limit.is_some_and(|limit| next > limit) {
            Some(Denied::Storage)
        } else {
            None
        };
        if let Some(error) = error {
            self.failed = true;
            return Err(error);
        }
        self.storage = next;
        Ok(())
    }
}
#[derive(Debug)]
enum Owner {
    Producers(SemanticOptionProducerPreparationV1),
    Option(SemanticOptionDominancePreparationV1),
    Enum(SemanticEnumPayloadDominancePreparationV1),
}
impl Owner {
    fn new(kind: usize) -> Self {
        match kind {
            0 => Self::Producers(SemanticOptionProducerPreparationV1::new()),
            1 => Self::Option(SemanticOptionDominancePreparationV1::new()),
            2 => Self::Enum(SemanticEnumPayloadDominancePreparationV1::new()),
            _ => unreachable!(),
        }
    }
    fn prepare<M: SemanticEnumPayloadMeterV1>(
        &mut self,
        f: &SemanticFunctionDeclV1,
        c: &[SemanticCallableDeclV1],
        t: &[SemanticTypeDeclV1],
        p: &[SemanticOptionProducerV1],
        m: &mut M,
    ) -> Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>> {
        match self {
            Self::Producers(s) => s.prepare_into(f, c, m),
            Self::Option(s) => s.prepare_into(f, p, m),
            Self::Enum(s) => s.prepare_into(f, t, m),
        }
    }
    fn complete(&self) -> bool {
        match self {
            Self::Producers(s) => s.completed().is_some(),
            Self::Option(s) => s.completed().is_some(),
            Self::Enum(s) => s.completed().is_some(),
        }
    }
}
fn inputs(
    kind: usize,
) -> (
    SemanticFunctionDeclV1,
    Vec<SemanticCallableDeclV1>,
    Vec<SemanticTypeDeclV1>,
    Vec<SemanticOptionProducerV1>,
) {
    if kind == 2 {
        let (t, f) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
        (f, Vec::new(), t, Vec::new())
    } else {
        let (f, c) = fixture();
        let p = semantic_option_producers_v1(&f, &c).unwrap();
        (f, c, Vec::new(), p)
    }
}
fn prepare<M: SemanticEnumPayloadMeterV1>(
    kind: usize,
    owner: &mut Owner,
    m: &mut M,
) -> Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>> {
    let (f, c, t, p) = inputs(kind);
    owner.prepare(&f, &c, &t, &p, m)
}
#[derive(Debug, Eq, PartialEq)]
enum Expected {
    Producers(Vec<SemanticOptionProducerV1>),
    Option(SemanticOptionDominanceV1),
    Enum(SemanticEnumPayloadDominanceV1),
}
fn frozen_expected(
    kind: usize,
    m: &mut Meter,
) -> Result<Expected, SemanticEnumPayloadMeteredErrorV1<Denied>> {
    let (f, c, t, p) = inputs(kind);
    let frame = match kind {
        0 => metered_fact_frame_storage_v1::<Meter, Vec<SemanticOptionProducerV1>>(),
        1 => metered_fact_frame_storage_v1::<Meter, SemanticOptionDominanceV1>(),
        _ => metered_fact_frame_storage_v1::<Meter, SemanticEnumPayloadDominanceV1>(),
    }
    .unwrap();
    let mut adapter = Adapter {
        original: m,
        failure: None,
    };
    let result = {
        let mut b = WorkBudgetV1 {
            used: 0,
            meter: Some(&mut adapter),
        };
        b.reserve_bytes(frame).and_then(|()| match kind {
            0 => oracle::producers(&f, &c, &mut b).map(Expected::Producers),
            1 => oracle::option_facts(&f, &p, &mut b).map(Expected::Option),
            _ => oracle::enum_facts(&f, &t, &mut b).map(Expected::Enum),
        })
    };
    match adapter.failure {
        Some(e) => Err(e),
        None => result.map_err(SemanticEnumPayloadMeteredErrorV1::Analysis),
    }
}
fn assert_completed(owner: &Owner, expected: &Expected) {
    match (owner, expected) {
        (Owner::Producers(s), Expected::Producers(e)) => {
            assert_eq!(s.completed(), Some(e.as_slice()))
        }
        (Owner::Option(s), Expected::Option(e)) => assert_eq!(s.completed(), Some(e)),
        (Owner::Enum(s), Expected::Enum(e)) => assert_eq!(s.completed(), Some(e)),
        _ => panic!("wrong test kind"),
    }
}

// Byte-exact fixture from the separately pinned original resource controls.
fn fixture() -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
    let (_, base) = payload_fixture(&[(0, 2), (1, 3)], 2, false);
    let source = SemanticSourceProvenanceV1::unavailable();
    let option = SemanticTypeIdV1::from_index(2);
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        Vec::new(),
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), Vec::new(), option).unwrap(),
            edge(SemanticEdgeRoleV1::CallReturn, 1),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([40; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([41; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([42; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([43; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([44; 32]),
        source,
        base.abi().clone(),
        base.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(45, Vec::new(), SemanticTerminatorKindV1::Call(call)),
            block(
                46,
                vec![base.blocks()[0].statements()[1].clone()],
                base.blocks()[0].terminator().kind().clone(),
            ),
            block(47, Vec::new(), SemanticTerminatorKindV1::Return),
            block(48, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap();
    let callables = vec![SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([50; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
            source,
            base.abi().clone(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice: option,
            index_witness: option,
            element: option,
            raw_index: option,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([55; 32]),
    }];
    (function, callables)
}

#[test]
fn fresh_owners_expose_no_data_or_allocation() {
    assert!(empty(&SemanticOptionProducerPreparationV1::new().producers));
    assert!(SemanticOptionDominancePreparationV1::new().pristine());
    assert!(SemanticEnumPayloadDominancePreparationV1::new().pristine());
    for kind in 0..3 {
        assert!(!Owner::new(kind).complete());
    }
}
#[test]
fn full_data_and_ordered_debits_equal_frozen_original_algorithms() {
    for kind in 0..3 {
        let mut old = Meter::default();
        let expected = frozen_expected(kind, &mut old).unwrap();
        let mut current = Meter::default();
        let mut owner = Owner::new(kind);
        prepare(kind, &mut owner, &mut current).unwrap();
        assert_completed(&owner, &expected);
        assert_eq!(current.work, old.work);
        assert_eq!(&current.events[1..], &old.events[1..]);
        assert!(
            matches!((old.events[0],current.events[0]),(Event::Storage(a),Event::Storage(b)) if b>a)
        );
        let retained = current.storage;
        drop(owner);
        assert_eq!(current.storage, retained);
    }
}
#[test]
fn facts_equal_both_unchanged_public_apis() {
    let (f, c) = fixture();
    let p = semantic_option_producers_v1(&f, &c).unwrap();
    let mut m = Meter::default();
    let mut owner = SemanticOptionDominancePreparationV1::new();
    owner.prepare_into(&f, &p, &mut m).unwrap();
    assert_eq!(
        owner.completed(),
        Some(&SemanticOptionDominanceV1::analyze(&f, &p).unwrap())
    );
    assert_eq!(
        owner.completed(),
        Some(
            &SemanticOptionDominanceV1::analyze_with_meter_v1(&f, &p, &mut Meter::default())
                .unwrap()
        )
    );
    for (cases, otherwise, second) in [
        (vec![(0, 1)], 1, false),
        (vec![(0, 1), (1, 1)], 2, false),
        (vec![(0, 1), (1, 2)], 1, false),
        (vec![(0, 1)], 2, false),
        (vec![(0, 1), (1, 2)], 3, false),
        (vec![(0, 1), (1, 2)], 3, true),
    ] {
        let (t, f) = payload_fixture(&cases, otherwise, second);
        let mut owner = SemanticEnumPayloadDominancePreparationV1::new();
        owner.prepare_into(&f, &t, &mut Meter::default()).unwrap();
        let expected = SemanticEnumPayloadDominanceV1::analyze(&f, &t).unwrap();
        assert_eq!(owner.completed(), Some(&expected));
        assert_eq!(
            owner.completed(),
            Some(
                &SemanticEnumPayloadDominanceV1::analyze_with_meter_v1(
                    &f,
                    &t,
                    &mut Meter::default()
                )
                .unwrap()
            )
        );
        assert!(!owner.completed().unwrap().grants_authority());
    }
}
#[test]
fn exact_and_one_short_work_and_storage_with_existing_floor() {
    for kind in 0..3 {
        let mut baseline = Meter::default();
        prepare(kind, &mut Owner::new(kind), &mut baseline).unwrap();
        for short in [None, Some(Denied::Work), Some(Denied::Storage)] {
            let mut m = Meter {
                work: 7,
                storage: 11,
                work_limit: Some(7 + baseline.work - usize::from(short == Some(Denied::Work))),
                storage_limit: Some(
                    11 + baseline.storage - usize::from(short == Some(Denied::Storage)),
                ),
                ..Default::default()
            };
            let mut owner = Owner::new(kind);
            let result = prepare(kind, &mut owner, &mut m);
            if let Some(error) = short {
                assert_eq!(result, Err(SemanticEnumPayloadMeteredErrorV1::Meter(error)));
                assert!(!owner.complete());
            } else {
                result.unwrap();
                assert!(owner.complete());
                assert_eq!(
                    (m.work, m.storage),
                    (7 + baseline.work, 11 + baseline.storage)
                );
            }
        }
    }
}
#[test]
fn every_debit_refuses_terminally_and_matches_old_dynamic_prefix() {
    for kind in 0..3 {
        let mut baseline = Meter::default();
        prepare(kind, &mut Owner::new(kind), &mut baseline).unwrap();
        for call in 1..=baseline.events.len() {
            let mut m = Meter {
                deny: Some(call),
                ..Default::default()
            };
            let mut owner = Owner::new(kind);
            assert_eq!(
                prepare(kind, &mut owner, &mut m),
                Err(SemanticEnumPayloadMeteredErrorV1::Meter(Denied::Injected))
            );
            assert_eq!(m.events.len(), call);
            assert_eq!(&m.events[..], &baseline.events[..call]);
            assert!(!owner.complete());
            let prior = format!("{owner:?}");
            let mut other = Meter::default();
            assert_eq!(
                prepare(kind, &mut owner, &mut other),
                Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()))
            );
            assert_eq!(prior, format!("{owner:?}"));
            assert!(other.events.is_empty());
            // Original frozen algorithm sees exactly the same dynamic debit index.
            let mut old = Meter {
                deny: Some(call),
                ..Default::default()
            };
            assert_eq!(
                frozen_expected(kind, &mut old),
                Err(SemanticEnumPayloadMeteredErrorV1::Meter(Denied::Injected))
            );
            assert_eq!(&old.events[1..], &m.events[1..]);
        }
    }
}
#[test]
fn every_debit_unwind_retains_identical_partial_physical_owner() {
    for kind in 0..3 {
        let mut baseline = Meter::default();
        prepare(kind, &mut Owner::new(kind), &mut baseline).unwrap();
        for call in 1..=baseline.events.len() {
            let mut denied = Owner::new(kind);
            let mut m = Meter {
                deny: Some(call),
                ..Default::default()
            };
            let _ = prepare(kind, &mut denied, &mut m);
            let mut unwound = Owner::new(kind);
            let mut p = Meter {
                panic: Some(call),
                ..Default::default()
            };
            assert!(
                catch_unwind(AssertUnwindSafe(|| prepare(kind, &mut unwound, &mut p))).is_err()
            );
            assert!(!unwound.complete());
            assert_eq!(format!("{unwound:?}"), format!("{denied:?}"));
            assert_eq!(capacities(&unwound), capacities(&denied));
            assert_eq!((p.work, p.storage), (m.work, m.storage));
            let before = p.storage;
            drop(unwound);
            assert_eq!(p.storage, before);
        }
    }
}
#[test]
fn success_is_one_shot_and_retains_original_completed_payload_on_retry() {
    for kind in 0..3 {
        let mut owner = Owner::new(kind);
        prepare(kind, &mut owner, &mut Meter::default()).unwrap();
        let before = format!("{owner:?}").replace("state: Complete", "state: Terminal");
        let mut m = Meter::default();
        assert_eq!(
            prepare(kind, &mut owner, &mut m),
            Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()))
        );
        assert_eq!(format!("{owner:?}"), before);
        assert!(!owner.complete());
        assert!(m.events.is_empty());
    }
}
#[test]
fn capacity_only_occupied_owners_are_rejected_without_dropping_vectors() {
    let (f, c, t, p) = inputs(0);
    let mut producer = SemanticOptionProducerPreparationV1::new();
    producer.producers.reserve_exact(1);
    let cap = producer.producers.capacity();
    let mut m = Meter::default();
    assert_eq!(
        producer.prepare_into(&f, &c, &mut m),
        Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()))
    );
    assert_eq!(producer.producers.capacity(), cap);
    assert!(m.events.is_empty());
    let _ = (t, p);
    for kind in 1..3 {
        let mut owner = Owner::new(kind);
        match &mut owner {
            Owner::Option(s) => s.dominators.pending_tree.reserve_exact(1),
            Owner::Enum(s) => s.availability.reserve_exact(1),
            _ => unreachable!(),
        }
        let before = format!("{owner:?}").replace("state: Fresh", "state: Terminal");
        assert_eq!(
            prepare(kind, &mut owner, &mut m),
            Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()))
        );
        assert_eq!(before, format!("{owner:?}"));
        assert!(m.events.is_empty());
    }
}
#[test]
fn all_dominator_scratch_fields_and_two_separate_queues_stay_owned() {
    let (f, c) = fixture();
    let p = semantic_option_producers_v1(&f, &c).unwrap();
    let mut s = SemanticOptionDominancePreparationV1::new();
    s.prepare_into(&f, &p, &mut Meter::default()).unwrap();
    let d = &s.dominators;
    assert!(!d.successors.is_empty());
    assert!(!d.predecessors.is_empty());
    assert!(!d.visited.is_empty());
    assert!(!d.postorder.is_empty());
    assert!(d.pending_rpo.capacity() > 0);
    assert!(!d.rpo_index.is_empty());
    assert!(!d.immediate.is_empty());
    assert!(!d.children.is_empty());
    assert!(d.pending_tree.capacity() > 0);
    assert!(d.pending_rpo.is_empty() && d.pending_tree.is_empty());
    assert!(d.preorder.is_empty() && d.subtree_end.is_empty());
    assert!(!s.completed().unwrap().dominator_preorder.is_empty());
    assert!(!s.completed().unwrap().dominator_subtree_end.is_empty());
    assert!(!s.definitions.is_empty() && !s.discriminants.is_empty());
}
#[test]
fn source_semantic_refusal_order_is_exact_and_partial_getter_stays_closed() {
    let (f, c) = fixture();
    let p = semantic_option_producers_v1(&f, &c).unwrap()[0];
    for producers in [
        vec![p, p],
        vec![SemanticOptionProducerV1::new(
            SemanticLocalIdV1::from_index(99),
            SemanticBlockIdV1::from_index(99),
        )],
        vec![SemanticOptionProducerV1::new(
            p.option_local(),
            SemanticBlockIdV1::from_index(99),
        )],
    ] {
        let expected =
            oracle::option_facts(&f, &producers, &mut WorkBudgetV1::default()).unwrap_err();
        let mut s = SemanticOptionDominancePreparationV1::new();
        let mut m = Meter::default();
        assert_eq!(
            s.prepare_into(&f, &producers, &mut m),
            Err(SemanticEnumPayloadMeteredErrorV1::Analysis(expected))
        );
        assert!(s.completed().is_none());
        assert!(!s.definitions.is_empty());
        assert!(m.storage > 0);
    }
    let (_, f) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    let expected = oracle::enum_facts(&f, &[], &mut WorkBudgetV1::default()).unwrap_err();
    let mut s = SemanticEnumPayloadDominancePreparationV1::new();
    assert_eq!(
        s.prepare_into(&f, &[], &mut Meter::default()),
        Err(SemanticEnumPayloadMeteredErrorV1::Analysis(expected))
    );
    assert!(s.completed().is_none());
    assert!(!s.discriminants.is_empty());
}
#[test]
fn option_duplicate_slot_replacement_precedes_refusal_and_target_append() {
    let (f, c) = fixture();
    let p = semantic_option_producers_v1(&f, &c).unwrap()[0];
    let mut s = SemanticOptionDominancePreparationV1::new();
    assert!(s.prepare_into(&f, &[p, p], &mut Meter::default()).is_err());
    assert_eq!(s.targets.len(), 1);
    assert_eq!(
        s.availability[p.option_local().index() as usize],
        Some(SemanticOptionAvailabilityV1(1))
    );
    assert!(s.completed().is_none());
}
#[test]
fn enum_target_append_precedes_availability_append_and_its_denial() {
    let (t, f) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    let mut measured = Meter::default();
    let mut full = SemanticEnumPayloadDominancePreparationV1::new();
    full.prepare_into(&f, &t, &mut measured).unwrap();
    let mut witnessed = false;
    for call in 1..=measured.events.len() {
        let mut s = SemanticEnumPayloadDominancePreparationV1::new();
        let mut m = Meter {
            deny: Some(call),
            ..Default::default()
        };
        let _ = s.prepare_into(&f, &t, &mut m);
        if s.targets.len() == 1 && s.availability.iter().all(Vec::is_empty) {
            witnessed = true;
            assert!(s.completed().is_none());
            assert!(s.targets.capacity() > 0);
        }
    }
    assert!(witnessed);
}
#[test]
fn legacy_local_work_limit_refuses_before_external_work_unchanged() {
    let (f, c) = fixture();
    let p = semantic_option_producers_v1(&f, &c).unwrap();
    let mut old = WorkBudgetV1 {
        used: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
        ..Default::default()
    };
    let expected = oracle::option_facts(&f, &p, &mut old).unwrap_err();
    let mut m = Meter::default();
    let mut adapter = Adapter {
        original: &mut m,
        failure: None,
    };
    let mut b = WorkBudgetV1 {
        used: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
        meter: Some(&mut adapter),
    };
    let mut s = SemanticOptionDominancePreparationV1::new();
    assert_eq!(retained_option(&f, &p, &mut s, &mut b), Err(expected));
    assert_eq!(m.work, 0);
    assert!(s.definitions.is_empty());
}
#[test]
fn definitions_and_dominator_intervals_equal_frozen_original_rows() {
    for kind in [1, 2] {
        let (f, _, _, _) = inputs(kind);
        let mut actual = Vec::new();
        let mut b = WorkBudgetV1::default();
        retained_definitions(&f, &mut actual, &mut b).unwrap();
        assert_eq!(
            actual,
            local_definition_counts(&f, &mut WorkBudgetV1::default()).unwrap()
        );
        let expected = DominatorIntervalsV1::analyze(&f, &mut WorkBudgetV1::default()).unwrap();
        let mut actual = RetainedDominators::default();
        RetainedDominators::prepare_into(&mut actual, &f, &mut b).unwrap();
        assert_eq!(actual.preorder, expected.preorder);
        assert_eq!(actual.subtree_end, expected.subtree_end);
        assert_eq!(actual.predecessors, expected.predecessors);
        assert_eq!(actual.entry, expected.entry);
    }
}
#[test]
fn source_vec_stays_attached_through_box_work_storage_error_and_unwind() {
    for panic in [false, true] {
        for call in [1, 2] {
            let mut values = Vec::with_capacity(4);
            values.push(7u64);
            let pointer = values.as_ptr();
            let capacity = values.capacity();
            let mut destination = None;
            let mut m = Meter {
                deny: (!panic).then_some(call),
                panic: panic.then_some(call),
                ..Default::default()
            };
            let mut adapter = Adapter {
                original: &mut m,
                failure: None,
            };
            let mut b = WorkBudgetV1 {
                used: 0,
                meter: Some(&mut adapter),
            };
            let result = catch_unwind(AssertUnwindSafe(|| {
                retained_box(&mut values, &mut destination, &mut b)
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap(),
                    Err(SemanticOptionDominanceErrorV1::Storage)
                );
            }
            assert_eq!(values.as_ptr(), pointer);
            assert_eq!(values.capacity(), capacity);
            assert_eq!(values, [7]);
            assert!(destination.is_none());
        }
    }
}
#[test]
fn earlier_boxed_candidate_stays_attached_when_later_box_refuses() {
    let mut first = Vec::with_capacity(4);
    first.push(1u64);
    let mut second = Vec::with_capacity(4);
    second.push(2u64);
    let (mut a, mut z) = (None, None);
    let mut m = Meter {
        deny: Some(3),
        ..Default::default()
    };
    let mut adapter = Adapter {
        original: &mut m,
        failure: None,
    };
    let mut b = WorkBudgetV1 {
        used: 0,
        meter: Some(&mut adapter),
    };
    retained_box(&mut first, &mut a, &mut b).unwrap();
    assert!(retained_box(&mut second, &mut z, &mut b).is_err());
    assert_eq!(a.as_deref(), Some([1u64].as_slice()));
    assert_eq!(second, [2]);
    assert!(z.is_none());
}
#[test]
fn nested_payload_is_not_dropped_before_owner_drops_after_box_refusal() {
    use std::cell::Cell;
    use std::rc::Rc;
    struct DropSpy(Rc<Cell<usize>>);
    impl Drop for DropSpy {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let mut values = Vec::with_capacity(4);
    values.push(vec![DropSpy(drops.clone()), DropSpy(drops.clone())]);
    let mut destination = None;
    let mut m = Meter {
        deny: Some(2),
        ..Default::default()
    };
    let mut adapter = Adapter {
        original: &mut m,
        failure: None,
    };
    let mut b = WorkBudgetV1 {
        used: 0,
        meter: Some(&mut adapter),
    };
    assert!(retained_box(&mut values, &mut destination, &mut b).is_err());
    assert_eq!(drops.get(), 0);
    drop(values);
    assert_eq!(drops.get(), 2);
    assert!(destination.is_none());
}
#[test]
fn partial_nested_fill_allocation_is_retained() {
    let mut values: Vec<Vec<u64>> = Vec::new();
    let mut m = Meter::default();
    let mut adapter = Adapter {
        original: &mut m,
        failure: None,
    };
    let mut b = WorkBudgetV1 {
        used: 0,
        meter: Some(&mut adapter),
    };
    retained_nested(&mut values, 3, &mut b).unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(values.capacity(), 3);
    for value in &values {
        assert!(empty(value));
    }
    assert_eq!(
        m.events,
        [
            Event::Work(3),
            Event::Work(0),
            Event::Storage(3 * size_of::<Vec<u64>>())
        ]
    );
}
#[derive(Default)]
struct LargeMeter {
    inner: Meter,
}
#[allow(clippy::result_large_err)]
impl SemanticEnumPayloadMeterV1 for LargeMeter {
    type Error = [u8; 8192];
    fn charge_work(&mut self, n: usize) -> Result<(), Self::Error> {
        self.inner.charge_work(n).map_err(|_| [1; 8192])
    }
    fn reserve_storage(&mut self, n: usize) -> Result<(), Self::Error> {
        self.inner.reserve_storage(n).map_err(|_| [2; 8192])
    }
}
#[test]
fn large_generic_error_headers_admit_before_first_dynamic_payload() {
    for kind in 0..3 {
        let frame = match kind {
            0 => retained_frame_storage::<
                LargeMeter,
                Vec<SemanticOptionProducerV1>,
                SemanticOptionProducerPreparationV1,
            >(),
            1 => retained_frame_storage::<
                LargeMeter,
                SemanticOptionDominanceV1,
                SemanticOptionDominancePreparationV1,
            >(),
            _ => retained_frame_storage::<
                LargeMeter,
                SemanticEnumPayloadDominanceV1,
                SemanticEnumPayloadDominancePreparationV1,
            >(),
        }
        .unwrap();
        assert!(frame > 8 * 8192);
        let mut m = LargeMeter {
            inner: Meter {
                storage: 11,
                storage_limit: Some(11 + frame - 1),
                ..Default::default()
            },
        };
        let mut owner = Owner::new(kind);
        assert!(
            matches!(prepare(kind,&mut owner,&mut m),Err(SemanticEnumPayloadMeteredErrorV1::Meter(e)) if e==[2;8192])
        );
        assert_eq!(m.inner.events, [Event::Storage(frame)]);
        assert_eq!((m.inner.work, m.inner.storage), (0, 11));
        assert!(!owner.complete());
    }
}
#[test]
fn frame_rows_use_full_owner_and_generic_error_types_without_new_padding() {
    let small = retained_frame_storage::<
        Meter,
        SemanticOptionDominanceV1,
        SemanticOptionDominancePreparationV1,
    >()
    .unwrap();
    let wide = retained_frame_storage::<
        LargeMeter,
        SemanticOptionDominanceV1,
        SemanticOptionDominancePreparationV1,
    >()
    .unwrap();
    assert!(wide > small + 6 * 8192);
    assert!(
        small
            > metered_fact_frame_storage_v1::<Meter, SemanticOptionDominanceV1>().unwrap()
                + size_of::<SemanticOptionDominancePreparationV1>()
    );
    assert_eq!(
        box_frame::<usize>(),
        size_of::<(
            &mut Vec<usize>,
            &mut Option<Box<[usize]>>,
            &mut WorkBudgetV1<'_>,
            usize,
            Vec<usize>,
            Box<[usize]>,
            Option<Box<[usize]>>,
            Result<(), SemanticOptionDominanceErrorV1>
        )>()
    );
}

fn dom_capacities(d: &RetainedDominators) -> Vec<usize> {
    let mut out = vec![
        d.successors.capacity(),
        d.predecessors.capacity(),
        d.visited.capacity(),
        d.postorder.capacity(),
        d.pending_rpo.capacity(),
        d.rpo_index.capacity(),
        d.immediate.capacity(),
        d.children.capacity(),
        d.preorder.capacity(),
        d.subtree_end.capacity(),
        d.pending_tree.capacity(),
    ];
    for nested in [&d.successors, &d.predecessors, &d.children] {
        out.extend(nested.iter().map(Vec::capacity));
    }
    out
}
fn capacities(owner: &Owner) -> Vec<usize> {
    match owner {
        Owner::Producers(s) => vec![s.producers.capacity()],
        Owner::Option(s) => {
            let mut v = dom_capacities(&s.dominators);
            v.extend([
                s.definitions.capacity(),
                s.discriminants.capacity(),
                s.availability.capacity(),
                s.targets.capacity(),
            ]);
            v.extend(s.discriminants.iter().map(Vec::capacity));
            v
        }
        Owner::Enum(s) => {
            let mut v = dom_capacities(&s.dominators);
            v.extend([
                s.definitions.capacity(),
                s.discriminants.capacity(),
                s.availability.capacity(),
                s.targets.capacity(),
            ]);
            v.extend(s.discriminants.iter().map(Vec::capacity));
            v.extend(s.availability.iter().map(Vec::capacity));
            v
        }
    }
}
#[test]
fn every_private_outer_allocation_slot_refuses_if_occupied_before_start() {
    let (f, c) = fixture();
    let p = semantic_option_producers_v1(&f, &c).unwrap();
    for field in 0..19 {
        let mut s = SemanticOptionDominancePreparationV1::new();
        match field {
            0 => s.definitions.reserve_exact(1),
            1 => s.discriminants.reserve_exact(1),
            2 => s.availability.reserve_exact(1),
            3 => s.targets.reserve_exact(1),
            4 => s.dominators.successors.reserve_exact(1),
            5 => s.dominators.predecessors.reserve_exact(1),
            6 => s.dominators.visited.reserve_exact(1),
            7 => s.dominators.postorder.reserve_exact(1),
            8 => s.dominators.pending_rpo.reserve_exact(1),
            9 => s.dominators.rpo_index.reserve_exact(1),
            10 => s.dominators.immediate.reserve_exact(1),
            11 => s.dominators.children.reserve_exact(1),
            12 => s.dominators.preorder.reserve_exact(1),
            13 => s.dominators.subtree_end.reserve_exact(1),
            14 => s.dominators.pending_tree.reserve_exact(1),
            15 => s.availability_box = Some(vec![None].into_boxed_slice()),
            16 => s.targets_box = Some(vec![SemanticBlockIdV1::from_index(0)].into_boxed_slice()),
            17 => s.preorder_box = Some(vec![0].into_boxed_slice()),
            18 => s.subtree_box = Some(vec![0].into_boxed_slice()),
            _ => unreachable!(),
        }
        let before = format!("{s:?}").replace("state: Fresh", "state: Terminal");
        let mut m = Meter::default();
        assert_eq!(
            s.prepare_into(&f, &p, &mut m),
            Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()))
        );
        assert_eq!(format!("{s:?}"), before);
        assert!(m.events.is_empty());
    }
}
#[test]
fn completed_candidate_cannot_be_rebranded_fresh() {
    let (f, c) = fixture();
    let p = semantic_option_producers_v1(&f, &c).unwrap();
    let mut s = SemanticOptionDominancePreparationV1::new();
    s.completed = Some(SemanticOptionDominanceV1::analyze(&f, &p).unwrap());
    let expected = s.completed.clone();
    let mut m = Meter::default();
    assert_eq!(
        s.prepare_into(&f, &p, &mut m),
        Err(SemanticEnumPayloadMeteredErrorV1::Analysis(occupied()))
    );
    assert_eq!(s.completed, expected);
    assert!(s.completed().is_none());
    assert!(m.events.is_empty());
}
#[test]
fn retained_box_success_transfers_without_extra_debits_for_exact_capacity() {
    let mut source = Vec::new();
    source.try_reserve_exact(2).unwrap();
    source.extend([3u32, 4]);
    assert_eq!(source.len(), source.capacity());
    let pointer = source.as_ptr();
    let mut destination = None;
    let mut m = Meter::default();
    let mut adapter = Adapter {
        original: &mut m,
        failure: None,
    };
    let mut b = WorkBudgetV1 {
        used: 0,
        meter: Some(&mut adapter),
    };
    retained_box(&mut source, &mut destination, &mut b).unwrap();
    assert!(empty(&source));
    assert_eq!(destination.as_deref(), Some([3u32, 4].as_slice()));
    assert_eq!(destination.as_ref().unwrap().as_ptr(), pointer);
    assert!(m.events.is_empty());
}

#[test]
fn real_enum_box_candidate_survives_later_shrink_refusal() {
    let (t, f) = payload_fixture(&[(0, 1), (1, 2)], 1, false);
    let mut measured = Meter::default();
    let mut full = SemanticEnumPayloadDominancePreparationV1::new();
    full.prepare_into(&f, &t, &mut measured).unwrap();
    assert_eq!(full.completed().unwrap().payload_targets.len(), 1);
    let mut witnessed = false;
    for call in 1..=measured.events.len() {
        let mut owner = SemanticEnumPayloadDominancePreparationV1::new();
        let mut m = Meter {
            deny: Some(call),
            ..Default::default()
        };
        let _ = owner.prepare_into(&f, &t, &mut m);
        if owner.availability_box.is_some() && owner.targets.len() == 1 {
            witnessed = true;
            assert!(owner.targets.capacity() > owner.targets.len());
            assert!(owner.targets_box.is_none() && owner.completed().is_none());
            assert_eq!(owner.availability_box.as_ref().unwrap()[1].len(), 1);
        }
    }
    assert!(witnessed);
}
#[test]
fn saturated_definition_count_and_reference_visitor_stay_original() {
    let (_, base) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut statements = vec![base.blocks()[0].statements()[0].clone(); 260];
    statements.push(base.blocks()[0].statements()[1].clone());
    let mut blocks = base.blocks().to_vec();
    blocks[0] = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([71; 32]),
        source,
        statements,
        base.blocks()[0].terminator().clone(),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([72; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([73; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([74; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([75; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([76; 32]),
        source,
        base.abi().clone(),
        base.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    let expected = local_definition_counts(&function, &mut WorkBudgetV1::default()).unwrap();
    assert_eq!(expected[1], u8::MAX);
    let mut actual = Vec::new();
    retained_definitions(&function, &mut actual, &mut WorkBudgetV1::default()).unwrap();
    assert_eq!(actual, expected);
}
