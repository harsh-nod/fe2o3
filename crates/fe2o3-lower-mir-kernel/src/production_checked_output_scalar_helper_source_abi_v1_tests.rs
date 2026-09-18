use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const F32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

#[derive(Clone, Copy)]
struct AbiCase {
    canon: SemanticCanonAbiV1,
    external: SemanticExternAbiV1,
    ownership: SemanticSourceArgumentOwnershipV1,
    argument: SemanticTypeIdV1,
    returned: SemanticTypeIdV1,
    unwind: bool,
    no_undef: bool,
    extension: SemanticAbiExtensionV1,
}

impl Default for AbiCase {
    fn default() -> Self {
        Self {
            canon: SemanticCanonAbiV1::Rust,
            external: SemanticExternAbiV1::Rust,
            ownership: SemanticSourceArgumentOwnershipV1::ByValue,
            argument: U32,
            returned: U32,
            unwind: false,
            no_undef: true,
            extension: SemanticAbiExtensionV1::None,
        }
    }
}

fn abi_value(
    ty: SemanticTypeIdV1,
    no_undef: bool,
    extension: SemanticAbiExtensionV1,
) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        if ty == UNIT {
            SemanticAbiPassModeV1::Ignore
        } else {
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, no_undef),
                    extension,
                    0,
                    None,
                )
                .unwrap(),
            )
        },
    )
}

fn source_types() -> Vec<SemanticTypeDeclV1> {
    let scalar = |tag, primitive, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    primitive,
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(shape),
        )
    };
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        scalar(
            2,
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            },
        ),
        scalar(
            3,
            SemanticBackendPrimitiveV1::float(32, 4),
            SemanticScalarTypeV1::Float { bits: 32 },
        ),
    ]
}

fn source_place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn zero(ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
    ))
}

fn source_block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let provenance = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        provenance,
        statements,
        SemanticTerminatorV1::new(provenance, terminator),
    )
    .unwrap()
}

fn source_decl(
    tag: u8,
    role: SemanticFunctionRoleV1,
    abi: SemanticFunctionAbiV1,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let provenance = SemanticSourceProvenanceV1::unavailable();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        role,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        provenance,
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 10 + i as u8; 32]),
                    *ty,
                    *role,
                    provenance,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

#[derive(Clone, Copy)]
enum EarlierCase {
    None,
    MissingReturn,
    WrongArgumentType,
    Orphan,
}

// This is genuine admitted semantic MIR, not a fabricated source/N owner.
// The root actually calls the helper, except in the explicitly orphaned case.
fn source_request(case: AbiCase, earlier: EarlierCase) -> InertSemanticMirRequestV1 {
    let root_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        abi_value(UNIT, true, SemanticAbiExtensionV1::None),
    )
    .unwrap();
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([90; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        case.canon,
        case.external,
        case.unwind,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(abi_value(
            case.argument,
            case.no_undef,
            case.extension,
        ))],
        abi_value(case.returned, true, SemanticAbiExtensionV1::None),
    )
    .unwrap()
    .with_source_argument_ownership(vec![case.ownership])
    .unwrap();
    let root_blocks = if matches!(earlier, EarlierCase::Orphan) {
        vec![source_block(21, vec![], SemanticTerminatorKindV1::Return)]
    } else {
        vec![
            source_block(
                21,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(1),
                        vec![zero(case.argument)],
                        Some(SemanticCallDestinationV1::new(
                            source_place(1, case.returned),
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
            source_block(22, vec![], SemanticTerminatorKindV1::Return),
        ]
    };
    let root = source_decl(
        20,
        SemanticFunctionRoleV1::KernelRoot,
        root_abi,
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (case.returned, SemanticLocalRoleV1::Temporary),
            // The exact source type closure includes both retained local types.
            (U32, SemanticLocalRoleV1::Temporary),
            (F32, SemanticLocalRoleV1::Temporary),
        ],
        root_blocks,
    );
    let statements = if case.returned == UNIT {
        vec![]
    } else {
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                source_place(0, case.returned),
                SemanticRvalueV1::new(
                    case.returned,
                    SemanticRvalueKindV1::Use(if case.argument == case.returned {
                        SemanticOperandV1::Copy(source_place(1, case.argument))
                    } else {
                        zero(case.returned)
                    }),
                ),
            )),
        )]
    };
    let helper = source_decl(
        90,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        &[
            (
                case.returned,
                if matches!(earlier, EarlierCase::MissingReturn) {
                    SemanticLocalRoleV1::Temporary
                } else {
                    SemanticLocalRoleV1::Return
                },
            ),
            (
                if matches!(earlier, EarlierCase::WrongArgumentType) {
                    F32
                } else {
                    case.argument
                },
                SemanticLocalRoleV1::Argument(0),
            ),
        ],
        vec![source_block(
            91,
            statements,
            SemanticTerminatorKindV1::Return,
        )],
    );
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source_types(),
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn admitted(case: AbiCase) -> AdmittedInertSemanticMirV1 {
    source_request(case, EarlierCase::None)
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap()
}

fn query(
    semantic: &AdmittedInertSemanticMirV1,
    function: u32,
    work_limit: usize,
) -> (R<bool>, usize, Option<usize>) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let result = source_helper_abi(
        semantic,
        SemanticFunctionIdV1::from_index(function),
        &mut budget,
    );
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR);
    assert_eq!(budget.failed_storage(), None);
    let accepted = budget.work();
    (result, accepted, work.failed_work())
}

fn abi_refusal(result: R<bool>, detail: &'static str) {
    assert!(
        matches!(result, Err(E::Unsupported { phase: "scalar helpers", detail: actual }) if actual == detail)
    );
}

#[test]
fn source_abi_accepts_reachable_direct_u32_and_exact_ignored_unit_only_as_component() {
    for returned in [U32, UNIT] {
        let semantic = admitted(AbiCase {
            returned,
            ..AbiCase::default()
        });
        let (result, work, failed) = query(&semantic, 1, 33);
        assert!(matches!(result, Ok(true)));
        assert_eq!((work, failed), (33, None));
        let (result, work, failed) = query(&semantic, 0, 3);
        assert!(matches!(result, Ok(false)));
        assert_eq!((work, failed), (3, None));
        let (result, work, failed) = query(&semantic, 2, 3);
        abi_refusal(result, "source function coordinate");
        assert_eq!((work, failed), (3, None));
    }
}

#[test]
fn source_abi_admitted_nonordinary_rust_conventions_are_not_scalar_helper_authority() {
    for (canon, external) in [
        (
            SemanticCanonAbiV1::C,
            SemanticExternAbiV1::C { unwind: false },
        ),
        (SemanticCanonAbiV1::RustCold, SemanticExternAbiV1::RustCold),
        (
            SemanticCanonAbiV1::RustPreserveNone,
            SemanticExternAbiV1::RustPreserveNone,
        ),
    ] {
        let semantic = admitted(AbiCase {
            canon,
            external,
            ..AbiCase::default()
        });
        let (result, work, failed) = query(&semantic, 1, 15);
        abi_refusal(result, "ordinary direct Rust helper ABI");
        assert_eq!((work, failed), (15, None));
    }
}

#[test]
fn source_abi_admitted_non_by_value_ownership_is_not_erased_by_direct_physical_mode() {
    for ownership in [
        SemanticSourceArgumentOwnershipV1::Unspecified,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
        SemanticSourceArgumentOwnershipV1::UniqueBorrow,
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        SemanticSourceArgumentOwnershipV1::RawPointer,
    ] {
        let semantic = admitted(AbiCase {
            ownership,
            ..AbiCase::default()
        });
        let (result, work, failed) = query(&semantic, 1, 23);
        abi_refusal(result, "direct by-value scalar helper arguments");
        assert_eq!((work, failed), (23, None));
    }
}

#[test]
fn source_abi_admitted_float_argument_and_return_refuse_at_distinct_checked_sites() {
    let semantic = admitted(AbiCase {
        argument: F32,
        returned: F32,
        ..AbiCase::default()
    });
    let (result, work, failed) = query(&semantic, 1, 23);
    abi_refusal(result, "direct by-value scalar helper arguments");
    assert_eq!((work, failed), (23, None));
    let semantic = admitted(AbiCase {
        returned: F32,
        ..AbiCase::default()
    });
    let (result, work, failed) = query(&semantic, 1, 29);
    abi_refusal(result, "direct scalar or exact Unit source return");
    assert_eq!((work, failed), (29, None));
}

#[test]
fn source_abi_exact_and_one_short_work_keep_original_floor_and_precedence() {
    let semantic = admitted(AbiCase::default());
    // 3 lookup + 12 ABI + 8 argument + 6 return + 2*2 actual locals.
    for (limit, accepted, failed) in [
        (2, 0, 3),
        (14, 3, 15),
        (22, 15, 23),
        (28, 23, 29),
        (30, 29, 31),
        (32, 31, 33),
    ] {
        let (result, actual, denied) = query(&semantic, 1, limit);
        assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
        assert_eq!((actual, denied), (accepted, Some(failed)));
    }
    let nonordinary = admitted(AbiCase {
        canon: SemanticCanonAbiV1::C,
        external: SemanticExternAbiV1::C { unwind: false },
        ..AbiCase::default()
    });
    let (result, accepted, failed) = query(&nonordinary, 1, 14);
    assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
    assert_eq!((accepted, failed), (3, Some(15)));
}

#[test]
fn source_abi_unwind_and_invalid_scalar_attributes_are_earlier_mir_refusals() {
    for case in [
        AbiCase {
            unwind: true,
            ..AbiCase::default()
        },
        AbiCase {
            no_undef: false,
            ..AbiCase::default()
        },
        AbiCase {
            extension: SemanticAbiExtensionV1::ZeroExtend,
            ..AbiCase::default()
        },
    ] {
        let result = source_request(case, EarlierCase::None)
            .admit_current_production(SemanticMirLimitsV1::default());
        assert!(matches!(
            result,
            Err(SemanticMirErrorV1::InvalidFunctionAbi)
        ));
    }
}

#[test]
fn source_abi_missing_return_and_wrong_argument_local_are_earlier_mir_refusals() {
    for earlier in [EarlierCase::MissingReturn, EarlierCase::WrongArgumentType] {
        let result = source_request(AbiCase::default(), earlier)
            .admit_current_production(SemanticMirLimitsV1::default());
        assert!(
            matches!(result, Err(SemanticMirErrorV1::InvalidLocalRoles { function }) if function == SemanticFunctionIdV1::from_index(1))
        );
    }
}

#[test]
fn source_abi_unreachable_helper_is_an_earlier_exact_source_closure_refusal() {
    let result = source_request(AbiCase::default(), EarlierCase::Orphan)
        .admit_current_production(SemanticMirLimitsV1::default());
    assert!(
        matches!(result, Err(SemanticMirErrorV1::FunctionOutsideRootClosure { function }) if function == SemanticFunctionIdV1::from_index(1))
    );
}

#[test]
fn source_abi_rust_call_tuple_field_cannot_masquerade_as_an_ordinary_rust_source_argument() {
    // is_source() includes this role, but the admitted ABI constructor rejects
    // it under ordinary Rust before source_helper_abi can observe such a row.
    let tuple_field = SemanticAbiArgumentV1::rust_call_tuple_field(
        0,
        abi_value(U32, true, SemanticAbiExtensionV1::None),
    );
    assert!(tuple_field.is_source());
    let result = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([90; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![U32],
        U32,
        vec![tuple_field],
        abi_value(U32, true, SemanticAbiExtensionV1::None),
    );
    assert!(matches!(
        result,
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    ));
}
