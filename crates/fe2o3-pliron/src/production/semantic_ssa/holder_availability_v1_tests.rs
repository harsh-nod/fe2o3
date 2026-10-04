use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
type CaptureError = crate::ProductionSemanticSsaOccurrenceErrorV1;

struct Ledger<'a, 'b>(&'a mut Budget<'b>);
impl Meter for Ledger<'_, '_> {
    type Error = CaptureError;
    fn work(&mut self, units: usize) -> Result<(), Self::Error> {
        self.0.charge_work(units).map_err(Into::into)
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.0.reserve_storage(bytes).map_err(Into::into)
    }
    fn release(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.0.release_storage(bytes).map_err(Into::into)
    }
}

fn v(n: u32) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(n)
}
fn block(events: Vec<SsaEventV1>, edges: Vec<SsaEdgeInputV1>) -> SsaBlockInputV1 {
    SsaBlockInputV1::new(events, edges)
}
fn edge(target: u32, definitions: &[u32]) -> SsaEdgeInputV1 {
    SsaEdgeInputV1::new(
        SsaEdgeRoleV1::new(1),
        SsaBlockIdV1::new(target),
        definitions.iter().copied().map(v).collect(),
    )
}
fn update(block: usize, use_event: usize, local: u32) -> FieldUpdate {
    FieldUpdate {
        block,
        use_event,
        define_event: use_event + 1,
        local: v(local),
    }
}
fn pair(local: u32) -> Vec<SsaEventV1> {
    vec![SsaEventV1::Use(v(local)), SsaEventV1::Define(v(local))]
}

#[derive(Default)]
struct Probe {
    work: usize,
    storage: usize,
    peak: usize,
}
impl Meter for Probe {
    type Error = ProductionSemanticSsaErrorV1;
    fn work(&mut self, units: usize) -> Result<(), Self::Error> {
        self.work += units;
        Ok(())
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.storage += bytes;
        self.peak = self.peak.max(self.storage);
        Ok(())
    }
    fn release(&mut self, bytes: usize) -> Result<(), Self::Error> {
        self.storage = self.storage.checked_sub(bytes).ok_or_else(mismatch)?;
        Ok(())
    }
}

fn run(
    blocks: &[SsaBlockInputV1],
    entries: &[SsaVariableIdV1],
    updates: &[FieldUpdate],
    variables: usize,
    limits: ProductionSemanticSsaLimitsV1,
) -> (
    Result<(), ProductionSemanticSsaErrorV1>,
    Vec<bool>,
    SemanticSsaAuxiliaryResourcesV1,
    Probe,
) {
    let mut meter = Probe {
        storage: 37,
        peak: 37,
        ..Probe::default()
    };
    let mut account = Account::new(SemanticFunctionIdV1::from_index(0), limits);
    let mut marks = account.markers(updates.len(), 0, &mut meter).unwrap();
    marks.extend_from_slice(updates);
    let mut promoted = vec![true; variables];
    let result = refine(
        SsaBlockIdV1::new(0),
        &mut promoted,
        entries,
        blocks,
        &marks,
        &mut account,
        &mut meter,
    );
    let resources = account.finish(marks, &mut meter).unwrap();
    assert_eq!(meter.storage, 37);
    (result, promoted, resources, meter)
}

#[test]
fn holder_sparse_whole_definition_entry_kill_and_field_definition_are_distinct() {
    use SsaEventV1::{Define, Kill};
    for (before, initial, expected) in [
        (vec![], false, false),
        (vec![], true, true),
        (vec![Define(v(0))], false, true),
        (vec![Kill(v(0))], true, false),
        (vec![Kill(v(0)), Define(v(0))], true, true),
    ] {
        let offset = before.len();
        let mut events = before;
        events.extend(pair(0));
        events.extend(pair(0));
        let entries = if initial { vec![v(0)] } else { vec![] };
        let (result, promoted, _, _) = run(
            &[block(events, vec![])],
            &entries,
            &[update(0, offset, 0), update(0, offset + 2, 0)],
            1,
            ProductionSemanticSsaLimitsV1::default(),
        );
        result.unwrap();
        assert_eq!(promoted, [expected]);
    }
}

#[test]
fn holder_sparse_all_paths_normal_return_edges_and_backedges_are_checked() {
    for missing in [false, true] {
        let blocks = [
            block(vec![], vec![edge(1, &[]), edge(2, &[])]),
            block(vec![], vec![edge(3, &[0])]),
            block(vec![], vec![edge(3, if missing { &[] } else { &[0] })]),
            block(pair(0), vec![edge(4, &[])]),
            block(vec![SsaEventV1::Kill(v(0))], vec![edge(3, &[0])]),
        ];
        let (result, promoted, _, _) = run(
            &blocks,
            &[],
            &[update(3, 0, 0)],
            1,
            ProductionSemanticSsaLimitsV1::default(),
        );
        result.unwrap();
        assert_eq!(promoted, [!missing]);
    }
    for reset_on_backedge in [false, true] {
        let blocks = [
            block(vec![SsaEventV1::Define(v(0))], vec![edge(1, &[])]),
            block(pair(0), vec![edge(2, &[]), edge(2, &[])]),
            block(
                if reset_on_backedge {
                    vec![SsaEventV1::Kill(v(0))]
                } else {
                    vec![]
                },
                vec![edge(1, &[])],
            ),
        ];
        let (result, promoted, _, _) = run(
            &blocks,
            &[],
            &[update(1, 0, 0)],
            1,
            ProductionSemanticSsaLimitsV1::default(),
        );
        result.unwrap();
        assert_eq!(promoted, [!reset_on_backedge]);
    }
}

#[test]
fn holder_sparse_unreachable_updates_and_failure_only_kills_do_not_demote() {
    let blocks = [
        block(vec![SsaEventV1::Kill(v(0))], vec![edge(1, &[])]).with_terminal_failure_start(0),
        block(pair(0), vec![]),
        block(pair(0), vec![]),
    ];
    let (result, promoted, _, _) = run(
        &blocks,
        &[v(0)],
        &[update(1, 0, 0), update(2, 0, 0)],
        1,
        ProductionSemanticSsaLimitsV1::default(),
    );
    result.unwrap();
    assert_eq!(promoted, [true]);
    let (result, promoted, _, _) = run(
        &[block(vec![], vec![]), block(pair(0), vec![])],
        &[],
        &[update(1, 0, 0)],
        1,
        ProductionSemanticSsaLimitsV1::default(),
    );
    result.unwrap();
    assert_eq!(promoted, [true]);
}

#[test]
fn holder_sparse_marker_mismatch_never_classifies_an_arbitrary_use_define_pair() {
    for fault in 0..8 {
        let mut blocks = vec![block(pair(0), vec![])];
        let mut updates = vec![update(0, 0, 0)];
        match fault {
            0 => updates[0].block = 1,
            1 => updates[0].use_event = 1,
            2 => updates[0].define_event = 0,
            3 => updates[0].local = v(1),
            4 => {
                blocks[0] = block(
                    vec![SsaEventV1::Kill(v(0)), SsaEventV1::Define(v(0))],
                    vec![],
                )
            }
            5 => blocks[0] = block(pair(0), vec![]).with_terminal_failure_start(1),
            6 => updates.push(updates[0]),
            7 => blocks[0] = block(pair(0), vec![edge(2, &[])]),
            _ => unreachable!(),
        }
        let (result, promoted, _, _) = run(
            &blocks,
            &[],
            &updates,
            1,
            ProductionSemanticSsaLimitsV1::default(),
        );
        assert_eq!(
            result,
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch),
            "fault {fault}"
        );
        assert_eq!(promoted, [true]);
    }
}

fn limit(work: usize, storage: usize) -> ProductionSemanticSsaLimitsV1 {
    let old = SsaPlannerLimitsV1::default();
    ProductionSemanticSsaLimitsV1::new(
        SsaPlannerLimitsV1::try_new(
            old.max_variables(),
            old.max_blocks(),
            old.max_edges(),
            old.max_events(),
            old.max_edge_definitions(),
            old.max_output_items(),
            storage,
            work,
        )
        .unwrap(),
    )
}

#[test]
fn holder_sparse_prepaid_independent_exact_and_one_short_resources() {
    #[allow(dead_code)]
    struct Marker {
        block: usize,
        read: usize,
        write: usize,
        local: SsaVariableIdV1,
    }
    #[allow(dead_code)]
    struct Header {
        function: SemanticFunctionIdV1,
        limits: ProductionSemanticSsaLimitsV1,
        work: usize,
        live: usize,
        peak: usize,
        baseline: [usize; 2],
        expected: usize,
    }
    #[allow(dead_code)]
    enum Transfer {
        Update,
        Present,
        Absent,
    }
    #[allow(dead_code)]
    struct Event {
        local: SsaVariableIdV1,
        block: usize,
        event: usize,
        transfer: Transfer,
    }
    #[allow(dead_code)]
    struct Fold {
        local: SsaVariableIdV1,
        block: usize,
        end: Option<bool>,
        need: bool,
        invalid: bool,
    }
    for n in [1_usize, 2, 8, 64] {
        let blocks = vec![block(
            (0..n).flat_map(|id| pair(id as u32)).collect(),
            vec![],
        )];
        let updates: Vec<_> = (0..n).map(|id| update(0, 2 * id, id as u32)).collect();
        let log = |value: usize| usize::BITS as usize - value.leading_zeros() as usize + 1;
        let work = 8
            + 6 * n
            + 3
            + 16
            + 12 * n
            + 16 * n * log(n)
            + 16 * (2 * n) * log(2 * n)
            + 2 * n * (12 + log(n))
            + n * (12 + 3 * log(2 * n));
        let bytes = size_of::<Header>()
            + size_of::<Vec<Marker>>()
            + n * size_of::<Marker>()
            + size_of::<Vec<SsaVariableIdV1>>()
            + n * size_of::<SsaVariableIdV1>()
            + size_of::<Vec<Event>>()
            + 2 * n * size_of::<Event>()
            + size_of::<Vec<Fold>>()
            + 2 * n * size_of::<Fold>()
            + size_of::<Vec<u8>>()
            + 1
            + size_of::<Vec<(usize, bool)>>()
            + 2 * size_of::<(usize, bool)>();
        let words = bytes.div_ceil(size_of::<usize>());
        let (result, promoted, resources, meter) =
            run(&blocks, &[], &updates, n, limit(work, words));
        result.unwrap();
        assert_eq!(promoted, vec![false; n]);
        assert_eq!(
            resources,
            SemanticSsaAuxiliaryResourcesV1 {
                work_units: work,
                storage_words: words
            }
        );
        assert_eq!(meter.work, work);
        assert_eq!(meter.peak, 37 + bytes);
        for (work_limit, word_limit, resource, required, available) in [
            (
                work - 1,
                words,
                SsaPlannerResourceV1::WorkUnits,
                work,
                work - 1,
            ),
            (
                work,
                words - 1,
                SsaPlannerResourceV1::StorageWords,
                words,
                words - 1,
            ),
        ] {
            let (result, promoted, _, meter) =
                run(&blocks, &[], &updates, n, limit(work_limit, word_limit));
            assert_eq!(
                result,
                Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                    function: SemanticFunctionIdV1::from_index(0),
                    resource,
                    required,
                    limit: available,
                })
            );
            assert_eq!(promoted, vec![true; n]);
            assert_eq!(meter.storage, 37);
            assert!(meter.peak < 37 + bytes);
        }
        for denied in [
            None,
            Some(SsaPlannerResourceV1::WorkUnits),
            Some(SsaPlannerResourceV1::StorageWords),
        ] {
            let external_work =
                7 + work - usize::from(denied == Some(SsaPlannerResourceV1::WorkUnits));
            let external_storage =
                37 + bytes - usize::from(denied == Some(SsaPlannerResourceV1::StorageWords));
            let mut work_budget = Work::new(external_work);
            let mut budget = Budget::new(&mut work_budget, external_storage);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(37).unwrap();
            let mut ledger = Ledger(&mut budget);
            let mut account = Account::new(
                SemanticFunctionIdV1::from_index(0),
                ProductionSemanticSsaLimitsV1::default(),
            );
            let mut marks = account.markers(n, 0, &mut ledger).unwrap();
            marks.extend_from_slice(&updates);
            let mut promoted = vec![true; n];
            let result = refine(
                SsaBlockIdV1::new(0),
                &mut promoted,
                &[],
                &blocks,
                &marks,
                &mut account,
                &mut ledger,
            );
            account.finish(marks, &mut ledger).unwrap();
            match denied {
                None => {
                    result.unwrap();
                    assert_eq!(promoted, vec![false; n]);
                    assert_eq!(ledger.0.work(), 7 + work);
                    assert_eq!(ledger.0.peak_storage(), 37 + bytes);
                }
                Some(SsaPlannerResourceV1::WorkUnits) => {
                    let Err(CaptureError::Resource(Resource::Work(error))) = result else {
                        panic!("expected work denial")
                    };
                    assert_eq!(error.actual(), 7 + work);
                    assert_eq!(promoted, vec![true; n]);
                    assert!(ledger.0.peak_storage() < 37 + bytes);
                }
                Some(SsaPlannerResourceV1::StorageWords) => {
                    let Err(CaptureError::Resource(Resource::Storage(error))) = result else {
                        panic!("expected storage denial")
                    };
                    assert_eq!(error.actual(), 37 + bytes);
                    assert_eq!(promoted, vec![true; n]);
                    assert!(ledger.0.peak_storage() < 37 + bytes);
                }
                _ => unreachable!(),
            }
            assert_eq!(ledger.0.storage(), 37);
        }
    }
}

#[test]
fn holder_sparse_marker_overflow_is_rejected_before_any_debit_or_allocation() {
    let mut meter = Probe::default();
    let mut account = Account::new(
        SemanticFunctionIdV1::from_index(0),
        ProductionSemanticSsaLimitsV1::default(),
    );
    assert_eq!(
        account.markers(usize::MAX, 0, &mut meter).unwrap_err(),
        ProductionSemanticSsaErrorV1::ResourceOverflow
    );
    assert_eq!((meter.work, meter.storage), (0, 0));
}

#[test]
fn holder_sparse_sort_is_total_for_repeated_reversed_and_empty_keys() {
    for n in 0..129 {
        let mut values: Vec<_> = (0..n).rev().map(|value| value % 7).collect();
        heap_sort(&mut values, |value| *value);
        assert!(values.windows(2).all(|pair| pair[0] <= pair[1]));
        assert_eq!(values.len(), n);
        for key in 0..7 {
            assert_eq!(
                values.iter().filter(|value| **value == key).count(),
                (0..n).filter(|value| value % 7 == key).count()
            );
        }
    }
}
