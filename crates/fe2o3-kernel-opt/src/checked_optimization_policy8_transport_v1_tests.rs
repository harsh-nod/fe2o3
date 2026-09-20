//! Reuse the genuine consumed P7/P8 fixture through its private child hook.
#[path = "checked_optimization_policy8_graph_pool_v1_tests.rs"]
mod graph_pool;
#[path = "checked_optimization_policy8_history_inputs_v1_tests.rs"]
mod history_inputs;
use super::*;
use CanonicalPolicy8HistoryRoleV1 as Role;
use fe2o3_kernel_ir::{
    InertCanonicalKirOccurrenceRowBytesV1 as RowBytes, VerifiedCanonicalKernelIrModuleV12 as Owner,
    encode_canonical_kir_occurrence_row_bytes_v1 as encode_rows,
};
type TransportError = CanonicalPolicy8HistoryErrorV1;
const ROLES: [Role; 7] = [
    Role::B,
    Role::C,
    Role::S,
    Role::O,
    Role::I,
    Role::J,
    Role::K,
];

fn inputs<'a>(p: &'a Prepared8, rows: &'a RowBytes) -> CanonicalPolicy8HistoryEncodingInputsV1<'a> {
    let p8 = p.inputs();
    let p7 = p8.prefix;
    let p6 = p7.prefix;
    let p5 = p6.prefix;
    CanonicalPolicy8HistoryEncodingInputsV1 {
        roles: [
            p5.input,
            p5.intermediate,
            p5.stored,
            p5.output,
            p6.output,
            p7.output,
            p8.output,
        ],
        policy4_wire: p5.policy4_wire,
        policy5_record: p5.policy5_record,
        load_rows: p5.load_rows,
        integer_record: p6.continuation.integer_record,
        transition_wire: p6.continuation.transition_wire,
        policy7_record: p7.continuation.execution_record,
        tail_rows: rows,
    }
}
fn make(p: &Prepared8) -> InertCanonicalPolicy8HistoryV1 {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    let (rows, row_storage) =
        encode_rows(p.inputs().continuation.occurrences, &mut budget).unwrap();
    budget
        .reserve_storage(row_storage.retained_storage())
        .unwrap();
    let (wire, storage) = encode_inert_policy8_history_v1(inputs(p, &rows), &mut budget).unwrap();
    assert_eq!(wire.storage(), storage);
    assert_eq!(budget.storage(), p.floor + row_storage.retained_storage());
    drop(rows);
    budget
        .release_storage(row_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), p.floor);
    wire
}
fn accepted(wire: &[u8], k: &Owner) -> bool {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(31).unwrap();
    let result = read_inert_policy8_history_v1(wire, k, &mut budget);
    let ok = result.is_ok();
    drop(result);
    assert_eq!(budget.storage(), 31);
    ok
}
fn word(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}
fn set_word(bytes: &mut [u8], at: usize, value: usize) {
    bytes[at..at + 4].copy_from_slice(&u32::try_from(value).unwrap().to_le_bytes());
}
fn section(bytes: &[u8], axis: usize) -> std::ops::Range<usize> {
    let start = 364 + (0..axis).map(|i| word(bytes, 24 + 4 * i)).sum::<usize>();
    start..start + word(bytes, 24 + 4 * axis)
}
fn replace_section(bytes: &[u8], axis: usize, replacement: &[u8]) -> Vec<u8> {
    let range = section(bytes, axis);
    let mut out = bytes[..range.start].to_vec();
    out.extend_from_slice(replacement);
    out.extend_from_slice(&bytes[range.end..]);
    let length = out.len();
    set_word(&mut out, 12, length);
    set_word(&mut out, 24 + 4 * axis, replacement.len());
    out
}

// Independent literal framing recipe. It does not invoke the production planner,
// serializer or decoder; payload bytes come from the actual fixture owners.
fn literal(inputs: CanonicalPolicy8HistoryEncodingInputsV1<'_>) -> Vec<u8> {
    let mut pool: Vec<&[u8]> = Vec::new();
    let mut references = [u32::MAX; 7];
    for (i, owner) in inputs.roles.iter().enumerate() {
        let bytes = owner.canonical().canonical_bytes();
        if bytes != inputs.roles[6].canonical().canonical_bytes() {
            let index = match pool.iter().position(|prior| *prior == bytes) {
                Some(index) => index,
                None => {
                    pool.push(bytes);
                    pool.len() - 1
                }
            };
            references[i] = u32::try_from(index).unwrap();
        }
    }
    let mut sections: [Vec<u8>; 8] = std::array::from_fn(|_| Vec::new());
    for bytes in &pool {
        sections[0].extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        sections[0].extend_from_slice(bytes);
    }
    sections[1].extend_from_slice(inputs.policy4_wire);
    sections[2].extend_from_slice(inputs.policy5_record);
    sections[3].extend_from_slice(&(inputs.load_rows.len() as u32).to_le_bytes());
    for row in inputs.load_rows {
        for site in [row.first, row.load] {
            for value in [site.block.function.0, site.block.block, site.operation] {
                sections[3].extend_from_slice(&value.to_le_bytes());
            }
        }
    }
    sections[4].extend_from_slice(inputs.integer_record);
    sections[5].extend_from_slice(inputs.transition_wire);
    sections[6].extend_from_slice(inputs.policy7_record);
    let tail_length = 84 + inputs.tail_rows.canonical_row_bytes().len();
    sections[7].extend_from_slice(&(tail_length as u32).to_le_bytes());
    sections[7].extend_from_slice(&40u32.to_le_bytes());
    for count in inputs.tail_rows.counts() {
        sections[7].extend_from_slice(&count.to_le_bytes());
    }
    sections[7].extend_from_slice(b"gpu-commutative-bitwise-dominance-cse-v1");
    sections[7].extend_from_slice(inputs.tail_rows.canonical_row_bytes());
    let total = 364 + sections.iter().map(Vec::len).sum::<usize>();
    let mut out = b"F2HET1\0\0".to_vec();
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&8u16.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&7u32.to_le_bytes());
    out.extend_from_slice(&(pool.len() as u32).to_le_bytes());
    for field in &sections {
        out.extend_from_slice(&(field.len() as u32).to_le_bytes());
    }
    assert_eq!(out.len(), 56);
    for (index, owner) in inputs.roles.iter().enumerate() {
        out.extend_from_slice(owner.canonical().identity().digest());
        out.extend_from_slice(
            &owner
                .canonical()
                .identity()
                .canonical_length()
                .to_le_bytes(),
        );
        out.extend_from_slice(&references[index].to_le_bytes());
    }
    assert_eq!(out.len(), 364);
    for field in sections {
        out.extend_from_slice(&field);
    }
    assert_eq!(out.len(), total);
    out
}

#[test]
fn genuine_four_histories_match_complete_literal_bytes_and_separate_semantics() {
    for stores in [false, true] {
        for swaps in [false, true] {
            let p = prepared(stores, swaps);
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(p.floor).unwrap();
            let (rows, row_storage) =
                encode_rows(p.inputs().continuation.occurrences, &mut budget).unwrap();
            budget
                .reserve_storage(row_storage.retained_storage())
                .unwrap();
            let (wire, wire_storage) =
                encode_inert_policy8_history_v1(inputs(&p, &rows), &mut budget).unwrap();
            budget
                .reserve_storage(wire_storage.retained_storage())
                .unwrap();
            assert_eq!(wire.canonical_bytes(), literal(inputs(&p, &rows)));
            let floor = budget.storage();
            {
                let view = read_inert_policy8_history_v1(
                    wire.canonical_bytes(),
                    p.tail.output(),
                    &mut budget,
                )
                .unwrap();
                budget
                    .reserve_storage(view.storage().retained_storage())
                    .unwrap();
                assert!(std::ptr::eq(view.external_output(), p.tail.output()));
                for (role, owner) in ROLES.into_iter().zip(inputs(&p, &rows).roles) {
                    assert_eq!(view.graph_bytes(role), owner.canonical().canonical_bytes());
                    assert_eq!(
                        view.role(role).digest(),
                        *owner.canonical().identity().digest()
                    );
                    assert_eq!(
                        view.role(role).canonical_length(),
                        owner.canonical().identity().canonical_length()
                    );
                    if let Some(index) = view.role(role).pool_index() {
                        assert_eq!(
                            view.unverified_pool_graph_bytes(index as usize),
                            Some(view.graph_bytes(role))
                        );
                    }
                }
                assert!(
                    view.unverified_pool_graph_bytes(view.stored_graph_count())
                        .is_none()
                );
                assert!(view.unverified_pool_graph_bytes(usize::MAX).is_none());
                assert_eq!(view.role(Role::J).is_external_output(), !swaps);
                assert_eq!(
                    view.graph_bytes(Role::I) == view.graph_bytes(Role::J),
                    !stores
                );
                assert!(view.role(Role::K).is_external_output());
                assert_eq!(view.policy4_wire(), inputs(&p, &rows).policy4_wire);
                assert_eq!(view.policy5_record(), inputs(&p, &rows).policy5_record);
                assert_eq!(view.integer_record(), inputs(&p, &rows).integer_record);
                assert_eq!(view.transition_wire(), inputs(&p, &rows).transition_wire);
                assert_eq!(view.policy7_record(), inputs(&p, &rows).policy7_record);
                assert_eq!(
                    view.unvalidated_policy6_record(),
                    p.inputs().prefix.prefix.continuation.composition_record
                );
                assert_eq!(
                    view.tail_rows().canonical_row_bytes(),
                    rows.canonical_row_bytes()
                );
                assert_eq!(view.tail_rows().counts(), rows.counts());
                assert_eq!(view.storage().retained_storage(), size_of_val(&view));
                assert!(!view.authenticates_execution() && !view.grants_authority());
                let semantic =
                    check_published_policy8_semantic_relation_v1(p.inputs(), &mut budget).unwrap();
                assert_eq!(semantic.continuation().proved_pairs(), usize::from(swaps));
                assert!(!semantic.authenticates_execution());
            }
            budget
                .release_storage(size_of::<InertPolicy8HistoryRefV1<'_, '_>>())
                .unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(!wire.grants_authority() && !wire.authenticates_execution());
            assert!(
                wire_storage.retained_storage()
                    >= size_of_val(&wire) + wire.canonical_bytes().len()
            );
            drop(wire);
            budget
                .release_storage(wire_storage.retained_storage())
                .unwrap();
            drop(rows);
            budget
                .release_storage(row_storage.retained_storage())
                .unwrap();
            assert_eq!(budget.storage(), p.floor);
        }
    }
}

#[test]
fn empty_no_op_uses_external_k_for_every_explicit_role_and_no_pool() {
    let p = prepared_module(&Module::new("empty-history"));
    let wire = make(&p);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let view = read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
        .unwrap();
    assert_eq!(view.stored_graph_count(), 0);
    assert_eq!(word(wire.canonical_bytes(), 24), 0);
    for role in ROLES {
        assert!(view.role(role).is_external_output());
    }
    assert_eq!(view.tail_rows().counts(), [0; 9]);
    assert!(view.tail_rows().canonical_row_bytes().is_empty());
}

#[test]
fn every_truncation_trailing_byte_header_and_extent_mutation_is_rejected() {
    let p = prepared(true, true);
    let wire = make(&p);
    let original = wire.canonical_bytes();
    for end in 0..original.len() {
        assert!(!accepted(&original[..end], p.tail.output()), "end {end}");
    }
    let mut extra = original.to_vec();
    extra.push(0);
    assert!(!accepted(&extra, p.tail.output()));
    for at in [0, 7, 8, 9, 10, 11, 12, 16] {
        let mut bytes = original.to_vec();
        bytes[at] ^= 1;
        assert!(!accepted(&bytes, p.tail.output()), "header {at}");
    }
    for at in [20, 24, 28, 32, 36, 40, 44, 48, 52] {
        let mut bytes = original.to_vec();
        set_word(&mut bytes, at, u32::MAX as usize);
        assert!(!accepted(&bytes, p.tail.output()), "count {at}");
    }
    for role in 0..7 {
        let at = 56 + 44 * role;
        let mut bytes = original.to_vec();
        bytes[at + 32..at + 40].fill(0);
        assert!(!accepted(&bytes, p.tail.output()));
        bytes[at + 32..at + 40].fill(255);
        assert!(!accepted(&bytes, p.tail.output()));
        let mut bytes = original.to_vec();
        set_word(&mut bytes, at + 40, 6);
        assert!(!accepted(&bytes, p.tail.output()));
    }
}

#[test]
fn external_k_requires_actual_identity_but_equal_byte_owner_is_only_semantic() {
    let p = prepared(true, true);
    let wire = make(&p);
    assert!(!accepted(wire.canonical_bytes(), p.inputs().prefix.output));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (equal, storage) = Owner::from_canonical_bytes_with_verification_budget_v12(
        p.tail.output().canonical().canonical_bytes(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(!std::ptr::eq(&equal, p.tail.output()));
    let view = read_inert_policy8_history_v1(wire.canonical_bytes(), &equal, &mut budget).unwrap();
    assert!(std::ptr::eq(view.external_output(), &equal));
    assert!(!view.authenticates_execution());
    for at in [56 + 6 * 44, 56 + 6 * 44 + 32] {
        let mut changed = wire.canonical_bytes().to_vec();
        changed[at] ^= 1;
        assert!(!accepted(&changed, &equal));
    }
}

#[test]
fn pool_reference_first_use_alias_dedup_and_unused_payload_rules_are_exact() {
    let p = prepared(true, true);
    let wire = make(&p);
    let original = wire.canonical_bytes();
    let count = word(original, 20);
    assert!(count >= 2);
    let first = (0..6)
        .find(|i| word(original, 56 + 44 * i + 40) == 0)
        .unwrap();
    let mut bytes = original.to_vec();
    set_word(&mut bytes, 56 + 44 * first + 40, 1);
    assert!(!accepted(&bytes, p.tail.output()));
    let mut bytes = original.to_vec();
    set_word(&mut bytes, 56 + 44 * 6 + 40, 0);
    assert!(!accepted(&bytes, p.tail.output()));
    let mut bytes = original.to_vec();
    set_word(&mut bytes, 20, count - 1);
    assert!(!accepted(&bytes, p.tail.output()));
    // Make two actual roles alias, then make their claimed digests disagree.
    let alias = if first == 0 { 1 } else { 0 };
    let mut bytes = original.to_vec();
    let row = original[56 + 44 * first..56 + 44 * (first + 1)].to_vec();
    bytes[56 + 44 * alias..56 + 44 * (alias + 1)].copy_from_slice(&row);
    bytes[56 + 44 * alias] ^= 1;
    assert!(!accepted(&bytes, p.tail.output()));
    // Append an unused pool payload with otherwise repaired framing.
    let mut pool = original[section(original, 0)].to_vec();
    pool.extend_from_slice(&1u32.to_le_bytes());
    pool.push(0x71);
    let mut bytes = replace_section(original, 0, &pool);
    set_word(&mut bytes, 20, count + 1);
    assert!(!accepted(&bytes, p.tail.output()));
    // All roles before K refer to a stored byte-identical K, which must instead
    // use the external sentinel. Identity and length alone do not waive dedup.
    let k = p.tail.output().canonical();
    let mut pool = (k.canonical_bytes().len() as u32).to_le_bytes().to_vec();
    pool.extend_from_slice(k.canonical_bytes());
    let mut bytes = replace_section(original, 0, &pool);
    set_word(&mut bytes, 20, 1);
    for i in 0..6 {
        bytes[56 + i * 44..56 + i * 44 + 32].copy_from_slice(k.identity().digest());
        bytes[56 + i * 44 + 32..56 + i * 44 + 40]
            .copy_from_slice(&k.identity().canonical_length().to_le_bytes());
        set_word(&mut bytes, 56 + i * 44 + 40, 0);
    }
    assert!(!accepted(&bytes, p.tail.output()));
    // Two separately referenced identical non-K payloads are forbidden as well.
    let start = section(original, 0).start;
    let length = word(original, start);
    let payload = &original[start + 4..start + 4 + length];
    let mut pool = Vec::new();
    for _ in 0..2 {
        pool.extend_from_slice(&(length as u32).to_le_bytes());
        pool.extend_from_slice(payload);
    }
    let mut bytes = replace_section(original, 0, &pool);
    set_word(&mut bytes, 20, 2);
    for i in 0..6 {
        bytes[56 + i * 44..56 + (i + 1) * 44].copy_from_slice(&row);
        set_word(&mut bytes, 56 + i * 44 + 40, usize::from(i > 0));
    }
    assert!(!accepted(&bytes, p.tail.output()));
}

#[test]
fn raw_pool_digest_and_graph_syntax_remain_explicitly_unverified() {
    let p = prepared(true, true);
    let wire = make(&p);
    let mut bytes = wire.canonical_bytes().to_vec();
    let first = (0..6)
        .find(|i| word(&bytes, 56 + 44 * i + 40) == 0)
        .unwrap();
    for role in 0..6 {
        if word(&bytes, 56 + 44 * role + 40) == 0 {
            bytes[56 + 44 * role] ^= 0x80;
        }
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let view = read_inert_policy8_history_v1(&bytes, p.tail.output(), &mut budget).unwrap();
    budget
        .reserve_storage(view.storage().retained_storage())
        .unwrap();
    let (admitted, storage) = Owner::from_canonical_bytes_with_verification_budget_v12(
        view.graph_bytes(ROLES[first]),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_ne!(
        view.role(ROLES[first]).digest(),
        *admitted.canonical().identity().digest()
    );
    assert!(!view.grants_authority());
    let mut malformed = wire.canonical_bytes().to_vec();
    let payload = section(&malformed, 0).start + 4;
    malformed[payload] ^= 255;
    let malformed_view =
        read_inert_policy8_history_v1(&malformed, p.tail.output(), &mut budget).unwrap();
    budget
        .reserve_storage(malformed_view.storage().retained_storage())
        .unwrap();
    assert!(matches!(
        Owner::from_canonical_bytes_with_verification_budget_v12(
            malformed_view.graph_bytes(ROLES[first]),
            &mut budget
        ),
        Err(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Decode(
                fe2o3_kernel_ir::KernelIrDecodeError::InvalidMagic
            )
        )
    ));
}

#[test]
fn opaque_prefix_garbage_parses_but_existing_full_semantic_checker_refuses() {
    let p = prepared(true, true);
    let wire = make(&p);
    for axis in [1, 2, 4, 5, 6] {
        let mut bytes = wire.canonical_bytes().to_vec();
        let range = section(&bytes, axis);
        bytes[range].fill(0xa5);
        assert!(accepted(&bytes, p.tail.output()), "opaque axis {axis}");
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    let garbage = [0xa5; 200];
    let mut claims = p.inputs();
    claims.prefix.prefix.prefix.policy5_record = &garbage;
    let Err(CanonicalPolicy8CompositionErrorV1::Policy7(policy7)) =
        check_published_policy8_semantic_relation_v1(claims, &mut budget)
    else {
        panic!("expected nested P5 claim refusal, not a resource failure");
    };
    let CanonicalPolicy7SemanticErrorV1::Policy6(policy6) = *policy7 else {
        panic!("expected nested P6 prefix error");
    };
    assert!(matches!(
        *policy6,
        CanonicalPolicy6SemanticErrorV1::Policy5(CanonicalPolicy5SemanticErrorV1::ExecutionClaim)
    ));
    assert_eq!(budget.storage(), p.floor);
    for (axis, length) in [(1, 0), (2, 199), (4, 417), (5, 0), (6, 383)] {
        let bytes = replace_section(wire.canonical_bytes(), axis, &vec![0; length]);
        assert!(!accepted(&bytes, p.tail.output()));
    }
}

#[test]
fn load_rows_tail_name_counts_tags_and_partitions_are_not_opaque() {
    let p = prepared(true, true);
    let wire = make(&p);
    let original = wire.canonical_bytes();
    let tail = section(original, 7).start;
    for at in [tail, tail + 4, tail + 8, tail + 44, tail + 83] {
        let mut bytes = original.to_vec();
        bytes[at] ^= 1;
        assert!(!accepted(&bytes, p.tail.output()), "tail {at}");
    }
    for axis in 0..9 {
        let mut bytes = original.to_vec();
        set_word(&mut bytes, tail + 8 + 4 * axis, u32::MAX as usize);
        assert!(!accepted(&bytes, p.tail.output()));
    }
    let blocks = word(original, tail + 12);
    assert!(blocks > 0);
    let functions = word(original, tail + 8);
    let mut bytes = original.to_vec();
    set_word(&mut bytes, tail + 84 + 8 * functions + 12, 0);
    assert!(!accepted(&bytes, p.tail.output()));
    // Two explicit rows are syntax claims only. Ordering is by load coordinate.
    let mut loads = 2u32.to_le_bytes().to_vec();
    for load in [1u32, 2] {
        for value in [0, 0, 0, 0, 0, load] {
            loads.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert!(accepted(
        &replace_section(original, 3, &loads),
        p.tail.output()
    ));
    let mut duplicate = loads.clone();
    set_word(&mut duplicate, 4 + 24 + 20, 1);
    assert!(!accepted(
        &replace_section(original, 3, &duplicate),
        p.tail.output()
    ));
    let mut reverse = loads.clone();
    set_word(&mut reverse, 4 + 20, 3);
    assert!(!accepted(
        &replace_section(original, 3, &reverse),
        p.tail.output()
    ));
    set_word(&mut loads, 0, 3);
    assert!(!accepted(
        &replace_section(original, 3, &loads),
        p.tail.output()
    ));
}

fn observe_read(
    p: &Prepared8,
    wire: &[u8],
    work_limit: usize,
    storage_limit: usize,
) -> (bool, usize, usize, Option<usize>) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(p.floor).unwrap();
    budget.charge_work(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = read_inert_policy8_history_v1(wire, p.tail.output(), &mut budget);
    let ok = result.is_ok();
    drop(result);
    assert_eq!(budget.storage(), p.floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (
        ok,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn borrowed_reader_exact_and_short_work_storage_preserve_the_inherited_floor() {
    let p = prepared(true, true);
    let wire = make(&p);
    let baseline = observe_read(&p, wire.canonical_bytes(), WORK, STORAGE);
    assert!(baseline.0);
    assert_eq!(
        observe_read(&p, wire.canonical_bytes(), baseline.1, baseline.2),
        baseline
    );
    assert!(!observe_read(&p, wire.canonical_bytes(), baseline.1 - 1, baseline.2).0);
    let short = observe_read(&p, wire.canonical_bytes(), baseline.1, baseline.2 - 1);
    assert!(!short.0);
    assert_eq!(short.3, Some(baseline.2));
    let mut work = Work::new(baseline.1 - 17);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    {
        let _view =
            read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
                .unwrap();
    }
    let before = budget.work();
    assert!(
        read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
            .is_err()
    );
    assert_eq!((budget.work(), budget.storage()), (before, p.floor));
}

#[test]
fn owned_encoder_exact_and_short_work_storage_include_row_overlap() {
    let p = prepared(true, true);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (rows, row_storage) =
        encode_rows(p.inputs().continuation.occurrences, &mut budget).unwrap();
    let floor = p.floor + row_storage.retained_storage();
    let observe = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let result = encode_inert_policy8_history_v1(inputs(&p, &rows), &mut budget);
        let ok = result.is_ok();
        if let Ok((wire, storage)) = &result {
            assert!(storage.retained_storage() >= size_of_val(wire) + wire.canonical_bytes().len());
            assert!(budget.peak_storage() >= floor + storage.retained_storage());
        }
        drop(result);
        assert_eq!(budget.storage(), floor);
        (
            ok,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    let baseline = observe(WORK, STORAGE);
    assert!(baseline.0);
    assert_eq!(observe(baseline.1, baseline.2), baseline);
    assert!(!observe(baseline.1 - 1, baseline.2).0);
    assert!(!observe(baseline.1, baseline.2 - 1).0);
}

#[test]
fn aggregate_cap_is_single_not_per_section_and_refuses_before_output_allocation() {
    let p = prepared(false, false);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (rows, _) = encode_rows(p.inputs().continuation.occurrences, &mut budget).unwrap();
    let giant = vec![0; MAX_CANONICAL_POLICY8_HISTORY_BYTES_V1];
    let mut encoding = inputs(&p, &rows);
    encoding.policy4_wire = &giant;
    let floor = budget.storage();
    assert!(matches!(
        encode_inert_policy8_history_v1(encoding, &mut budget),
        Err(TransportError::Limit)
    ));
    assert_eq!(budget.storage(), floor);
    assert!(budget.peak_storage() < giant.len());
    let giant = vec![0; MAX_CANONICAL_POLICY8_HISTORY_BYTES_V1 + 1];
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    assert!(matches!(
        read_inert_policy8_history_v1(&giant, p.tail.output(), &mut budget),
        Err(TransportError::Limit)
    ));
    assert_eq!((budget.work(), budget.peak_storage()), (1, 0));
}

fn materialize_genuine_history(p: &Prepared8) {
    let wire = make(p);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(p.floor).unwrap();
    budget
        .reserve_storage(wire.storage().retained_storage())
        .unwrap();
    let frame_floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let view_storage;
    {
        let view =
            read_inert_policy8_history_v1(wire.canonical_bytes(), p.tail.output(), &mut budget)
                .unwrap();
        view_storage = view.storage().retained_storage();
        budget.reserve_storage(view_storage).unwrap();
        let floor = budget.storage();
        let (owned, storage) = fe2o3_kernel_ir::materialize_canonical_kir_occurrence_rows_v1(
            view.tail_rows(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let actual = owned.candidate();
        let expected = p.inputs().continuation.occurrences;
        macro_rules! same {
            ($($axis:ident),+ $(,)?) => { $(assert_eq!(actual.$axis, expected.$axis);)+ };
        }
        same!(
            functions,
            blocks,
            segments,
            operations,
            definitions,
            definition_outputs,
            uses,
            edges,
            edge_arguments
        );
        {
            let (encoded, _) = encode_rows(actual, &mut budget).unwrap();
            assert_eq!(
                encoded.canonical_row_bytes(),
                view.tail_rows().canonical_row_bytes()
            );
            assert_eq!(encoded.counts(), view.tail_rows().counts());
        }
        assert!(!owned.grants_authority());
        assert!(!view.grants_authority() && !view.authenticates_execution());
        let mut claims = p.inputs();
        claims.continuation.occurrences = owned.candidate();
        let semantic = check_published_policy8_semantic_relation_v1(claims, &mut budget).unwrap();
        let semantic_storage = semantic.storage().retained_storage();
        budget.reserve_storage(semantic_storage).unwrap();
        assert_eq!(
            semantic.continuation().proved_pairs(),
            p.tail.proved_pairs()
        );
        assert!(!semantic.grants_authority() && !semantic.authenticates_execution());
        budget.charge_work(1).unwrap();
        assert!(budget.work_ledger_identity_v1() == ledger);
        drop(semantic);
        budget.release_storage(semantic_storage).unwrap();
        drop(owned);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    budget.release_storage(view_storage).unwrap();
    assert_eq!(budget.storage(), frame_floor);
    let wire_storage = wire.storage().retained_storage();
    drop(wire);
    budget.release_storage(wire_storage).unwrap();
    assert_eq!(budget.storage(), p.floor);
}

#[test]
fn genuine_four_histories_materialize_actual_tail_rows_for_existing_full_replay() {
    for stores in [false, true] {
        for swaps in [false, true] {
            materialize_genuine_history(&prepared(stores, swaps));
        }
    }
}

#[test]
fn genuine_empty_no_op_materializes_no_rows_without_zeroing_the_retained_header() {
    materialize_genuine_history(&prepared_module(&Module::new("empty-materialized-history")));
}
