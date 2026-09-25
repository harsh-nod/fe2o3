//! Inert transport/bound controls only; these do not mint authentic source seeds.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Cursor;

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
        Expansion::F32MatrixAccumulatorIntoValues,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(slot(role(expansion).unwrap()), i);
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
    let actual = Transport::capture(&c).unwrap();
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
    let original = Transport::capture(&call(
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
        assert_ne!(Transport::capture(&changed).unwrap(), original);
    }
}
#[test]
fn bf16_source_transport_refuses_extra_arity_and_executable_unwind() {
    assert!(
        Transport::capture(&call(
            vec![scalar(0); 5],
            5,
            SemanticUnwindActionV1::Continue
        ))
        .is_err()
    );
    assert!(Transport::capture(&call(vec![], 5, SemanticUnwindActionV1::Terminate)).is_err());
    assert!(
        Transport::capture(&call(
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
    assert!(Transport::capture(&c).is_err());
    let c = call(
        vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
            SemanticTypeIdV1::from_index(3),
            SemanticConstantValueV1::ZeroSized,
        ))],
        5,
        SemanticUnwindActionV1::Continue,
    );
    assert!(Transport::capture(&c).is_err());
}
#[test]
fn helper_move_transport_is_exact_and_never_inferred_from_equal_payloads() {
    let local = SemanticLocalIdV1::from_index(7);
    let other = SemanticLocalIdV1::from_index(8);
    let key = |moved| {
        Some(OperandKey::Local {
            local,
            ty: SemanticTypeIdV1::from_index(3),
            moved,
        })
    };
    for moved in [false, true] {
        assert!(defined_argument(0, Some(local), Some(moved), key(moved)).is_ok());
    }
    for index in 1..4 {
        assert!(defined_argument(index, Some(local), Some(true), key(true)).is_ok());
        assert!(defined_argument(index, Some(local), Some(false), key(false)).is_err());
        assert!(defined_argument(index, Some(other), Some(true), key(true)).is_err());
        assert!(defined_argument(index, None, Some(true), key(true)).is_err());
        assert!(defined_argument(index, Some(local), None, key(true)).is_err());
        assert!(defined_argument(index, Some(local), Some(false), key(true)).is_err());
        assert!(defined_argument(index, Some(local), Some(true), None).is_err());
    }
    assert!(defined_argument(4, Some(local), Some(true), key(true)).is_err());
}
#[test]
fn helper_dimensions_are_two_bodies_one_call_and_original_finite_scan() {
    assert!(dimensions(2, 2, 1, 1, 32, 4096).is_ok());
    for args in [
        (1, 2, 1, 1, 32, 4096),
        (2, 1, 1, 1, 32, 4096),
        (2, 2, 2, 1, 32, 4096),
        (2, 2, 1, 2, 32, 4096),
        (2, 2, 1, 1, 33, 4096),
        (2, 2, 1, 1, 32, 4097),
    ] {
        assert!(dimensions(args.0, args.1, args.2, args.3, args.4, args.5).is_err());
    }
}
#[test]
fn helper_roster_is_distinct_from_root_only_p0() {
    assert_eq!(ROLES.map(slot), [0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(ROLES[..5].iter().copied().all(root_role));
    assert!(!root_role(Role::Matrix));
    assert!(!root_role(Role::Values));
    assert!(root_role(Role::HelperCall));
    assert!(std::mem::size_of::<Bf16TileValuesImportCaptureV1<'_>>() <= 32);
    assert!(std::mem::size_of::<Pair<'_>>() < 65536);
    assert!(hir::scratch_bytes() < 16384);
    assert!(hir::work() < 1_000_000);
}
#[test]
fn helper_disabled_observer_cannot_mint_source_completion() {
    let capture = Bf16TileValuesImportCaptureV1::default();
    assert!(matches!(capture, Bf16TileValuesImportCaptureV1::Disabled));
    // No test fabricates a TyCtxt, Instance, source seed, or completed capture.
}
#[test]
fn helper_source_file_hash_rejects_changes_and_reads_actual_eof() {
    let text = "actual helper source";
    assert_eq!(
        file::hash_stream(&mut Cursor::new(text.as_bytes()), text).unwrap(),
        <[u8; 32]>::from(Sha256::digest(text.as_bytes()))
    );
    assert!(file::hash_stream(&mut Cursor::new(b"actual helper sourcX"), text).is_err());
    assert!(file::hash_stream(&mut Cursor::new(b"actual helper source!"), text).is_err());
    assert!(file::hash_stream(&mut Cursor::new(b"actual helper"), text).is_err());
}
