//! Synthetic storage/mechanics controls only. No actual input/source loan is forged.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn expected_frame<T>(locals: usize) -> Result<usize> {
    Ok(locals + 2 * size_of::<T>() + 2 * size_of::<Result<T>>())
}
fn expected_sum(rows: &[usize]) -> Result<usize> {
    Ok(rows.iter().sum())
}
fn independently_typed_rows<R, F>() -> Result<[usize; FRAME_ROWS]> {
    type P = PendingActualRootPrefixIndicesV1;
    type A = PendingArgumentProducersV1;
    type S = InitializationSource<'static>;
    type V = ActualRootArgumentInitializationV1<'static>;
    type Resources = Prep<'static, 'static>;
    Ok([
        // Entry caller, explicit parameters, closure/result transfers.
        expected_frame::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut P,
            F,
            Result<R>,
            LedgerId,
            bool,
            &ProductionPreRankedKirOwnerV1,
        )>())?,
        // Source-graph continuation; the graph constructor ALSO admits its concrete closure.
        expected_frame::<R>(size_of::<(
            F,
            &mut Context,
            NominalCompleteForProfileGraphV1<'static>,
            &mut RootEntryPrefixV1,
            &mut A,
            &ProductionPreRankedKirOwnerV1,
            ActualSelectedInputsV1<'static>,
            S,
            V,
            Result<R>,
        )>())?,
        // Admission resource callback, including all captured argument borrows.
        expected_frame::<()>(size_of::<(
            &mut Resources,
            &mut P,
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            LedgerId,
            Option<LedgerId>,
            usize,
            Result<usize>,
            bool,
        )>())?,
        // New prefix+initializer resource callback, not the unchanged prefix body.
        expected_frame::<()>(size_of::<(
            &mut Resources,
            &mut RootEntryPrefixV1,
            &mut A,
            &S,
        )>())?,
        // Attached initializer.
        expected_frame::<()>(size_of::<(
            &mut A,
            &S,
            &mut Resources,
            usize,
            Option<LedgerId>,
            bool,
        )>())?,
        // Two distinct reached row initializer calls and their simultaneous transfers.
        expected_frame::<()>(size_of::<(
            &mut Vec<Option<u32>>,
            usize,
            &mut Resources,
            Option<u32>,
        )>())?
        .checked_mul(2)
        .ok_or_else(arithmetic)?,
        // Original preparation reserve<Option<u32>> reached by the new initializer.
        expected_frame::<()>(size_of::<(
            &mut Resources,
            &mut Vec<Option<u32>>,
            usize,
            usize,
            Option<usize>,
            bool,
            std::result::Result<(), std::collections::TryReserveError>,
        )>())?,
        expected_frame::<()>(size_of::<(&mut Resources, &mut &mut Budget<'static>, usize)>())?, // work
        expected_frame::<()>(size_of::<(
            &mut Resources,
            usize,
            &mut &mut Budget<'static>,
            &mut &mut usize,
            usize,
            Option<usize>,
        )>())?, // storage
        expected_frame::<Option<LedgerId>>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            &Budget<'static>,
            LedgerId,
        )>())?,
        expected_frame::<bool>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            Option<usize>,
            Option<usize>,
        )>())?
        .checked_add(expected_frame::<bool>(size_of::<&Resources>())?)
        .ok_or_else(arithmetic)?,
        expected_frame::<&[SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>())?,
        // Three separate source methods, each with its actual receiver.
        expected_sum(&[
            expected_frame::<bool>(size_of::<(&A, bool)>())?,
            expected_frame::<bool>(size_of::<(&A, Phase)>())?,
            expected_frame::<bool>(size_of::<(&LaterProducerPayloads, bool)>())?,
        ])?,
        // Nine reached concrete Vec<T> vacant callees; headers only, never T traversal.
        expected_sum(&[
            expected_frame::<bool>(size_of::<(&Vec<Option<ProjectedOrdinaryIndexV1>>, bool)>())?,
            expected_frame::<bool>(size_of::<(&Vec<ProjectedUniformInductionV1>, bool)>())?,
            expected_frame::<bool>(size_of::<(&Vec<Option<GuardPredicateV1>>, bool)>())?,
            expected_frame::<bool>(size_of::<(
                &Vec<Option<ProjectedDeterministicSwitchV1>>,
                bool,
            )>())?,
            expected_frame::<bool>(size_of::<(&Vec<Option<GuardedRankedAccessV1>>, bool)>())?
                .checked_mul(3)
                .ok_or_else(arithmetic)?,
            expected_frame::<bool>(size_of::<(&Vec<Option<ProjectedPipelineEffectV1>>, bool)>())?,
            expected_frame::<bool>(size_of::<(
                &Vec<Option<Vec<ProjectedGeneratedExecutableEffectV1>>>,
                bool,
            )>())?,
        ])?,
        // Immutable DATA getters: function, two slots, count, operations, SSA.
        expected_sum(&[
            expected_frame::<&SemanticFunctionDeclV1>(size_of::<&V>())?,
            expected_frame::<&[Option<u32>]>(size_of::<&V>())?
                .checked_mul(2)
                .ok_or_else(arithmetic)?,
            expected_frame::<usize>(size_of::<&V>())?,
            expected_frame::<&[ProductionRankedOperationV1]>(size_of::<&V>())?,
            expected_frame::<u32>(size_of::<&V>())?,
        ])?,
        // Ending state/result transfer after all nested postflights have returned.
        expected_frame::<R>(size_of::<(&mut A, Phase, Result<R>, bool)>())?,
        expected_frame::<[usize; FRAME_ROWS]>(size_of::<(
            [usize; FRAME_ROWS],
            usize,
            Option<usize>,
            Result<usize>,
        )>())?,
        expected_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        // Arithmetic/resource mapper plus error-only return paths (no arbitrary closure).
        expected_frame::<Error>(size_of::<(Resource, Option<usize>)>())?
            .checked_mul(3)
            .ok_or_else(arithmetic)?,
        // New private payload helper, distinct from source authentication.
        expected_frame::<()>(size_of::<(&mut A, usize, &mut Resources)>())?,
        // Nested row-array constructors passed to sum above: actual lengths.
        expected_frame::<[usize; 3]>(size_of::<[usize; 3]>())?,
        expected_frame::<[usize; 7]>(size_of::<[usize; 7]>())?,
        expected_frame::<[usize; 5]>(size_of::<[usize; 5]>())?,
        // additional_frame helper caller/return and whole roster receiver.
        expected_frame::<usize>(size_of::<([usize; FRAME_ROWS], Result<[usize; FRAME_ROWS]>)>())?,
        // Generic call_frame's executable arithmetic has scalar locals only.
        expected_frame::<usize>(size_of::<(usize, usize, usize, Option<usize>, Result<usize>)>())?,
    ])
}

fn original_slot_oracle(local_count: usize) -> (Vec<Option<u32>>, Vec<Option<u32>>, usize) {
    let mut runtime_index_arguments = vec![None; local_count];
    let mut runtime_slice_extent_arguments = vec![None; local_count];
    let mut next_runtime_argument = 1_usize;
    // The three donor statements are unchanged; these no-op mutable borrows
    // preserve their exact text without unused_mut qualification warnings.
    let _ = (
        &mut runtime_index_arguments,
        &mut runtime_slice_extent_arguments,
        &mut next_runtime_argument,
    );
    (
        runtime_index_arguments,
        runtime_slice_extent_arguments,
        next_runtime_argument,
    )
}
#[derive(Clone, Copy, Debug)]
struct Snapshot {
    ok: bool,
    index_len: usize,
    slice_len: usize,
    index_capacity: usize,
    slice_capacity: usize,
    next: usize,
    initialized: bool,
    owned: usize,
    work: usize,
    denied_work: bool,
    denied_storage: bool,
}
fn mechanics(count: usize, work_limit: usize, storage_limit: usize) -> Snapshot {
    // Isolated primitive charges ONLY; full source/driver frames are tested below
    // and by the separate genuine observer. Test fixture/header allocations are
    // not presented as a production resource admission.
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let mut owned = 0;
    let mut state = PendingArgumentProducersV1::new();
    state.phase = Phase::Terminal;
    let outcome = {
        let mut resources = Prep::new(&mut budget, &mut owned);
        state.initialize_payload(count, &mut resources)
    };
    let snapshot = Snapshot {
        ok: outcome.is_ok(),
        index_len: state.index_arguments.len(),
        slice_len: state.slice_arguments.len(),
        index_capacity: state.index_arguments.capacity(),
        slice_capacity: state.slice_arguments.capacity(),
        next: state.next_argument,
        initialized: state.initialized,
        owned,
        work: budget.work(),
        denied_work: budget.failed_work().is_some(),
        denied_storage: budget.failed_storage().is_some(),
    };
    assert_eq!(budget.storage(), owned);
    assert_eq!(
        state.phase,
        Phase::Terminal,
        "mechanics cannot mint an authentic continuation"
    );
    assert!(state.later.vacant());
    if outcome.is_ok() {
        let expected = original_slot_oracle(count);
        assert_eq!(state.index_arguments, expected.0);
        assert_eq!(state.slice_arguments, expected.1);
        assert_eq!(state.next_argument, expected.2);
    }
    // Physical arrays remain alive until the caller inspects its owned debit.
    drop(state);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
    snapshot
}

#[test]
fn actual_argument_initialization_owner_is_dormant_without_second_prefix_or_counter() {
    let pending = PendingActualRootPrefixIndicesV1::new();
    assert!(pending.arguments.vacant());
    assert!(pending.prefix.entry_operations.is_empty());
    assert_eq!(pending.prefix.next_value, 0);
    assert!(!pending.started && !pending.completed);
}
#[test]
fn actual_argument_initialization_matches_frozen_three_statement_oracle() {
    for n in [0, 1, 3, 17] {
        let got = mechanics(n, 2 * n, 2 * n * size_of::<Option<u32>>());
        assert!(got.ok && got.initialized);
        assert_eq!((got.index_len, got.slice_len, got.next), (n, n, 1));
        assert_eq!(got.work, 2 * n);
        assert_eq!(got.owned, 2 * n * size_of::<Option<u32>>());
        assert!(!got.denied_work && !got.denied_storage);
    }
}
#[test]
fn actual_argument_initialization_refuses_before_first_row_visit() {
    let got = mechanics(3, 2, 1024);
    assert!(!got.ok && got.denied_work);
    assert_eq!(
        (got.index_len, got.slice_len, got.next, got.owned),
        (0, 0, 0, 0)
    );
}
#[test]
fn actual_argument_initialization_second_work_refusal_retains_first_array() {
    let got = mechanics(3, 3, 1024);
    assert!(!got.ok && got.denied_work && !got.initialized);
    assert_eq!((got.index_len, got.slice_len, got.next), (3, 0, 0));
    assert_eq!(got.index_capacity, 3);
    assert_eq!(got.slice_capacity, 0);
    assert_eq!(got.owned, 3 * size_of::<Option<u32>>());
}
#[test]
fn actual_argument_initialization_first_storage_refusal_has_no_array() {
    let got = mechanics(3, 6, 3 * size_of::<Option<u32>>() - 1);
    assert!(!got.ok && got.denied_storage);
    assert_eq!(
        (got.index_len, got.slice_len, got.next, got.owned),
        (0, 0, 0, 0)
    );
}
#[test]
fn actual_argument_initialization_second_storage_refusal_retains_first_array() {
    let got = mechanics(3, 6, 6 * size_of::<Option<u32>>() - 1);
    assert!(!got.ok && got.denied_storage);
    assert_eq!((got.index_len, got.slice_len, got.next), (3, 0, 0));
    assert_eq!(got.owned, 3 * size_of::<Option<u32>>());
}
#[test]
fn actual_argument_initialization_all_small_cut_points_keep_counter_last() {
    let n = 3;
    for work in 0..=2 * n {
        for storage in 0..=2 * n * size_of::<Option<u32>>() {
            let got = mechanics(n, work, storage);
            assert_eq!(
                got.ok,
                work == 2 * n && storage == 2 * n * size_of::<Option<u32>>()
            );
            if !got.ok {
                assert_eq!(got.next, 0);
                assert!(!got.initialized);
            }
            assert!(got.slice_len == 0 || got.index_len == n);
        }
    }
}
#[test]
fn actual_argument_initialization_rejects_occupied_header_and_capacity() {
    let mut state = PendingArgumentProducersV1::new();
    state.index_arguments.reserve_exact(1);
    assert!(state.index_arguments.is_empty());
    assert!(!state.vacant());
    let mut later = PendingArgumentProducersV1::new();
    later.later.direct = Some(DirectComparisonPreparationV1::new());
    assert!(!later.vacant());
}
#[test]
fn actual_argument_initialization_terminal_state_cannot_be_dormant() {
    for phase in [Phase::Terminal, Phase::InitializedBeforeArgumentWriters] {
        let mut state = PendingArgumentProducersV1::new();
        state.phase = phase;
        assert!(!state.dormant() && !state.vacant());
    }
}
#[test]
fn actual_argument_initialization_work_and_storage_remain_owned_after_unwind() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let mut work = Work::new(6);
    let mut budget = Budget::new(&mut work, 6 * size_of::<Option<u32>>());
    let mut owned = 0;
    let mut state = PendingArgumentProducersV1::new();
    state.phase = Phase::Terminal;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        state
            .initialize_payload(3, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        panic!("synthetic caller unwind after accepted arrays");
    }));
    assert!(outcome.is_err());
    assert_eq!(state.phase, Phase::Terminal);
    assert_eq!(state.index_arguments.len(), 3);
    assert_eq!(state.slice_arguments.len(), 3);
    assert_eq!(budget.storage(), owned);
    drop(outcome);
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn actual_argument_initialization_typed_rows_are_independently_reconstructed() {
    assert_eq!(
        frame_rows::<(), ()>().unwrap(),
        independently_typed_rows::<(), ()>().unwrap()
    );
    assert_eq!(
        frame_rows::<[u8; 4097], [u8; 8193]>().unwrap(),
        independently_typed_rows::<[u8; 4097], [u8; 8193]>().unwrap()
    );
}
#[test]
fn actual_argument_initialization_frame_arithmetic_refuses_overflow() {
    assert!(call_frame::<()>(usize::MAX).is_err());
    assert!(sum(&[usize::MAX, 1]).is_err());
}
#[test]
fn actual_argument_initialization_large_callback_and_result_are_not_hidden() {
    let small = additional_frame::<(), ()>().unwrap();
    let large = additional_frame::<[u8; 4097], [u8; 8193]>().unwrap();
    assert!(large >= small + 2 * 8193 + 2 * 4097);
}

#[test]
fn actual_argument_initialization_existing_assembly_constructor_delta_is_explicit() {
    let expected = expected_sum(&[
        expected_frame::<PendingArgumentProducersV1>(size_of::<(
            Phase,
            Option<usize>,
            Option<LedgerId>,
            bool,
        )>())
        .unwrap(),
        expected_frame::<LaterProducerPayloads>(0).unwrap(),
        expected_frame::<bool>(size_of::<(&PendingArgumentProducersV1, Phase, bool)>()).unwrap(),
        expected_frame::<[usize; 8]>(size_of::<[usize; 8]>()).unwrap(),
        expected_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())
        .unwrap(),
        expected_frame::<usize>(size_of::<(usize, Option<usize>, Result<usize>)>()).unwrap(),
        expected_frame::<usize>(size_of::<(usize, usize, usize, Option<usize>, Result<usize>)>())
            .unwrap(),
        expected_frame::<Error>(size_of::<(Resource, Option<usize>)>()).unwrap(),
    ])
    .unwrap();
    assert_eq!(construction_frame_v1().unwrap(), expected);
    assert!(
        assembly_frame::<(), ()>().unwrap()
            >= size_of::<PendingActualRootPrefixIndicesV1>() + expected
    );
}
