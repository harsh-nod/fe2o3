pub(super) mod private_capture_tests {
    use super::super::private_scalar_capture_v1::PrivateScalarReads;
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiCastV1, SemanticAbiRegisterKindV1, SemanticAbiRegisterV1, SemanticAbiUniformV1,
    };

    const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const FLOAT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const MUT_ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
    const SHARED_ENV: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    const RAW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

    #[derive(Clone, Copy)]
    enum Change {
        None,
        Write,
        Dead,
        Move,
        RawEscape,
        Repeat,
    }

    fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
    }

    fn dereference(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty).unwrap()],
            ty,
        )
        .unwrap()
    }

    fn float(value: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            FLOAT,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 4).unwrap()),
        ))
    }

    fn reference_type(
        tag: u8,
        pointee: SemanticTypeIdV1,
        mutability: SemanticMutabilityV1,
        kind: SemanticAbiPointeeKindV1,
        size: u64,
        alignment: u64,
    ) -> SemanticTypeDeclV1 {
        neutral_reference_type_v1(tag, pointee, mutability, kind).with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(kind, size, alignment).unwrap()),
                None,
            ),
        )
    }

    fn types() -> Vec<SemanticTypeDeclV1> {
        let unit = neutral_semantic_types_v1().remove(0);
        let float = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(221)),
            SemanticLayoutIdentityV1::from_sha256(bytes(221)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                neutral_scalar_backend_v1(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    u32::MAX.into(),
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        );
        let reference = reference_type(
            222,
            FLOAT,
            SemanticMutabilityV1::Immutable,
            SemanticAbiPointeeKindV1::SharedReference { frozen: true },
            4,
            4,
        );
        let environment = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(223)),
            SemanticLayoutIdentityV1::from_sha256(bytes(223)),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![REF]).unwrap()),
        );
        vec![
            unit,
            float,
            reference,
            environment,
            reference_type(
                224,
                ENV,
                SemanticMutabilityV1::Mutable,
                SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                8,
                8,
            ),
            reference_type(
                225,
                ENV,
                SemanticMutabilityV1::Immutable,
                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                8,
                8,
            ),
            neutral_pointer_type_v1(
                226,
                FLOAT,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
            ),
        ]
    }

    fn env_argument() -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            ENV,
            SemanticAbiPassModeV1::Cast {
                pad_i32: false,
                cast: SemanticAbiCastV1::new(
                    [None; 8],
                    None,
                    SemanticAbiUniformV1::new(
                        SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                        8,
                    )
                    .unwrap(),
                    SemanticAbiValueAttributesV1::plain(),
                ),
            },
        )
    }

    fn shared_env_argument() -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            SHARED_ENV,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        true,
                        Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                        true,
                        true,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    8,
                    Some(8),
                )
                .unwrap(),
            ),
        )
    }

    fn helper(
        tag: u8,
        input: SemanticAbiValueV1,
        ownership: SemanticSourceArgumentOwnershipV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(input)],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![ownership])
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn capture_owner(
        change: Change,
    ) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
        let baseline = neutral_ranked_program_v1();
        let semantic = baseline.semantic_ssa_owner.source_semantic();
        let original = &semantic.functions()[0];
        let capture = || {
            typed_assignment(
                3,
                ENV,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Aggregate,
                    vec![SemanticOperandV1::Copy(place(2, REF))],
                )
                .unwrap(),
            )
        };
        let mut root_statements = vec![
            typed_assignment(1, FLOAT, SemanticRvalueKindV1::Use(float(0x3f800000))),
            typed_assignment(
                2,
                REF,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(1, FLOAT),
                },
            ),
            capture(),
        ];
        match change {
            Change::Write => root_statements.push(typed_assignment(
                1,
                FLOAT,
                SemanticRvalueKindV1::Use(float(0x40000000)),
            )),
            Change::Dead => root_statements.push(statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(1),
            ))),
            Change::Move => root_statements.push(typed_assignment(
                5,
                FLOAT,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, FLOAT))),
            )),
            // A pointer cast may not preserve private reference authority.
            Change::RawEscape => root_statements.push(typed_assignment(
                6,
                RAW,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: SemanticOperandV1::Copy(place(2, REF)),
                },
            )),
            Change::None | Change::Repeat => {}
        }
        let mut root_blocks = vec![block(
            10,
            root_statements,
            neutral_test_call_v1(1, vec![SemanticOperandV1::Move(place(3, ENV))], 4, UNIT, 1),
        )];
        if matches!(change, Change::Repeat) {
            root_blocks.push(block(
                11,
                vec![capture()],
                neutral_test_call_v1(1, vec![SemanticOperandV1::Move(place(3, ENV))], 4, UNIT, 2),
            ));
        }
        root_blocks.push(block(12, vec![], SemanticTerminatorKindV1::Return));
        let root = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(10)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(10)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(10)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(10)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(10)),
            SemanticSourceProvenanceV1::unavailable(),
            original.abi().clone(),
            vec![
                local(1, UNIT, SemanticLocalRoleV1::Return),
                local(2, FLOAT, SemanticLocalRoleV1::Temporary),
                local(3, REF, SemanticLocalRoleV1::Temporary),
                local(4, ENV, SemanticLocalRoleV1::Temporary),
                local(5, UNIT, SemanticLocalRoleV1::Temporary),
                local(6, FLOAT, SemanticLocalRoleV1::Temporary),
                local(7, RAW, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            root_blocks,
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
        let wrapper = helper(
            20,
            env_argument(),
            SemanticSourceArgumentOwnershipV1::ByValue,
            vec![
                local(1, UNIT, SemanticLocalRoleV1::Return),
                local(2, ENV, SemanticLocalRoleV1::Argument(0)),
                local(3, MUT_ENV, SemanticLocalRoleV1::Temporary),
                local(4, SHARED_ENV, SemanticLocalRoleV1::Temporary),
                local(5, UNIT, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    20,
                    vec![
                        typed_assignment(
                            2,
                            MUT_ENV,
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Mutable,
                                place: place(1, ENV),
                            },
                        ),
                        typed_assignment(
                            3,
                            SHARED_ENV,
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: dereference(2, ENV),
                            },
                        ),
                    ],
                    neutral_test_call_v1(
                        2,
                        vec![SemanticOperandV1::Move(place(3, SHARED_ENV))],
                        4,
                        UNIT,
                        1,
                    ),
                ),
                block(21, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ENV).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), REF).unwrap(),
            ],
            REF,
        )
        .unwrap();
        let leaf = helper(
            30,
            shared_env_argument(),
            SemanticSourceArgumentOwnershipV1::SharedBorrow,
            vec![
                local(1, UNIT, SemanticLocalRoleV1::Return),
                local(2, SHARED_ENV, SemanticLocalRoleV1::Argument(0)),
                local(3, REF, SemanticLocalRoleV1::Temporary),
                local(4, FLOAT, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(
                30,
                vec![
                    typed_assignment(
                        2,
                        REF,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
                    ),
                    typed_assignment(
                        3,
                        FLOAT,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference(2, FLOAT))),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        );
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            types(),
            vec![],
            vec![],
            vec![],
            vec![root, wrapper, leaf],
            (0..3)
                .map(|function| {
                    SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(function))
                })
                .collect(),
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let owner = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        ProductionSemanticSsaOwnerV1::try_new(
            owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
    }

    fn classified(owner: &ProductionSemanticSsaOwnerV1) -> usize {
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let reads = PrivateScalarReads::for_root(owner, root).unwrap();
        view.body()
            .blocks()
            .iter()
            .flat_map(|block| block.statements())
            .filter(|statement| reads.contains(view.body(), statement))
            .count()
    }

    #[test]
    fn private_scalar_capture_fixture_reference_abi_matches_nonzero_pointees() {
        let types = types();
        for ty in [REF, MUT_ENV, SHARED_ENV] {
            let declaration = &types[ty.index() as usize];
            let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
                panic!("capture reference must be a pointer");
            };
            let layout = types[pointer.pointee().index() as usize].layout();
            let pointee = declaration.abi_properties().first_pointee().unwrap();
            assert_eq!(pointee.guaranteed_size_bytes(), layout.rustc_size_bytes());
            assert_eq!(pointee.reliable_alignment_bytes(), layout.alignment_bytes());
        }
        let argument = shared_env_argument();
        let SemanticAbiPassModeV1::Direct(attributes) = argument.mode() else {
            panic!("capture reference argument must be direct");
        };
        let pointee = types[SHARED_ENV.index() as usize]
            .abi_properties()
            .first_pointee()
            .unwrap();
        assert_eq!(
            attributes.pointee_size_bytes(),
            pointee.guaranteed_size_bytes()
        );
        assert_eq!(
            attributes.pointee_alignment_bytes(),
            Some(pointee.reliable_alignment_bytes())
        );
    }

    #[test]
    fn private_scalar_capture_checked_move_field_and_shared_reborrow() {
        let owner = capture_owner(Change::None).unwrap();
        let bytes = owner.source_semantic().canonical_encoding().to_vec();
        assert_eq!(classified(&owner), 1);
        assert_eq!(owner.source_semantic().canonical_encoding(), bytes);
        owner.verify_replay().unwrap();
    }

    #[test]
    fn private_scalar_capture_repeated_call_instances_remain_distinct() {
        assert_eq!(classified(&capture_owner(Change::Repeat).unwrap()), 2);
    }

    #[test]
    fn private_scalar_capture_rejects_write_move_death_and_cast_escape() {
        for change in [Change::Write, Change::Move, Change::Dead] {
            if let Ok(owner) = capture_owner(change) {
                assert_eq!(classified(&owner), 0);
            }
        }
        let escaped = capture_owner(Change::RawEscape)
            .expect("a typed reference-to-raw-pointer cast must reach capture analysis");
        assert_eq!(classified(&escaped), 0);
    }

    #[test]
    fn private_scalar_capture_certificate_is_exact_statement_and_owner_bound() {
        let owner = capture_owner(Change::None).unwrap();
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
        let statement = view
            .body()
            .blocks()
            .iter()
            .flat_map(|block| block.statements())
            .find(|statement| reads.contains(view.body(), statement))
            .unwrap();
        assert!(!reads.contains(view.body(), &statement.clone()));
        let other = capture_owner(Change::None).unwrap();
        let other_view = other.execution_view_for_root(root).unwrap();
        assert!(!reads.contains(other_view.body(), statement));
    }

    #[test]
    fn private_scalar_capture_ranked_hook_requires_the_exact_checked_read() {
        let owner = capture_owner(Change::None).unwrap();
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let function = view.body();
        let types = owner.source_semantic().types();
        let reads = PrivateScalarReads::for_root(&owner, root).unwrap();
        let (block, read) = function
            .blocks()
            .iter()
            .enumerate()
            .find_map(|(block, body)| {
                body.statements()
                    .iter()
                    .find(|statement| reads.contains(function, statement))
                    .map(|statement| (block, statement))
            })
            .unwrap();
        let contracts = ProjectionLocalContractsV1 {
            checked_references: CheckedReferencesV1 {
                origins: vec![None; function.locals().len()],
                option_dominance: SemanticOptionDominanceV1::analyze(function, &[]).unwrap(),
                enum_payload_dominance: SemanticEnumPayloadDominanceV1::analyze(function, types)
                    .unwrap(),
            },
            allocations: vec![None; function.locals().len()],
            allocation_provenance: vec![None; function.locals().len()],
        };
        let audit = |statement, certificate| {
            let mut operations = Vec::new();
            let mut sources = Vec::new();
            let result = project_statement_accesses(
                types,
                function,
                block,
                &[],
                statement,
                &ProjectedGlobalSemanticUsesV1::default(),
                certificate,
                &vec![None; function.locals().len()],
                &contracts,
                &[],
                &mut Vec::new(),
                &mut vec![None; function.locals().len()],
                &mut operations,
                &mut sources,
                &mut 0,
                &mut String::new(),
            );
            assert!(operations.is_empty());
            assert!(sources.is_empty());
            result
        };
        assert!(matches!(
            audit(read, None),
            Err(ProductionRankedProjectionErrorV1::UnrankedDereference(_))
        ));
        audit(read, Some(&reads)).unwrap();
        assert!(matches!(
            audit(&read.clone(), Some(&reads)),
            Err(ProductionRankedProjectionErrorV1::UnrankedDereference(_))
        ));
    }

    include!("private_scalar_capture_v1/aggregate_read_tests.rs");
    include!("private_scalar_capture_v1/exclusive_aggregate_tests.rs");
}
