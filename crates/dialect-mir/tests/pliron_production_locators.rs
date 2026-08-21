#![cfg(feature = "pliron")]
#![forbid(unsafe_code)]

use dialect_mir::pliron::{
    MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1, MirProductionBlockLocatorV1,
    MirProductionFunctionLocatorV1, MirProductionLocatorErrorV1, MirProductionModuleHandleV1,
    MirProductionModuleLocatorV1, MirProductionPlironLimitsV1, MirProductionPlironResourceV1,
    MirProductionSemanticSha256V1, MirProductionStatementLocatorV1, MirProductionSuccessorArcV1,
    MirProductionTerminatorLocatorV1, mir_dialect_registration, register_mir_dialect,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticEdgeRoleV1, SemanticFunctionIdV1,
};
use fe2o3_pliron::{PlironSession, ShellLimits};
use pliron::context::Context;

fn block(
    id: u32,
    statements: u32,
    successors: Vec<MirProductionSuccessorArcV1>,
) -> MirProductionBlockLocatorV1 {
    MirProductionBlockLocatorV1::try_new(
        SemanticBlockIdV1::from_index(id),
        (0..statements)
            .map(MirProductionStatementLocatorV1::new)
            .collect(),
        MirProductionTerminatorLocatorV1::try_new(successors).unwrap(),
    )
    .unwrap()
}

fn function(
    id: u32,
    entry: u32,
    blocks: Vec<MirProductionBlockLocatorV1>,
) -> MirProductionFunctionLocatorV1 {
    MirProductionFunctionLocatorV1::try_new(
        SemanticFunctionIdV1::from_index(id),
        SemanticBlockIdV1::from_index(entry),
        blocks,
    )
    .unwrap()
}

fn representative_module() -> MirProductionModuleLocatorV1 {
    let functions = vec![
        function(
            0,
            2,
            vec![
                block(
                    0,
                    0,
                    vec![MirProductionSuccessorArcV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(2),
                    )],
                ),
                block(1, 1, vec![]),
                block(
                    2,
                    2,
                    vec![
                        MirProductionSuccessorArcV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(1),
                        ),
                        MirProductionSuccessorArcV1::new(
                            SemanticEdgeRoleV1::SwitchValue,
                            SemanticBlockIdV1::from_index(1),
                        ),
                        MirProductionSuccessorArcV1::new(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            SemanticBlockIdV1::from_index(0),
                        ),
                    ],
                ),
            ],
        ),
        function(1, 0, vec![block(0, 0, vec![])]),
    ];
    MirProductionModuleLocatorV1::try_new(
        MirProductionSemanticSha256V1::from_sha256([0xa5; 32]),
        functions,
    )
    .unwrap()
}

#[test]
fn real_pliron_ops_round_trip_nonzero_entry_source_order_and_duplicate_arcs() {
    let expected = representative_module();
    let mut context = Context::new();
    register_mir_dialect(&mut context);

    let handle = MirProductionModuleHandleV1::try_new(
        &mut context,
        expected.clone(),
        MirProductionPlironLimitsV1::default(),
    )
    .unwrap();

    assert_eq!(handle.verify(&context), Ok(()));
    assert_eq!(handle.snapshot(&context).unwrap(), expected);
    assert_eq!(
        handle.snapshot(&context).unwrap().functions()[0]
            .entry_block_id()
            .index(),
        2
    );
    assert_eq!(
        handle.snapshot(&context).unwrap().functions()[0]
            .blocks()
            .iter()
            .map(|block| block.block_id().index())
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(!handle.grants_authority());
}

#[test]
fn snapshots_are_deterministic_across_independent_contexts() {
    let expected = representative_module();
    let mut first = Context::new();
    let mut second = Context::new();
    register_mir_dialect(&mut first);
    register_mir_dialect(&mut second);

    let first_handle = MirProductionModuleHandleV1::try_new(
        &mut first,
        expected.clone(),
        MirProductionPlironLimitsV1::default(),
    )
    .unwrap();
    let second_handle = MirProductionModuleHandleV1::try_new(
        &mut second,
        expected,
        MirProductionPlironLimitsV1::default(),
    )
    .unwrap();

    assert_eq!(
        first_handle.snapshot(&first).unwrap(),
        second_handle.snapshot(&second).unwrap()
    );
    assert_eq!(
        first_handle.verify(&second),
        Err(MirProductionLocatorErrorV1::ForeignContext)
    );
}

#[test]
fn dialect_registration_hook_installs_the_real_production_schema() {
    let registration = mir_dialect_registration().unwrap();
    let mut session = PlironSession::new(ShellLimits::default(), [registration]).unwrap();
    let expected = representative_module();

    session
        .with_context_mut(|context| {
            let handle = MirProductionModuleHandleV1::try_new(
                context,
                expected.clone(),
                MirProductionPlironLimitsV1::default(),
            )
            .unwrap();
            assert_eq!(handle.snapshot(context).unwrap(), expected);
        })
        .unwrap();
}

#[test]
fn production_successors_do_not_inherit_the_legacy_256_arc_cap() {
    let arcs = (0..257)
        .map(|_| {
            MirProductionSuccessorArcV1::new(
                SemanticEdgeRoleV1::SwitchValue,
                SemanticBlockIdV1::from_index(0),
            )
        })
        .collect::<Vec<_>>();
    assert!(MAX_PRODUCTION_LOCATOR_SUCCESSOR_ARCS_V1 > arcs.len());
    let locator = MirProductionModuleLocatorV1::try_new(
        MirProductionSemanticSha256V1::from_sha256([7; 32]),
        vec![function(0, 0, vec![block(0, 0, arcs)])],
    )
    .unwrap();
    let mut context = Context::new();
    register_mir_dialect(&mut context);

    let handle = MirProductionModuleHandleV1::try_new(
        &mut context,
        locator.clone(),
        MirProductionPlironLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(handle.snapshot(&context).unwrap(), locator);
}

#[test]
fn closed_recipes_reject_noncanonical_duplicate_and_dangling_locators() {
    let terminator = MirProductionTerminatorLocatorV1::try_new(vec![]).unwrap();
    assert!(matches!(
        MirProductionBlockLocatorV1::try_new(
            SemanticBlockIdV1::from_index(0),
            vec![MirProductionStatementLocatorV1::new(1)],
            terminator.clone(),
        ),
        Err(MirProductionLocatorErrorV1::NonCanonicalStatementOrdinal {
            expected: 0,
            found: 1
        })
    ));

    let reversed = MirProductionFunctionLocatorV1::try_new(
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(1),
        vec![block(1, 0, vec![]), block(0, 0, vec![])],
    );
    assert!(matches!(
        reversed,
        Err(MirProductionLocatorErrorV1::NonCanonicalBlockId {
            expected: 0,
            found: 1
        })
    ));

    let dangling = MirProductionFunctionLocatorV1::try_new(
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(0),
        vec![block(
            0,
            0,
            vec![MirProductionSuccessorArcV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(1),
            )],
        )],
    );
    assert!(matches!(
        dangling,
        Err(MirProductionLocatorErrorV1::DanglingSuccessor {
            block: 0,
            target: 1
        })
    ));

    let duplicate_function = MirProductionModuleLocatorV1::try_new(
        MirProductionSemanticSha256V1::from_sha256([3; 32]),
        vec![
            function(0, 0, vec![block(0, 0, vec![])]),
            function(0, 0, vec![block(0, 0, vec![])]),
        ],
    );
    assert!(matches!(
        duplicate_function,
        Err(MirProductionLocatorErrorV1::NonCanonicalFunctionId {
            expected: 1,
            found: 0
        })
    ));
}

#[test]
fn named_middle_end_limit_rejects_before_materialization_without_poisoning_context() {
    let locator = representative_module();
    let mut context = Context::new();
    register_mir_dialect(&mut context);
    let rejection = MirProductionModuleHandleV1::try_new(
        &mut context,
        locator.clone(),
        MirProductionPlironLimitsV1::new(1).unwrap(),
    );
    assert!(matches!(
        rejection,
        Err(
            MirProductionLocatorErrorV1::MiddleEndResourceLimitExceeded {
                resource: MirProductionPlironResourceV1::TreeWork,
                ..
            }
        )
    ));

    let handle = MirProductionModuleHandleV1::try_new(
        &mut context,
        locator,
        MirProductionPlironLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(handle.verify(&context), Ok(()));
}
