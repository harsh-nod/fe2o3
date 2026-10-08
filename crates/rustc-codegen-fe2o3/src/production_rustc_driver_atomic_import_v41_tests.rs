//! Genuine explicitly selected V41 import through the original rustc transaction.
//! This stops at the original SSA owner. It does not enable atomic KIR emission.
use super::*;

const ATOMIC_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::atomic_import_v41_tests::atomic_v41_original_import_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct AtomicRowV41 {
    function: u32,
    function_identity: [u8; 32],
    block: u32,
    statement: u32,
    operation: u8,
    ordering: u8,
    element: u32,
    signed: bool,
}
#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct AtomicImportV41 {
    source: [u8; 32],
    legacy_source: [u8; 32],
    functions: usize,
    types: usize,
    roots: usize,
    signed_markers: usize,
    unsigned_markers: usize,
    rows: Vec<AtomicRowV41>,
}

// Closed ten-kind fixture contract; numeric tags are test report fields only.
fn operation_contract_v41(operation: SemanticAtomicRmwOpV1) -> Option<(u8, bool)> {
    Some(match operation {
        SemanticAtomicRmwOpV1::Exchange => (0, false),
        SemanticAtomicRmwOpV1::Add => (1, false),
        SemanticAtomicRmwOpV1::Subtract => (2, false),
        SemanticAtomicRmwOpV1::BitAnd => (3, false),
        SemanticAtomicRmwOpV1::BitOr => (4, false),
        SemanticAtomicRmwOpV1::BitXor => (5, false),
        SemanticAtomicRmwOpV1::UnsignedMinimum => (6, false),
        SemanticAtomicRmwOpV1::UnsignedMaximum => (7, false),
        SemanticAtomicRmwOpV1::SignedMinimum => (8, true),
        SemanticAtomicRmwOpV1::SignedMaximum => (9, true),
        SemanticAtomicRmwOpV1::BitNand => return None,
    })
}
fn ordering_tag_v41(ordering: SemanticAtomicOrderingV1) -> u8 {
    match ordering {
        SemanticAtomicOrderingV1::Relaxed => 0,
        SemanticAtomicOrderingV1::Release => 1,
        SemanticAtomicOrderingV1::Acquire => 2,
        SemanticAtomicOrderingV1::AcquireRelease => 3,
        SemanticAtomicOrderingV1::SequentiallyConsistent => 4,
    }
}

fn complete_atomic_census_v41(semantic: &AdmittedInertSemanticMirV1) -> Vec<AtomicRowV41> {
    let mut rows = Vec::new();
    let mut kinds = [false; 10];
    for (function, declaration) in semantic.functions().iter().enumerate() {
        for (block, body) in declaration.blocks().iter().enumerate() {
            for (statement, row) in body.statements().iter().enumerate() {
                let SemanticStatementKindV1::AtomicRmw(atomic) = row.kind() else {
                    continue;
                };
                let (operation, signed) = operation_contract_v41(atomic.operation())
                    .expect("only the exact ten fixture operations");
                assert_eq!(atomic.access().scope(), SemanticAtomicScopeV1::System);
                let element = atomic.value().ty();
                assert_eq!(atomic.destination().ty(), element);
                assert_eq!(atomic.address().ty(), element);
                assert_eq!(
                    semantic.types()[element.index() as usize].shape(),
                    &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed,
                        bits: 32
                    })
                );
                kinds[usize::from(operation)] = true;
                rows.push(AtomicRowV41 {
                    function: u32::try_from(function).unwrap(),
                    function_identity: *declaration.identity().as_bytes(),
                    block: u32::try_from(block).unwrap(),
                    statement: u32::try_from(statement).unwrap(),
                    operation,
                    ordering: ordering_tag_v41(atomic.access().ordering()),
                    element: element.index(),
                    signed,
                });
            }
        }
    }
    assert_eq!(
        kinds, [true; 10],
        "the complete original census must cover all ten kinds"
    );
    // Core helpers match runtime Ordering into const-order intrinsic arms.
    // Preserve every retained statement: ten Rust calls do NOT imply ten MIR
    // statements, and no arm or duplicate source occurrence is discarded.
    for (operation, ordering) in [
        (0, 4),
        (1, 0),
        (2, 2),
        (3, 1),
        (4, 3),
        (5, 4),
        (6, 0),
        (7, 2),
        (8, 1),
        (9, 3),
    ] {
        assert!(
            rows.iter()
                .any(|row| row.operation == operation && row.ordering == ordering),
            "missing fixture operation/order {operation}/{ordering}"
        );
    }
    rows
}

#[derive(Default)]
struct AtomicImportCallbacksV41 {
    result: Option<Result<AtomicImportV41, String>>,
}
impl Callbacks for AtomicImportCallbacksV41 {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            // Two separate genuine source captures; neither is decoded from the
            // other nor supplied a copied ABI, producer table or source witness.
            let legacy = transaction()?
                .source_owned_ssa_for_test_v29(false)
                .map_err(|error| format!("original unchanged profile: {error:?}"))?;
            let source = transaction()?
                .source_owned_atomic_ssa_for_test_v41()
                .map_err(|error| format!("explicit original atomic V41 import: {error:?}"))?;
            let before = legacy.source_semantic();
            let after = source.source_semantic();
            assert_ne!(before.wire_version(), SemanticMirWireVersionV1::V41);
            assert_eq!(after.wire_version(), SemanticMirWireVersionV1::V41);
            assert_eq!(before.target(), after.target());
            assert_eq!(
                before.functions(),
                after.functions(),
                "complete original function/body/ABI/source identities remain unchanged"
            );
            assert_eq!(before.callables(), after.callables());
            assert_eq!(before.roots(), after.roots());
            assert_eq!(before.allocations(), after.allocations());
            assert_eq!(before.statics(), after.statics());
            assert_eq!(before.vtables(), after.vtables());
            assert_eq!(before.types().len(), after.types().len());
            let mut signed_markers = 0;
            let mut unsigned_markers = 0;
            for (old, current) in before.types().iter().zip(after.types()) {
                assert!(
                    !matches!(
                        old.rust_type_kind(),
                        SemanticRustTypeKindV1::AtomicI32 | SemanticRustTypeKindV1::AtomicU32
                    ),
                    "the default importer must not acquire the selected marker"
                );
                assert_eq!(
                    current.clone().with_rust_type_kind(old.rust_type_kind()),
                    *old,
                    "only nominal metadata changes, not source identity/layout/shape/ABI"
                );
                let expected_signed = match current.rust_type_kind() {
                    SemanticRustTypeKindV1::AtomicI32 => {
                        signed_markers += 1;
                        Some(true)
                    }
                    SemanticRustTypeKindV1::AtomicU32 => {
                        unsigned_markers += 1;
                        Some(false)
                    }
                    _ => None,
                };
                if let Some(signed) = expected_signed {
                    let index = after
                        .types()
                        .iter()
                        .position(|row| std::ptr::eq(row, current))
                        .unwrap();
                    let chain = semantic_atomic_storage_chain_v41(
                        after.types(),
                        SemanticTypeIdV1::from_index(u32::try_from(index).unwrap()),
                    )
                    .expect("original constructor commits the complete field chain");
                    assert_eq!(
                        after.types()[chain[2].index() as usize].shape(),
                        &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed,
                            bits: 32
                        })
                    );
                }
            }
            assert_eq!((signed_markers, unsigned_markers), (1, 1));
            let rows = complete_atomic_census_v41(after);
            assert_eq!(
                rows,
                complete_atomic_census_v41(before),
                "all retained atomic statements, identities, orders and multiplicities are preserved"
            );
            assert_ne!(
                source.source_semantic_sha256(),
                legacy.source_semantic_sha256(),
                "the selected version/nominal commitments must not equal the old owner"
            );
            assert_eq!(after.roots().len(), 1);
            Ok(AtomicImportV41 {
                source: *source.source_semantic_sha256(),
                legacy_source: *legacy.source_semantic_sha256(),
                functions: after.functions().len(),
                types: after.types().len(),
                roots: after.roots().len(),
                signed_markers,
                unsigned_markers,
                rows,
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn atomic_v41_original_import_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = AtomicImportCallbacksV41::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("original atomic source callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("atomic source result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "original atomic import: {result:?}");
}

fn atomic_source_v41(_: &str) -> String {
    // Same ten public operations/orderings as production-extraction-device's
    // core_atomic_rmw_v1 fixture; the normal helper compiles this actual source.
    r#"use fe2o3_device::{kernel, DeviceGlobalMutPtr};
use core::sync::atomic::Ordering;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn core_atomic_rmw_v1(unsigned: DeviceGlobalMutPtr<u32>, signed: DeviceGlobalMutPtr<i32>) {
    let unsigned = unsigned.as_atomic();
    let _ = unsigned.swap(1, Ordering::SeqCst);
    let _ = unsigned.fetch_add(2, Ordering::Relaxed);
    let _ = unsigned.fetch_sub(3, Ordering::Acquire);
    let _ = unsigned.fetch_and(4, Ordering::Release);
    let _ = unsigned.fetch_or(5, Ordering::AcqRel);
    let _ = unsigned.fetch_xor(6, Ordering::SeqCst);
    let _ = unsigned.fetch_min(7, Ordering::Relaxed);
    let _ = unsigned.fetch_max(8, Ordering::Acquire);
    let signed = signed.as_atomic();
    let _ = signed.fetch_min(-9, Ordering::Release);
    let _ = signed.fetch_max(10, Ordering::AcqRel);
}
"#
    .to_owned()
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and original source compilation"]
fn actual_atomic_v41_import_preserves_original_bodies_and_all_ten_rmw_kinds() {
    run_actual_sources::<AtomicImportV41>(
        &[("atomic-v41", ""), ("atomic-v41", "")],
        &[(0, 0)],
        ATOMIC_CHILD,
        "ATOMIC_V41_ORIGINAL_IMPORT",
        atomic_source_v41,
        |_, _, label, result, prior| {
            assert_eq!(result.roots, 1);
            assert_eq!((result.signed_markers, result.unsigned_markers), (1, 1));
            assert!(!result.rows.is_empty());
            assert_ne!(result.source, result.legacy_source);
            if let Some(previous) = prior.get(label) {
                assert_eq!(&result, previous);
            } else {
                prior.insert(label.to_owned(), result);
            }
        },
    );
}

#[test]
fn atomic_v41_fixture_oracle_has_exact_ten_kinds_and_never_accepts_nand() {
    let expected = [
        SemanticAtomicRmwOpV1::Exchange,
        SemanticAtomicRmwOpV1::Add,
        SemanticAtomicRmwOpV1::Subtract,
        SemanticAtomicRmwOpV1::BitAnd,
        SemanticAtomicRmwOpV1::BitOr,
        SemanticAtomicRmwOpV1::BitXor,
        SemanticAtomicRmwOpV1::UnsignedMinimum,
        SemanticAtomicRmwOpV1::UnsignedMaximum,
        SemanticAtomicRmwOpV1::SignedMinimum,
        SemanticAtomicRmwOpV1::SignedMaximum,
    ];
    for (index, operation) in expected.into_iter().enumerate() {
        assert_eq!(
            operation_contract_v41(operation),
            Some((u8::try_from(index).unwrap(), index >= 8))
        );
    }
    assert_eq!(operation_contract_v41(SemanticAtomicRmwOpV1::BitNand), None);
    assert_eq!(
        [
            SemanticAtomicOrderingV1::Relaxed,
            SemanticAtomicOrderingV1::Release,
            SemanticAtomicOrderingV1::Acquire,
            SemanticAtomicOrderingV1::AcquireRelease,
            SemanticAtomicOrderingV1::SequentiallyConsistent,
        ]
        .map(ordering_tag_v41),
        [0, 1, 2, 3, 4]
    );
}

#[path = "production_rustc_driver_atomic_root_v41_tests.rs"]
mod atomic_root_v41_tests;
