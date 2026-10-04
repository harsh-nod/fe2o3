//! Actual borrowed descriptors/capabilities through the fixed original-source route.
use super::*;
use fe2o3_kernel_ir::{AddressSpace, OperationKind};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionRoleV1, SemanticSourceArgumentOwnershipV1 as Ownership,
    SemanticTerminatorKindV1,
};
use std::cell::Cell;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::borrowed_helper_tests::borrowed_helper_source_child";
const FINAL_REFUSAL: &str = "original source integer final admission required";
const STORE: &str = r#"
#[inline(never)]
fn store(output: &mut DisjointSlice<u32, GridExclusive>, leader: &GridLeader, index: usize, value: u32) {
    let Some(slot) = output.get_mut_exclusive(leader, index) else { fe2o3_device::trap(); };
    *slot = value;
}
"#;
const DIRECT: &str = r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn direct(mut output: DisjointSlice<u32, GridExclusive>, value: u32) {
    let Some(leader) = thread::grid_leader() else { return; };
    store(&mut output, &leader, 0, value);
}
"#;
const REPEATED: &str = r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn repeated(mut left: DisjointSlice<u32, GridExclusive>, mut right: DisjointSlice<u32, GridExclusive>, value: u32) {
    let Some(leader) = thread::grid_leader() else { return; };
    store(&mut left, &leader, 0, value);
    store(&mut right, &leader, 1, value);
}
"#;
const NESTED: &str = r#"
#[inline(never)]
fn pair(output: &mut DisjointSlice<u32, GridExclusive>, leader: &GridLeader, value: u32) {
    store(output, leader, 0, value);
    store(output, leader, 1, value);
}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn nested(mut output: DisjointSlice<u32, GridExclusive>, value: u32) {
    let Some(leader) = thread::grid_leader() else { return; };
    pair(&mut output, &leader, value);
}
"#;

fn program(body: &str) -> String {
    format!(
        "use fe2o3_device::{{DisjointSlice, GridExclusive, GridLeader, kernel, thread}};\n{STORE}\n{body}"
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    source: [u8; 32],
    original: [u8; 32],
    output: [u8; 32],
    version: u16,
    instances: usize,
    paired_borrow_instances: usize,
    repeated_callee_instances: usize,
    nested_instances: usize,
    memory_anchors: usize,
    nonprivate_writes: usize,
    prepared: bool,
    materialized: bool,
    foreign_owner_refused: bool,
    incomplete_abi_refused: bool,
    invalid_instance_refused: bool,
    foreign_ledger_refused: bool,
    final_refused: bool,
}

#[derive(Default)]
struct BorrowedCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for BorrowedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|e| format!("independent original helper source: {e:?}"))?;
            start_preparation_observation_v29();
            let continuation = transaction()?.with_original_source_integer_custody_v18(
                |source, handoff, roots, _, budget| {
                    let semantic = source.source_semantic(budget)?;
                    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V35);
                    assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                    assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                    source.require_kernel_argument_abi_v18(AbiInput { roots }, budget)?;
                    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                    assert_eq!(source.root_count(budget)?, 1);
                    let count = source.instance_count(0, budget)?;
                    let mut paired = Vec::new();
                    let mut nested = 0;
                    let mut anchors = 0;
                    for instance in 0..count {
                        let (function, incoming) = source.instance(0, instance, budget)?;
                        let declaration = &semantic.functions()[function.index() as usize];
                        if let Some((caller, block)) = incoming {
                            assert!(caller < instance, "real expansion precedes its child");
                            let (caller_function, caller_incoming) = source.instance(0, caller, budget)?;
                            let caller_body = &semantic.functions()[caller_function.index() as usize];
                            let SemanticTerminatorKindV1::Call(call) = caller_body.blocks()[block.index() as usize].terminator().kind() else {
                                panic!("instance parent must be the original source call");
                            };
                            assert!(matches!(semantic.callables().get(call.callee().index() as usize),
                                Some(SemanticCallableDeclV1::Defined { function: actual }) if *actual == function));
                            let ownership = declaration.abi().source_argument_ownership();
                            if declaration.role() == SemanticFunctionRoleV1::InternalHelper
                                && ownership.first() == Some(&Ownership::UniqueBorrow)
                                && ownership.get(1) == Some(&Ownership::SharedBorrow)
                            {
                                assert!(source.instance_active(0, instance, budget)?);
                                paired.push(function);
                                nested += usize::from(caller_incoming.is_some());
                                anchors += source.memory_anchor_count(0, instance, budget)?;
                            }
                        } else {
                            assert_eq!(instance, 0);
                            assert_eq!(source.root(0, budget)?.0, function);
                        }
                    }
                    assert!(!paired.is_empty(), "actual borrowed helper body must survive capture");
                    let repeated = paired.iter().enumerate().filter(|(index, function)| paired[..*index].contains(function)).count();
                    let original = source.canonical(budget)?;
                    let output = handoff.output(budget)?;
                    assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                    let nonprivate_writes = output.owner().module().functions.iter()
                        .filter_map(|f| f.body.as_ref()).flat_map(|body| &body.blocks)
                        .flat_map(|block| &block.operations)
                        .filter(|operation| matches!(&operation.kind,
                            OperationKind::Store { access, .. } | OperationKind::GuardedStore { access, .. }
                            if access.address_space != AddressSpace::Private)).count();
                    assert!(anchors > 0 && nonprivate_writes > 0, "helper transport cannot erase all output effects");
                    Ok(Observation {
                        source: *source.source_ssa(budget)?.source_semantic_sha256(),
                        original: Sha256::digest(original.canonical_bytes()).into(),
                        output: Sha256::digest(output.owner().canonical_bytes()).into(),
                        version: semantic.wire_version().as_u16(), instances: count,
                        paired_borrow_instances: paired.len(), repeated_callee_instances: repeated,
                        nested_instances: nested, memory_anchors: anchors, nonprivate_writes,
                        prepared: false, materialized: false, foreign_owner_refused: false,
                        incomplete_abi_refused: false, invalid_instance_refused: false,
                        foreign_ledger_refused: false, final_refused: false,
                    })
                },
            ).map_err(|e| format!("borrowed helper original-source handoff: {e:?}"))?;
            let preparation =
                take_preparation_observation_v29().expect("helper preparation observed");
            assert!(preparation.materialized);
            let mut report = continuation.into_observation();
            report.prepared = true;
            report.materialized = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |source, _, _, _, budget| {
                        let error = source.check_original_source(&foreign, budget).unwrap_err();
                        assert!(matches!(
                            error,
                            SourceError::Binding("foreign original SSA owner")
                        ));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(
                completed.get(),
                "sticky failure cannot conceal a callback assertion panic"
            );
            assert!(matches!(
                error,
                Error::Source(SourceError::Binding("foreign original SSA owner"))
            ));
            report.foreign_owner_refused = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |source, _, roots, _, budget| {
                        assert_eq!(roots.len(), 1);
                        let root = &roots[0];
                        assert!(!root.arguments.is_empty());
                        let changed = [AbiRoot {
                            kernel_binding: root.kernel_binding,
                            export: root.export,
                            arguments: &root.arguments[..root.arguments.len() - 1],
                            explicit_argument_bytes: root.explicit_argument_bytes,
                            kernarg_alignment_bytes: root.kernarg_alignment_bytes,
                        }];
                        let error = source
                            .require_kernel_argument_abi_v18(AbiInput { roots: &changed }, budget)
                            .unwrap_err();
                        assert!(matches!(error, SourceError::Binding(
                            "kernel argument ABI profile differs from its original descriptor/source contract"
                        )));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Binding(
                    "kernel argument ABI profile differs from its original descriptor/source contract"
                ))
            ));
            report.incomplete_abi_refused = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |source, _, _, _, budget| {
                        let invalid = source.instance_count(0, budget)?;
                        let error = source.instance(0, invalid, budget).unwrap_err();
                        assert!(matches!(error, SourceError::Binding("instance ordinal")));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Binding("instance ordinal"))
            ));
            report.invalid_instance_refused = true;

            let completed = Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |_, handoff, _, _, _| {
                        let mut work =
                            fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                        let foreign = Budget::new(&mut work, 20_000_000);
                        let error = match handoff.output(&foreign) {
                            Err(error) => error,
                            Ok(_) => panic!("foreign ledger acquired helper output"),
                        };
                        assert!(matches!(
                            error,
                            SourceError::Resource(ResourceError::Accounting)
                        ));
                        completed.set(true);
                        Err(error.into())
                    },
                ),
            );
            assert!(completed.get());
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));
            report.foreign_ledger_refused = true;

            start_preparation_observation_v29();
            let error = transaction()?
                .original_source_integer_finalizer_refusal_v18()
                .unwrap_err();
            let preparation =
                take_preparation_observation_v29().expect("final-refusal preparation observed");
            assert!(preparation.materialized);
            assert!(matches!(error, Error::Unsupported(FINAL_REFUSAL)));
            report.final_refused = true;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn borrowed_helper_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = BorrowedCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("borrowed helper callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("borrowed helper result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "borrowed helper source: {result:?}");
}

fn check_source(
    label: &str,
    body: &str,
    paired: usize,
    repeated: usize,
    nested: usize,
    writes: usize,
) {
    run_actual_sources::<Observation>(
        &[(label, body)],
        &[(0, 0)],
        CHILD,
        "BORROWED_HELPER_SOURCE_V18",
        program,
        |_, _, _, report, _| {
            assert_eq!(report.version, 35);
            assert!(report.instances > report.paired_borrow_instances);
            assert_eq!(report.paired_borrow_instances, paired);
            assert_eq!(report.repeated_callee_instances, repeated);
            assert!(report.nested_instances >= nested);
            assert!(report.memory_anchors > 0 && report.nonprivate_writes >= writes);
            assert!(report.prepared && report.materialized);
            assert!(report.foreign_owner_refused && report.incomplete_abi_refused);
            assert!(report.invalid_instance_refused && report.foreign_ledger_refused);
            assert!(report.final_refused);
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_original_source_borrows_descriptor_and_leader_in_helper() {
    check_source("borrowed_direct", DIRECT, 1, 0, 0, 1);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_original_source_distinguishes_repeated_helper_instances_and_descriptors() {
    check_source("borrowed_repeated", REPEATED, 2, 1, 0, 2);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_original_source_composes_nested_descriptor_and_capability_reborrows() {
    check_source("borrowed_nested", NESTED, 3, 1, 2, 2);
}

#[test]
fn borrowed_helper_child_and_final_refusal_protocol_are_distinct() {
    assert!(
        CHILD.ends_with(
            "::original_source_tests::borrowed_helper_tests::borrowed_helper_source_child"
        )
    );
    assert_ne!(CHILD, ORIGINAL_CHILD);
    assert_ne!(CHILD, MIXED_CHILD);
    assert_eq!(
        FINAL_REFUSAL,
        "original source integer final admission required"
    );
}

#[test]
fn borrowed_helper_source_fixtures_keep_real_mutable_capability_calls() {
    for body in [DIRECT, REPEATED, NESTED] {
        let source = program(body);
        assert_eq!(source.matches(STORE).count(), 1);
        assert_eq!(source.matches(body).count(), 1);
        assert!(source.contains("output: &mut DisjointSlice<u32, GridExclusive>"));
        assert!(source.contains("leader: &GridLeader"));
        assert!(source.contains("output.get_mut_exclusive(leader, index)"));
        assert!(source.contains("*slot = value"));
        assert!(source.contains("thread::grid_leader()"));
        assert!(!source.contains("unsafe"));
    }
}
