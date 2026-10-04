//! Inert transport/bound controls only; these do not mint authentic source seeds.
use super::*;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(3),
    )
    .unwrap()
}
fn scalar(bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(3),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
    ))
}
fn call(
    args: Vec<SemanticOperandV1>,
    target: u32,
    unwind: SemanticUnwindActionV1,
) -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(9),
        args,
        Some(SemanticCallDestinationV1::new(
            place(11),
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallReturn,
                SemanticBlockIdV1::from_index(target),
            ),
        )),
        unwind,
    )
    .unwrap()
}
#[test]
fn bf16_source_roles_are_closed_distinct_and_not_width_only() {
    for (i, expansion) in [
        Expansion::MatrixContextCurrent,
        Expansion::WaveLaneCurrent,
        Expansion::Bf16MatrixALoadZeroFilledV2,
        Expansion::Bf16MatrixBLoadZeroFilledV2,
        Expansion::F32MatrixAccumulatorZero,
        Expansion::MatrixMultiplyAccumulate,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(role_index(role(expansion).unwrap()), i);
    }
    assert_eq!(role(Expansion::Gfx942OrderedProgramE32), None);
    assert_eq!(role(Expansion::Gfx942CompleteBodyE32), None);
}
#[test]
fn bf16_source_transport_retains_permuted_semantic_ids_not_raw_coordinates() {
    let c = call(
        vec![
            SemanticOperandV1::Copy(place(7)),
            SemanticOperandV1::Move(place(2)),
            scalar(0),
        ],
        5,
        SemanticUnwindActionV1::Continue,
    );
    let actual = CallTransport::capture(&c).unwrap();
    assert_eq!(
        actual.arguments[0],
        Some(OperandKey::Local {
            local: SemanticLocalIdV1::from_index(7),
            ty: SemanticTypeIdV1::from_index(3),
            moved: false
        })
    );
    assert_eq!(
        actual.arguments[1],
        Some(OperandKey::Local {
            local: SemanticLocalIdV1::from_index(2),
            ty: SemanticTypeIdV1::from_index(3),
            moved: true
        })
    );
    assert_eq!(actual.destination.0, SemanticLocalIdV1::from_index(11));
    assert_eq!(
        actual.destination.2.target(),
        SemanticBlockIdV1::from_index(5)
    );
}
#[test]
fn bf16_source_transport_detects_order_move_value_and_return_changes() {
    let original = CallTransport::capture(&call(
        vec![SemanticOperandV1::Copy(place(7)), scalar(0)],
        5,
        SemanticUnwindActionV1::Continue,
    ))
    .unwrap();
    for changed in [
        call(
            vec![scalar(0), SemanticOperandV1::Copy(place(7))],
            5,
            SemanticUnwindActionV1::Continue,
        ),
        call(
            vec![SemanticOperandV1::Move(place(7)), scalar(0)],
            5,
            SemanticUnwindActionV1::Continue,
        ),
        call(
            vec![SemanticOperandV1::Copy(place(7)), scalar(1)],
            5,
            SemanticUnwindActionV1::Continue,
        ),
        call(
            vec![SemanticOperandV1::Copy(place(7)), scalar(0)],
            6,
            SemanticUnwindActionV1::Continue,
        ),
        call(
            vec![SemanticOperandV1::Copy(place(7)), scalar(0)],
            5,
            SemanticUnwindActionV1::Unreachable,
        ),
    ] {
        assert_ne!(CallTransport::capture(&changed).unwrap(), original);
    }
}
#[test]
fn bf16_source_transport_refuses_extra_arity_and_executable_unwind() {
    assert!(
        CallTransport::capture(&call(
            vec![scalar(0); 5],
            5,
            SemanticUnwindActionV1::Continue
        ))
        .is_err()
    );
    assert!(CallTransport::capture(&call(vec![], 5, SemanticUnwindActionV1::Terminate)).is_err());
    assert!(
        CallTransport::capture(&call(
            vec![],
            5,
            SemanticUnwindActionV1::Cleanup(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::CallUnwind,
                SemanticBlockIdV1::from_index(6)
            ))
        ))
        .is_err()
    );
}
#[test]
fn bf16_source_transport_refuses_absent_return_and_non_scalar_constant() {
    let c = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(9),
        vec![],
        None,
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    assert!(CallTransport::capture(&c).is_err());
    let c = call(
        vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
            SemanticTypeIdV1::from_index(3),
            SemanticConstantValueV1::ZeroSized,
        ))],
        5,
        SemanticUnwindActionV1::Continue,
    );
    assert!(CallTransport::capture(&c).is_err());
}
#[test]
fn bf16_source_fixed_roster_deduplicates_exact_rows_but_refuses_overflow() {
    let mut values = [None; 2];
    insert_unique(&mut values, 17).unwrap();
    insert_unique(&mut values, 17).unwrap();
    insert_unique(&mut values, 29).unwrap();
    assert_eq!(values, [Some(17), Some(29)]);
    assert!(insert_unique(&mut values, 31).is_err());
    assert_eq!(values, [Some(17), Some(29)]);
}
struct Short<'a> {
    remaining: &'a [u8],
    reads: usize,
}
impl Read for Short<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        self.reads += 1;
        let n = out.len().min(self.remaining.len()).min(3);
        out[..n].copy_from_slice(&self.remaining[..n]);
        self.remaining = &self.remaining[n..];
        Ok(n)
    }
}
#[test]
fn bf16_source_read_continues_short_reads_to_real_eof() {
    let text = "actual source span bytes";
    let mut input = Short {
        remaining: text.as_bytes(),
        reads: 0,
    };
    assert_eq!(
        file::hash_stream(&mut input, text).unwrap(),
        <[u8; 32]>::from(Sha256::digest(text.as_bytes()))
    );
    assert!(input.reads > text.len() / 3);
}
#[test]
fn bf16_source_read_refuses_changed_truncated_extended_and_oversized_bytes() {
    assert!(file::hash_stream(&mut Cursor::new(b"abX"), "abc").is_err());
    assert!(file::hash_stream(&mut Cursor::new(b"ab"), "abc").is_err());
    assert!(file::hash_stream(&mut Cursor::new(b"abcd"), "abc").is_err());
    let text = "a".repeat(65537);
    assert!(file::hash_stream(&mut Cursor::new(text.as_bytes()), &text).is_err());
}
#[test]
fn bf16_source_disabled_handle_and_query_payload_are_finite() {
    assert!(std::mem::size_of::<Bf16MfmaImportCaptureV1<'_>>() <= 32);
    assert!(std::mem::size_of::<Captured<'_>>() < 65536);
    assert!(hir::scratch_bytes() < 16384);
    assert!(hir::work() < 1_000_000);
}

#[test]
fn bf16_source_whole_root_profile_accepts_exact_32_and_refuses_33() {
    assert_eq!(WHOLE_ROOT_BLOCKS, 32);
    assert!(validate_root_dimensions(32, 32, 32, 32).is_ok());
    assert!(validate_root_dimensions(33, 32, 33, 32).is_err());
    // Exact measured root counts are dimensions only, not authentic custody.
    assert!(validate_root_dimensions(20, 32, 20, 32).is_ok());
}

#[test]
fn bf16_source_whole_root_preserves_local_and_mapping_bounds() {
    assert!(validate_root_dimensions(32, 4096, 32, 4096).is_ok());
    assert!(validate_root_dimensions(32, 4097, 32, 4097).is_err());
    assert!(validate_root_dimensions(20, 32, 19, 32).is_err());
    assert!(validate_root_dimensions(20, 32, 21, 32).is_err());
    assert!(validate_root_dimensions(20, 32, 20, 31).is_err());
    assert!(validate_root_dimensions(20, 32, 20, 33).is_err());
}

#[test]
fn bf16_source_paid_header_scan_accepts_exact_statement_bound_and_refuses_one_over() {
    let counts = [128usize; 32];
    let visits = std::cell::Cell::new(0);
    assert_eq!(
        checked_root_statement_count(&counts, |n| {
            visits.set(visits.get() + 1);
            *n
        })
        .unwrap(),
        4096
    );
    assert_eq!(visits.get(), 32);
    let mut over = counts;
    over[0] += 1;
    assert!(checked_root_statement_count(&over, |n| *n).is_err());
    assert!(checked_root_statement_count(&[usize::MAX, 1], |n| *n).is_err());
}

#[test]
fn bf16_source_oversized_root_refuses_before_any_header_visit() {
    let visits = std::cell::Cell::new(0);
    assert!(
        checked_root_statement_count(&[0usize; 33], |n| {
            visits.set(visits.get() + 1);
            *n
        })
        .is_err()
    );
    assert_eq!(visits.get(), 0);
}

#[test]
fn bf16_source_whole_root_profile_does_not_enlarge_sparse_storage() {
    assert_eq!((BLOCKS, LOCALS, CALLS), (16, 16, 16));
    let mut slots = [None; BLOCKS];
    for value in 0..16 {
        insert_unique(&mut slots, value).unwrap();
    }
    let before = slots;
    assert!(insert_unique(&mut slots, 16).is_err());
    assert_eq!(slots, before);
    insert_unique(&mut slots, 0).unwrap();
    assert_eq!(slots, before);
}

#[test]
fn bf16_source_root_scan_is_inside_unchanged_original_validation_allowance() {
    assert_eq!(ROOT_SCAN_WORK_ENVELOPE, 160);
    assert_eq!(VALIDATION_WORK_ALLOWANCE, 4_000_000);
    assert_eq!(prepaid_validation_work(0).unwrap(), 4_000_000);
    assert_eq!(prepaid_validation_work(123).unwrap(), 4_000_123);
    assert_eq!(
        prepaid_validation_work(usize::MAX - 4_000_000).unwrap(),
        usize::MAX
    );
    assert!(prepaid_validation_work(usize::MAX - 3_999_999).is_err());
}
