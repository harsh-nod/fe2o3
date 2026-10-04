#![forbid(unsafe_code)]

//! Cross-crate conformance tests for the bounded Pliron integration surfaces.

use dialect_mir::{MirTypeId, pliron::mir_dialect_registration};
use fe2o3_lower_mir_kernel as lower_mir_kernel;
use fe2o3_pliron::{ContextBuildError, DialectRegistration, PlironSession, ShellLimits};

const FORWARD_DIALECTS: [&str; 8] = [
    dialect_mir::DIALECT,
    dialect_kernel::DIALECT_NAME,
    dialect_schedule::DIALECT_NAME,
    dialect_tile::DIALECT_NAME,
    dialect_gpu::DIALECT_NAME,
    dialect_proof::DIALECT_NAME,
    dialect_dispatch::DIALECT_NAME,
    dialect_autotune::DIALECT_NAME,
];

const REVERSE_DIALECTS: [&str; 8] = [
    dialect_autotune::DIALECT_NAME,
    dialect_dispatch::DIALECT_NAME,
    dialect_proof::DIALECT_NAME,
    dialect_gpu::DIALECT_NAME,
    dialect_tile::DIALECT_NAME,
    dialect_schedule::DIALECT_NAME,
    dialect_kernel::DIALECT_NAME,
    dialect_mir::DIALECT,
];

fn registration(name: &str) -> DialectRegistration {
    match name {
        dialect_mir::DIALECT => mir_dialect_registration().expect("valid MIR registration"),
        dialect_kernel::DIALECT_NAME => {
            dialect_kernel::dialect_registration().expect("valid kernel registration")
        }
        dialect_schedule::DIALECT_NAME => {
            dialect_schedule::dialect_registration().expect("valid schedule registration")
        }
        dialect_tile::DIALECT_NAME => {
            dialect_tile::dialect_registration().expect("valid tile registration")
        }
        dialect_gpu::DIALECT_NAME => {
            dialect_gpu::dialect_registration().expect("valid GPU registration")
        }
        dialect_proof::DIALECT_NAME => {
            dialect_proof::dialect_registration().expect("valid proof registration")
        }
        dialect_dispatch::DIALECT_NAME => {
            dialect_dispatch::dialect_registration().expect("valid dispatch registration")
        }
        dialect_autotune::DIALECT_NAME => {
            dialect_autotune::dialect_registration().expect("valid autotune registration")
        }
        _ => panic!("unknown conformance dialect {name}"),
    }
}

fn combined_session(order: &[&str]) -> PlironSession {
    PlironSession::new(
        ShellLimits::default(),
        order.iter().copied().map(registration),
    )
    .expect("combined registration must succeed in a fresh context")
}

fn mir_config(rank: u32) -> lower_mir_kernel::LoweringConfig {
    lower_mir_kernel::LoweringConfig::new(
        lower_mir_kernel::LoweringLimits::new(1, 4, 8, 32, 4).expect("bounded MIR lowering limits"),
        rank,
    )
    .expect("bounded structured rank")
}

fn mir_conformance_input(identity: &str) -> lower_mir_kernel::MirKernelLoweringConformanceInputV1 {
    lower_mir_kernel::MirKernelLoweringConformanceInputV1::new(
        identity,
        vec![
            lower_mir_kernel::MirKernelLoweringConformanceFunctionV1::new(
                format!("{identity}::entry"),
                vec![MirTypeId(7), MirTypeId(2)],
            ),
        ],
    )
}

fn exercise_lowering() -> lower_mir_kernel::LoweringRecord {
    let result = lower_mir_kernel::MirKernelLoweringConformanceV1
        .run(&mir_conformance_input("lowering"), mir_config(2))
        .expect("supported MIR lowering");
    assert!(!result.grants_authority());
    result.record().clone()
}

#[test]
fn combined_registration_is_fresh_idempotent_and_order_independent() {
    let forward = combined_session(&FORWARD_DIALECTS);
    let reverse = combined_session(&REVERSE_DIALECTS);

    assert_eq!(
        forward.manifest().registration_order(),
        &FORWARD_DIALECTS.map(str::to_owned)
    );
    assert_eq!(
        reverse.manifest().registration_order(),
        &REVERSE_DIALECTS.map(str::to_owned)
    );
    assert_eq!(
        forward.manifest().pliron_revision(),
        reverse.manifest().pliron_revision()
    );
    assert_eq!(forward.manifest().registration_order().len(), 8);
    assert_eq!(reverse.manifest().registration_order().len(), 8);
}

#[test]
fn bounded_lowering_records_are_deterministic_across_private_sessions() {
    let forward = exercise_lowering();
    let reverse = exercise_lowering();

    assert_eq!(forward, reverse);
    assert_eq!(forward.source().identity(), "lowering");
    assert_eq!(forward.rewrite_count(), 1);
}

#[test]
fn duplicate_dialect_registration_fails_before_use() {
    let duplicate = registration(dialect_mir::DIALECT);
    let result = PlironSession::new(ShellLimits::default(), [duplicate.clone(), duplicate]);
    assert!(matches!(
        result,
        Err(ContextBuildError::DuplicateDialect(name)) if name == dialect_mir::DIALECT
    ));
}

#[test]
fn terminal_invalid_input_has_no_fallback_or_prior_result_channel() {
    let runner = lower_mir_kernel::MirKernelLoweringConformanceV1;
    let valid = runner
        .run(&mir_conformance_input("terminal"), mir_config(1))
        .expect("initial success");
    let empty = lower_mir_kernel::MirKernelLoweringConformanceInputV1::new("empty", vec![]);
    assert_eq!(
        runner.run(&empty, mir_config(1)),
        Err(lower_mir_kernel::LoweringError::EmptyModule)
    );
    let repeated = runner
        .run(&mir_conformance_input("terminal"), mir_config(1))
        .expect("fresh private session");
    assert_eq!(valid, repeated);
}
