//! Genuine-source export and independent CPU oracle, not native/source-proof authority.
use super::*;
use crate::production_rustc_driver_v1::scoped_tile_diagnostic_export_v18::run_with_observation;
use fe2o3_kernel_ir as kir;
use fe2o3_kir_sim as sim;
use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;

#[path = "production_scoped_tile_cpu_oracle_v29_tests.rs"]
mod oracle;
const CHILD_PREFIX: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::tile_export_tests::";

#[derive(Debug, Serialize, Deserialize)]
struct Exported {
    order: String,
    kernel: String,
    scalar: [u8; 32],
    source: [u8; 32],
    pending: [u8; 32],
    schedule: [u8; 32],
    bytes: u64,
    vectors: usize,
    replays: usize,
}

fn export_child(order: Order) {
    let args_path = env::var_os(ARGS).expect("actual-source arguments required");
    let result_path = std::path::PathBuf::from(env::var_os(RESULT).expect("result path required"));
    let output_path = result_path.with_extension("canonical-v18.bin");
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(args_path).unwrap()).unwrap();
    let report = run_with_observation(&args, &output_path, order)
        .expect("genuine source must complete exact diagnostic-only export");
    let bytes = std::fs::read(&output_path).unwrap();
    assert_eq!(bytes.len() as u64, report.bytes);
    use sha2::Digest;
    assert_eq!(
        <[u8; 32]>::from(sha2::Sha256::digest(&bytes)),
        report.raw_sha256
    );
    let mut work = kir::CanonicalKernelIrWorkBudgetV1::new(1 << 28);
    let mut budget =
        kir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 256 * 1024 * 1024);
    let limits = kir::StorageLayoutLimitsV1 {
        rows: 4096,
        edges: 32768,
        containment_depth: 64,
        object_bytes: 256 * 1024 * 1024,
    };
    let ledger = budget.work_ledger_identity_v1();
    let result: Result<_, kir::CanonicalKernelIrVerificationResourceErrorV1> = budget
        .with_prepaid_scope(0, 0, 0, 0, |budget| {
            let (owner, storage) = kir::VerifiedCanonicalKernelIrModuleV18::
                from_canonical_bytes_with_verification_budget_v18(&bytes, limits, budget).unwrap();
            budget.reserve_storage(storage.retained_storage())?;
            assert_eq!(owner.identity().digest(), &report.scalar);
            assert_eq!(owner.identity().canonical_length(), report.bytes);
            assert_eq!(owner.canonical_bytes(), &bytes);
            assert!(!owner.module().storage_layouts.is_empty());
            let limits = sim::SimulationLimitsV1::default();
            let (simulation, receipt) =
                sim::AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
                    &owner, limits, budget,
                )
                .unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            assert_eq!(simulation.module(), owner.module());
            assert_eq!(simulation.identity().digest(), &report.scalar);
            assert!(!simulation.grants_execution_authority());
            let totals = oracle::run(&simulation, limits);
            let kernel = simulation.module().kernels[0].id.as_str().to_owned();
            drop(simulation);
            budget.release_storage(receipt.retained_storage())?;
            drop(owner);
            budget.release_storage(storage.retained_storage())?;
            Ok((totals, kernel))
        });
    let ((vectors, replays, reads, writes), kernel) = result.unwrap();
    assert_eq!(budget.storage(), 0);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(reads > 0 && writes > 0);
    let observation = Exported {
        order: format!("{order:?}"),
        kernel,
        scalar: report.scalar,
        source: report.source,
        pending: report.pending,
        schedule: report.schedule,
        bytes: report.bytes,
        vectors,
        replays,
    };
    std::fs::write(
        result_path,
        serde_json::to_vec(&Ok::<_, String>(observation)).unwrap(),
    )
    .unwrap();
    std::fs::remove_file(output_path).unwrap();
}

#[test]
#[ignore = "private genuine-source export subprocess; parent supplies exact arguments and result"]
fn scoped_tile_export_blocked_child() {
    export_child(Order::Blocked);
}

#[test]
#[ignore = "private genuine-source export subprocess; parent supplies exact arguments and result"]
fn scoped_tile_export_striped_child() {
    export_child(Order::Striped);
}

#[test]
#[ignore = "requires pinned nightly rust-src and authentic AMD SDK source compilation"]
fn actual_scoped_tile_export_preserves_canonical_identity_and_cpu_oracle() {
    for (child, order) in [
        ("scoped_tile_export_blocked_child", "Blocked"),
        ("scoped_tile_export_striped_child", "Striped"),
    ] {
        run_actual_sources::<Exported>(
            &[("masked_tile", MASKED_TILE_SOURCE)],
            &[(0, 0), (3, 2)],
            &format!("{CHILD_PREFIX}{child}"),
            "ACTUAL_SCOPED_TILE_DIAGNOSTIC_EXPORT_V18",
            str::to_owned,
            |_, _, _, observation, _| {
                assert_eq!(observation.order, order);
                assert!(!observation.kernel.is_empty());
                assert_eq!(observation.vectors, 28);
                assert_eq!(observation.replays, 56);
                assert!(observation.bytes > 0);
                assert_ne!(observation.scalar, [0; 32]);
                assert_ne!(observation.source, [0; 32]);
                assert_ne!(observation.pending, [0; 32]);
                assert_ne!(observation.schedule, [0; 32]);
                // Separate source sessions are not asserted to share authenticated identity.
            },
        );
    }
}
