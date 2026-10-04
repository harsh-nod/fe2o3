use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceLaunchErrorV1, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
};

fn source_launch_roster_fixture() -> AdmittedInertSemanticMirV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let mut functions = Vec::new();
    for (ordinal, tag, symbol, workgroup, binding) in [
        (0_u8, 60_u8, None, [1, 1, 1], 0_u8),
        (1_u8, 70_u8, Some("alpha_export"), [64, 1, 1], 0xa1_u8),
        (2_u8, 75_u8, None, [1, 1, 1], 0_u8),
        (3_u8, 80_u8, Some("zeta_export"), [4, 4, 4], 0x7a_u8),
    ] {
        let kernel_root = symbol.is_some();
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            if kernel_root {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel_root {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let dimensions = SemanticWorkgroupDimensionsV1::new(workgroup).unwrap();
        let contract = SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            if kernel_root {
                SemanticFunctionRoleV1::KernelRoot
            } else {
                SemanticFunctionRoleV1::InternalHelper
            },
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(bytes(tag + 1)),
                unit,
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            )],
            SemanticBlockIdV1::from_index(0),
            if kernel_root {
                vec![
                    block(
                        90 + ordinal,
                        vec![],
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new(
                                SemanticFunctionIdV1::from_index(u32::from(ordinal - 1)),
                                vec![],
                                Some(SemanticCallDestinationV1::new(
                                    local_place(0, unit),
                                    SemanticControlFlowEdgeV1::new(
                                        SemanticEdgeRoleV1::CallReturn,
                                        SemanticBlockIdV1::from_index(1),
                                    ),
                                )),
                                SemanticUnwindActionV1::Unreachable,
                            )
                            .unwrap(),
                        ),
                    ),
                    block(110 + ordinal, vec![], SemanticTerminatorKindV1::Return),
                ]
            } else {
                vec![block(
                    90 + ordinal,
                    vec![],
                    SemanticTerminatorKindV1::Return,
                )]
            },
        )
        .unwrap();
        let function = match symbol {
            Some(symbol) => function.with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(symbol.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(binding)),
                contract,
            )),
            None => function,
        };
        functions.push(function);
    }
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(3),
        ],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap()
}

fn root_input(
    name: &str,
    binding: u8,
    rank: u8,
    workgroup: Option<[u32; 3]>,
    grid: [u32; 3],
) -> ProductionSourceLaunchRootInputV1<'_> {
    ProductionSourceLaunchRootInputV1::new(
        name,
        bytes(binding),
        ProductionSourceLaunchInputV1::new(rank, workgroup, grid),
    )
}

fn exact_inputs() -> [ProductionSourceLaunchRootInputV1<'static>; 2] {
    [
        root_input("logical_alpha", 0xa1, 1, Some([64, 1, 1]), [3, 1, 1]),
        root_input(
            "logical_zeta",
            0x7a,
            3,
            Some([4, 4, 4]),
            [u32::MAX, u32::from(u16::MAX), u32::from(u16::MAX)],
        ),
    ]
}

#[test]
fn source_roster_retains_exact_source_order_identity_and_geometry_without_ranked_ir() {
    let semantic = source_launch_roster_fixture();
    let inputs = exact_inputs();
    let roster = ProductionSourceLaunchRosterV1::try_new(&semantic, &inputs).unwrap();
    assert_eq!(
        roster.semantic_sha256(),
        semantic.semantic_sha256().as_bytes()
    );
    assert_eq!(roster.roots().len(), 2);
    assert_eq!(semantic.functions().len(), 4);
    assert_eq!(
        semantic.roots(),
        &[
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(3),
        ]
    );
    assert_eq!(
        semantic.functions()[0].role(),
        SemanticFunctionRoleV1::InternalHelper
    );
    assert_eq!(
        semantic.functions()[2].role(),
        SemanticFunctionRoleV1::InternalHelper
    );
    for (root, helper) in [(1, 0), (3, 2)] {
        let SemanticTerminatorKindV1::Call(call) =
            semantic.functions()[root].blocks()[0].terminator().kind()
        else {
            panic!("each sparse root must actually retain its preceding helper");
        };
        assert_eq!(call.callee().index(), helper);
    }
    for (ordinal, root) in roster.roots().iter().enumerate() {
        assert_eq!(root.selected_root(), semantic.roots()[ordinal]);
        assert_eq!(
            root.semantic_root_identity(),
            semantic.functions()[root.selected_root().index() as usize].identity()
        );
        assert!(root.layout().full_physical_workgroups());
        assert_eq!(root.layout().subgroup_size(), 64);
    }
    let first = roster.roots()[0];
    assert_eq!(first.kernel_binding(), bytes(0xa1));
    assert_eq!(first.source_rank(), 1);
    assert_eq!(
        first.source_launch(),
        ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1],)
    );
    assert_ne!(
        first.source_launch(),
        ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [4, 1, 1],)
    );
    assert_eq!(
        first.layout().grid_identity(),
        u64::from_le_bytes([0xa1; 8])
    );
    assert_eq!(first.layout().global_extents(), [192, 1, 1]);
    assert_eq!(first.layout().workgroup_extents(), [64, 1, 1]);
    let second = roster.roots()[1];
    assert_eq!(second.source_rank(), 3);
    assert_eq!(second.layout().global_extents(), [0, 0, 0]);
    assert_eq!(second.layout().workgroup_extents(), [4, 4, 4]);
    assert!(!roster.grants_artifact_or_launch_authority());
    drop(semantic);
    assert_eq!(roster.roots()[1], second);
}

#[test]
fn source_roster_preserves_order_count_name_and_full_binding_checks() {
    let semantic = source_launch_roster_fixture();
    let exact = exact_inputs();
    for (inputs, detail) in [
        (vec![], "an incomplete typed/semantic ranked root roster"),
        (
            vec![exact[0]],
            "an incomplete typed/semantic ranked root roster",
        ),
        (
            vec![exact[1], exact[0]],
            "a reordered or substituted typed/semantic kernel binding in the ranked roster",
        ),
        (
            vec![
                exact[0],
                root_input("logical_alpha", 0x7a, 3, Some([4, 4, 4]), [1, 1, 1]),
            ],
            "duplicate typed logical roots in the ranked roster",
        ),
        (
            vec![
                exact[0],
                root_input("logical_zeta", 0xa1, 3, Some([4, 4, 4]), [1, 1, 1]),
            ],
            "duplicate typed kernel bindings in the ranked roster",
        ),
        (
            vec![
                exact[0],
                root_input("logical_zeta", 0xff, 3, Some([4, 4, 4]), [1, 1, 1]),
            ],
            "a reordered or substituted typed/semantic kernel binding in the ranked roster",
        ),
    ] {
        assert_eq!(
            ProductionSourceLaunchRosterV1::try_new(&semantic, &inputs).unwrap_err(),
            ProductionSourceLaunchErrorV1::Unsupported(detail)
        );
    }
}

#[test]
fn source_roster_checks_every_root_workgroup_and_rank_without_publishing_partial_output() {
    let semantic = source_launch_roster_fixture();
    let first = exact_inputs()[0];
    for (second, expected) in [
        (
            root_input("logical_zeta", 0x7a, 3, Some([8, 4, 4]), [1, 1, 1]),
            ProductionSourceLaunchErrorV1::Unsupported(
                "authenticated LaunchContract workgroup disagrees with semantic source workgroup",
            ),
        ),
        (
            root_input("logical_zeta", 0x7a, 1, Some([4, 4, 4]), [1, 1, 1]),
            ProductionSourceLaunchErrorV1::Unsupported(
                "authenticated launch rank disagrees with source workgroup axes",
            ),
        ),
        (
            root_input("logical_zeta", 0x7a, 3, None, [1, 1, 1]),
            ProductionSourceLaunchErrorV1::Incomplete(
                "concurrency verification requires an exact authenticated LaunchContract workgroup",
            ),
        ),
    ] {
        assert_eq!(
            ProductionSourceLaunchRosterV1::try_new(&semantic, &[first, second]).unwrap_err(),
            expected
        );
    }
}

#[test]
fn source_roster_uses_full_binding_not_the_truncated_grid_identity() {
    let semantic = source_launch_roster_fixture();
    let mut inputs = exact_inputs();
    let mut changed = bytes(0xa1);
    changed[31] ^= 1;
    assert_eq!(&changed[..8], &bytes(0xa1)[..8]);
    inputs[0] = ProductionSourceLaunchRootInputV1::new(
        "logical_alpha",
        changed,
        ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
    );
    assert_eq!(
        ProductionSourceLaunchRosterV1::try_new(&semantic, &inputs).unwrap_err(),
        ProductionSourceLaunchErrorV1::Unsupported(
            "a reordered or substituted typed/semantic kernel binding in the ranked roster"
        )
    );
}

#[test]
fn source_roster_missing_required_workgroup_retains_incomplete_diagnostic() {
    let source = checked_arithmetic_owner();
    let inputs = [root_input(
        "logical_checked",
        49,
        1,
        Some([64, 1, 1]),
        [1, 1, 1],
    )];
    assert_eq!(
        ProductionSourceLaunchRosterV1::try_new(source.semantic(), &inputs).unwrap_err(),
        ProductionSourceLaunchErrorV1::Incomplete(
            "concurrency verification requires exact source workgroup dimensions"
        )
    );
}
