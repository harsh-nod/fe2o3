// Synthetic B1a component controls. No actual factory, source or bounds authority.
mod bounds_extent_preparation_controls {
    use super::super::bf16_nominal_source_preparation_v1::with_rich_tables_for_test_v1;
    use super::super::root_bounds_extent_preparation_v1::{
        self as extent, BoundsExtentArgumentsV1,
    };
    use super::super::root_bounds_source_scan_v1::BoundsSourceStorageV1;
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
    use ranked_projection_source_v1::resource;
    use slice_extent_projection_v1::{Context, Scratch};
    type Error = ProductionRankedProjectionErrorV1;
    const LIMIT: usize = 16 * 1024 * 1024;
    const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const USIZE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
    struct OriginalContext<'a> {
        scratch: &'a mut Scratch,
        facts: &'a mut dyn ProjectedAssertionFactsV1,
    }

    // BEGIN FROZEN INDEPENDENT ORIGINAL DECISION ORACLE
    impl OriginalContext<'_> {
        pub(super) fn extent(
            &mut self,
            types: &[SemanticTypeDeclV1],
            function: &SemanticFunctionDeclV1,
            length: SemanticLocalIdV1,
            definitions: &[BoundsLocalDefinitionV1<'_>],
            next_argument: &mut usize,
        ) -> Result<Option<ProductionRankedValueV1>, Error> {
            self.facts.charge_private_array_work(16)?;
            let local = length.index() as usize;
            let Some(definition) = definitions.get(local) else {
                return Ok(None);
            };
            let Some(value) = definition.value else {
                return Ok(None);
            };
            if definition.count != 1
                || self.scratch.definitions.get(local) != Some(&1)
                || self.scratch.escaped.get(local) != Some(&false)
                || function
                    .locals()
                    .get(local)
                    .is_none_or(|local| local.ty() != value.result_type())
            {
                return Ok(None);
            }
            let receiver = match value.kind() {
                SemanticRvalueKindV1::Length(place)
                    if matches!(place.projections(), [projection]
                if projection.kind() == SemanticProjectionKindV1::Dereference) =>
                {
                    place.local()
                }
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand,
                } => match simple_operand_local(operand) {
                    Some(local) => local,
                    None => return Ok(None),
                },
                _ => return Ok(None),
            };
            if !self.metadata_preserving_origin(
                types,
                function,
                receiver,
                value.result_type(),
                definitions,
            )? {
                return Ok(None);
            }
            let origin = self.scratch.origins[receiver.index() as usize]
                .ok_or_else(|| resource(Resource::Accounting))? as usize;
            let Some(slot) = self.scratch.arguments.get(origin) else {
                return Err(resource(Resource::Accounting));
            };
            if slot.is_none()
                && *next_argument >= fe2o3_pliron::HARD_MAX_PRODUCTION_RANKED_ARGUMENTS
            {
                return Err(Error::Unsupported(
                    "slice metadata exceeds the ranked argument limit",
                ));
            }
            self.facts.charge_private_array_work(8)?;
            let value = original_project_runtime_slice_extent_argument_v1(
                receiver.index() as usize,
                &self.scratch.origins,
                &mut self.scratch.arguments,
                next_argument,
            )?;
            Ok(Some(value))
        }

        fn metadata_preserving_origin(
            &mut self,
            types: &[SemanticTypeDeclV1],
            function: &SemanticFunctionDeclV1,
            receiver: SemanticLocalIdV1,
            length_type: SemanticTypeIdV1,
            definitions: &[BoundsLocalDefinitionV1<'_>],
        ) -> Result<bool, Error> {
            let Some(declaration) = function.locals().get(receiver.index() as usize) else {
                return Ok(false);
            };
            let ty = declaration.ty();
            let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Ok(false);
            };
            if pointer.kind() != SemanticPointerKindV1::Reference
                || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
                || !matches!(
                    types
                        .get(pointer.pointee().index() as usize)
                        .map(SemanticTypeDeclV1::shape),
                    Some(SemanticTypeShapeV1::Slice { .. })
                )
            {
                return Ok(false);
            }
            // Older admitted MIR records usize structurally. The exact metadata
            // operation, not a nominal tag or any arbitrary integer, supplies length.
            if types.get(length_type.index() as usize).is_none_or(|length| {
            !matches!(
                length.rust_type_kind(),
                SemanticRustTypeKindV1::Ordinary | SemanticRustTypeKindV1::Usize
            ) || !matches!(
                length.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits })
                    if *bits == pointer.pointer_width_bits()
            )
        }) {
            return Ok(false);
        }
            let Some(origin) = self
                .scratch
                .origins
                .get(receiver.index() as usize)
                .copied()
                .flatten()
            else {
                return Ok(false);
            };
            let mut current = receiver;
            // Stable scalar provenance also follows casts. Metadata equality needs
            // the stronger whole-value, identical-type copy/move chain below.
            for _ in 0..64 {
                self.facts.charge_private_array_work(16)?;
                let index = current.index() as usize;
                let Some(local) = function.locals().get(index) else {
                    return Ok(false);
                };
                let Some(definition) = definitions.get(index) else {
                    return Ok(false);
                };
                if local.ty() != ty
                    || self.scratch.escaped.get(index) != Some(&false)
                    || self.scratch.origins.get(index) != Some(&Some(origin))
                {
                    return Ok(false);
                }
                if let SemanticLocalRoleV1::Argument(argument) = local.role() {
                    return Ok(argument == origin
                        && definition.count == 0
                        && self.scratch.definitions.get(index) == Some(&0)
                        && function
                            .abi()
                            .adjusted_arguments()
                            .get(argument as usize)
                            .is_some_and(|argument| argument.ty() == ty));
                }
                if definition.count != 1 || self.scratch.definitions.get(index) != Some(&1) {
                    return Ok(false);
                }
                let Some(value) = definition.value else {
                    return Ok(false);
                };
                let SemanticRvalueKindV1::Use(operand) = value.kind() else {
                    return Ok(false);
                };
                let Some(source) = simple_operand_local(operand) else {
                    return Ok(false);
                };
                if value.result_type() != ty || operand.ty() != ty {
                    return Ok(false);
                }
                current = source;
            }
            Ok(false)
        }
    }

    fn original_project_runtime_slice_extent_argument_v1(
        receiver: usize,
        stable_argument_origins: &[Option<u32>],
        arguments: &mut [Option<u32>],
        next_argument: &mut usize,
    ) -> Result<ProductionRankedValueV1, ProductionRankedProjectionErrorV1> {
        let origin = stable_argument_origins
            .get(receiver)
            .copied()
            .flatten()
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "a volatile load slice length without one stable kernel-argument origin",
            ))? as usize;
        let slot =
            arguments
                .get_mut(origin)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "a volatile load slice extent origin outside the semantic local table",
                ))?;
        let argument = match *slot {
            Some(argument) => argument,
            None => {
                let argument = u32::try_from(*next_argument).map_err(|_| {
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "too many volatile load slice extent arguments",
                    )
                })?;
                *next_argument = next_argument.checked_add(1).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "volatile load slice extent argument count overflow",
                    ),
                )?;
                *slot = Some(argument);
                argument
            }
        };
        Ok(ProductionRankedValueV1::Argument(argument))
    }

    // END FROZEN INDEPENDENT ORIGINAL DECISION ORACLE
    fn types() -> Vec<SemanticTypeDeclV1> {
        let mut types = volatile_load_source_types_v1(
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Immutable,
        );
        types.push(
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(230)),
                SemanticLayoutIdentityV1::from_sha256(bytes(230)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 64,
                }),
            )
            .with_rust_type_kind(SemanticRustTypeKindV1::Usize),
        );
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(231)),
            SemanticLayoutIdentityV1::from_sha256(bytes(231)),
            SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
        ));
        types
    }

    fn slice_place(receiver: u32, index: Option<u32>) -> SemanticPlaceV1 {
        let mut projections =
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap()];
        if let Some(index) = index {
            projections.push(
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(index)),
                    SCALAR_TYPE,
                )
                .unwrap(),
            );
        }
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(receiver),
            projections,
            if index.is_some() { SCALAR_TYPE } else { SLICE },
        )
        .unwrap()
    }

    fn rebuild(
        original: &SemanticFunctionDeclV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            locals,
            original.entry(),
            blocks,
        )
        .unwrap()
    }

    fn fixture(cast: bool) -> SemanticFunctionDeclV1 {
        let base = projection_function(vec![block(232, vec![], SemanticTerminatorKindV1::Return)]);
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(233)),
            SemanticLayoutIdentityV1::from_sha256(bytes(233)),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![
                SemanticAbiValueV1::new(
                    REFERENCE,
                    SemanticAbiPassModeV1::Pair {
                        first: SemanticAbiValueAttributesV1::plain(),
                        second: SemanticAbiValueAttributesV1::plain(),
                    },
                ),
                SemanticAbiValueV1::new(
                    REFERENCE,
                    SemanticAbiPassModeV1::Pair {
                        first: SemanticAbiValueAttributesV1::plain(),
                        second: SemanticAbiValueAttributesV1::plain(),
                    },
                ),
                SemanticAbiValueV1::new(
                    USIZE,
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                ),
            ],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let mut locals = vec![
            local(234, SCALAR_TYPE, SemanticLocalRoleV1::Return),
            local(235, REFERENCE, SemanticLocalRoleV1::Argument(0)),
            local(236, REFERENCE, SemanticLocalRoleV1::Argument(1)),
            local(237, USIZE, SemanticLocalRoleV1::Argument(2)),
            local(238, REFERENCE, SemanticLocalRoleV1::Temporary),
            local(239, REFERENCE, SemanticLocalRoleV1::Temporary),
        ];
        for n in 6..12 {
            locals.push(local(
                234 + n,
                if n < 9 { USIZE } else { BOOL },
                SemanticLocalRoleV1::Temporary,
            ));
        }
        let mut prefix = vec![
            typed_assignment(
                4,
                REFERENCE,
                if cast {
                    SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Transmute,
                        operand: typed_operand(1, REFERENCE),
                    }
                } else {
                    SemanticRvalueKindV1::Use(typed_operand(1, REFERENCE))
                },
            ),
            typed_assignment(
                5,
                REFERENCE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(typed_place(4, REFERENCE))),
            ),
            typed_assignment(6, USIZE, SemanticRvalueKindV1::Length(slice_place(4, None))),
            typed_assignment(
                7,
                USIZE,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: typed_operand(5, REFERENCE),
                },
            ),
            typed_assignment(8, USIZE, SemanticRvalueKindV1::Length(slice_place(2, None))),
        ];
        let mut blocks = Vec::new();
        for n in 0..3_u32 {
            let mut statements = if n == 0 {
                std::mem::take(&mut prefix)
            } else {
                vec![typed_assignment(
                    0,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(slice_place(
                        if n == 1 { 4 } else { 5 },
                        Some(3),
                    ))),
                )]
            };
            statements.push(typed_assignment(
                9 + n,
                BOOL,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: typed_operand(3, USIZE),
                    right: typed_operand(6 + n, USIZE),
                },
            ));
            blocks.push(block(
                246 + n as u8,
                statements,
                SemanticTerminatorKindV1::Assert {
                    condition: typed_operand(9 + n, BOOL),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: typed_operand(6 + n, USIZE),
                        index: typed_operand(3, USIZE),
                    },
                    target: cfg_edge(SemanticEdgeRoleV1::AssertSuccess, n + 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ));
        }
        blocks.push(block(
            249,
            vec![typed_assignment(
                0,
                SCALAR_TYPE,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(slice_place(2, Some(3)))),
            )],
            SemanticTerminatorKindV1::Return,
        ));
        SemanticFunctionDeclV1::new(
            base.identity(),
            base.role(),
            base.item_definition_identity(),
            base.monomorphization_identity(),
            base.generic_type_arguments_identity(),
            base.const_generic_arguments_identity(),
            base.source(),
            abi,
            locals,
            base.entry(),
            blocks,
        )
        .unwrap()
    }

    fn scratch(types: &[SemanticTypeDeclV1], function: &SemanticFunctionDeclV1) -> Scratch {
        let inventory = assertion_definition_inventory(function).unwrap();
        Scratch {
            origins: local_stable_argument_origins(types, function).unwrap(),
            arguments: vec![None; function.locals().len()],
            definitions: inventory.counts,
            escaped: inventory.address_escaped,
        }
    }

    fn definitions(function: &SemanticFunctionDeclV1) -> Vec<BoundsLocalDefinitionV1<'_>> {
        let mut definitions = vec![BoundsLocalDefinitionV1::default(); function.locals().len()];
        for block in function.blocks() {
            for statement in block.statements() {
                if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                    && assignment.destination().projections().is_empty()
                {
                    let slot = &mut definitions[assignment.destination().local().index() as usize];
                    slot.count += 1;
                    slot.value = Some(assignment.value());
                }
            }
        }
        definitions
    }

    #[derive(Default)]
    struct TraceFacts {
        calls: Vec<usize>,
        deny: Option<usize>,
    }
    impl ProjectedAssertionFactsV1 for TraceFacts {
        fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Error> {
            let call = self.calls.len();
            self.calls.push(amount);
            if self.deny == Some(call) {
                Err(Error::Unsupported("injected work denial"))
            } else {
                Ok(())
            }
        }
        fn private_array_initializer_count(
            &mut self,
            _: usize,
            _: usize,
        ) -> Result<Option<u64>, Error> {
            panic!("extent controls must not request initializer authority")
        }
        fn is_materialized_block(&mut self, _: usize) -> Result<bool, Error> {
            panic!("extent controls must not request materialization authority")
        }
        fn condition(
            &mut self,
            _: usize,
            _: bool,
            _: SemanticBlockIdV1,
        ) -> Result<canonical_assertion_facts_v1::ProjectedAssertionConditionV1, Error> {
            panic!("extent controls must not request assertion authority")
        }
    }
    fn copy_scratch(seed: &Scratch) -> Scratch {
        Scratch {
            origins: seed.origins.clone(),
            arguments: seed.arguments.clone(),
            definitions: seed.definitions.clone(),
            escaped: seed.escaped.clone(),
        }
    }
    #[derive(Debug, PartialEq)]
    struct Outcome {
        values: Vec<String>,
        arguments: Vec<Option<u32>>,
        next: usize,
        work: Vec<usize>,
    }
    fn parity(
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        definitions: &[BoundsLocalDefinitionV1<'_>],
        seed: &Scratch,
        lengths: &[u32],
        next: usize,
        deny: Option<usize>,
    ) -> Outcome {
        let mut old = copy_scratch(seed);
        let mut new = copy_scratch(seed);
        let mut old_next = next;
        let mut new_next = next;
        let mut old_facts = TraceFacts {
            deny,
            ..Default::default()
        };
        let mut new_facts = TraceFacts {
            deny,
            ..Default::default()
        };
        let mut old_values = Vec::new();
        let mut new_values = Vec::new();
        for &length in lengths {
            old_values.push(format!(
                "{:?}",
                OriginalContext {
                    scratch: &mut old,
                    facts: &mut old_facts
                }
                .extent(
                    types,
                    function,
                    SemanticLocalIdV1::from_index(length),
                    definitions,
                    &mut old_next
                )
            ));
            new_values.push(format!(
                "{:?}",
                Context {
                    scratch: &mut new,
                    facts: &mut new_facts
                }
                .extent(
                    types,
                    function,
                    SemanticLocalIdV1::from_index(length),
                    definitions,
                    &mut new_next
                )
            ));
        }
        assert_eq!(new.origins, old.origins);
        assert_eq!(new.definitions, old.definitions);
        assert_eq!(new.escaped, old.escaped);
        let old = Outcome {
            values: old_values,
            arguments: old.arguments,
            next: old_next,
            work: old_facts.calls,
        };
        let new = Outcome {
            values: new_values,
            arguments: new.arguments,
            next: new_next,
            work: new_facts.calls,
        };
        assert_eq!(new, old);
        new
    }
    fn is_none(result: &Outcome) {
        assert!(result.values.iter().all(|v| v == "Ok(None)"));
    }

    #[test]
    fn extent_legacy_oracle_copy_move_reuse_and_distinct_origins() {
        let types = types();
        let function = fixture(false);
        let seed = scratch(&types, &function);
        let actual = parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6, 7, 6, 8],
            3,
            None,
        );
        assert_eq!(actual.arguments[0], Some(3));
        assert_eq!(actual.arguments[1], Some(4));
        assert_eq!(actual.next, 5);
        assert_eq!(
            actual.work,
            [16, 16, 16, 8, 16, 16, 16, 16, 8, 16, 16, 16, 8, 16, 16, 8]
        );
    }
    #[test]
    fn extent_legacy_oracle_missing_definition_and_entry_short_circuits() {
        let types = types();
        let function = fixture(false);
        for case in 0..8 {
            let mut seed = scratch(&types, &function);
            let mut rows = definitions(&function);
            let length = match case {
                0 => 99,
                1 => {
                    rows.clear();
                    6
                }
                2 => {
                    rows[6].value = None;
                    6
                }
                3 => {
                    rows[6].count = 2;
                    6
                }
                4 => {
                    seed.definitions[6] = 2;
                    6
                }
                5 => {
                    seed.escaped[6] = true;
                    6
                }
                6 => {
                    seed.definitions.truncate(6);
                    6
                }
                _ => {
                    seed.escaped.truncate(6);
                    6
                }
            };
            let actual = parity(&types, &function, &rows, &seed, &[length], 3, None);
            is_none(&actual);
            assert_eq!(actual.work, [16]);
            assert_eq!(actual.next, 3);
        }
    }
    #[test]
    fn extent_legacy_oracle_metadata_origin_alias_escape_and_abi_refusals() {
        let types = types();
        let function = fixture(false);
        for case in 0..9 {
            let mut seed = scratch(&types, &function);
            let mut rows = definitions(&function);
            match case {
                0 => seed.origins[4] = None,
                1 => seed.origins[4] = Some(1),
                2 => seed.origins[1] = Some(1),
                3 => seed.escaped[4] = true,
                4 => seed.definitions[4] = 2,
                5 => rows[4].count = 0,
                6 => rows[4].value = None,
                7 => rows[1].count = 1,
                _ => seed.definitions[1] = 1,
            }
            let actual = parity(&types, &function, &rows, &seed, &[6], 3, None);
            is_none(&actual);
            assert_eq!(actual.next, 3);
        }
        // Argument origin and ABI entry must agree even if the scalar origin rows are made consistent.
        let mut seed = scratch(&types, &function);
        seed.origins[4] = Some(2);
        seed.origins[1] = Some(2);
        is_none(&parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6],
            3,
            None,
        ));
    }
    #[test]
    fn extent_legacy_oracle_same_type_cast_is_not_copy_metadata() {
        let types = types();
        let function = fixture(true);
        let seed = scratch(&types, &function);
        let actual = parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6, 7],
            3,
            None,
        );
        is_none(&actual);
        assert_eq!(actual.next, 3);
    }
    #[test]
    fn extent_legacy_oracle_result_width_signedness_and_rust_kind() {
        let function = fixture(false);
        for kind in [
            SemanticRustTypeKindV1::Ordinary,
            SemanticRustTypeKindV1::Usize,
            SemanticRustTypeKindV1::Isize,
        ] {
            for (bits, signed) in [(64, false), (32, false), (64, true)] {
                let mut types = types();
                types[3] = SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256(bytes(230)),
                    SemanticLayoutIdentityV1::from_sha256(bytes(230)),
                    SemanticTypeLayoutV1::new(Some(u64::from(bits / 8)), u64::from(bits / 8))
                        .unwrap(),
                    SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { bits, signed }),
                )
                .with_rust_type_kind(kind);
                let seed = scratch(&types, &function);
                let actual = parity(
                    &types,
                    &function,
                    &definitions(&function),
                    &seed,
                    &[6],
                    3,
                    None,
                );
                assert_eq!(
                    actual.next,
                    if bits == 64 && !signed && kind != SemanticRustTypeKindV1::Isize {
                        4
                    } else {
                        3
                    }
                );
            }
        }
    }
    #[test]
    fn extent_legacy_oracle_missing_pointer_and_slice_types() {
        let function = fixture(false);
        let original = types();
        let seed = scratch(&original, &function);
        for n in 0..4 {
            let mut types = original.clone();
            types.truncate(n);
            is_none(&parity(
                &types,
                &function,
                &definitions(&function),
                &seed,
                &[6],
                3,
                None,
            ));
        }
        let mut types = original.clone();
        types[1] = original[0].clone();
        is_none(&parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6],
            3,
            None,
        ));
        let mut types = original.clone();
        types[2] = original[0].clone();
        is_none(&parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6],
            3,
            None,
        ));
    }
    #[test]
    fn extent_legacy_oracle_hard_max_reuse_and_malformed_slot() {
        let types = types();
        let function = fixture(false);
        let mut seed = scratch(&types, &function);
        let max = fe2o3_pliron::HARD_MAX_PRODUCTION_RANKED_ARGUMENTS;
        let refused = parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6],
            max,
            None,
        );
        assert!(refused.values[0].contains("slice metadata exceeds the ranked argument limit"));
        assert_eq!(refused.work, [16, 16, 16]);
        assert_eq!(refused.next, max);
        seed.arguments[0] = Some(u32::MAX);
        let reused = parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6],
            usize::MAX,
            None,
        );
        assert_eq!(reused.arguments[0], Some(u32::MAX));
        assert_eq!(reused.next, usize::MAX);
        seed.arguments.clear();
        let malformed = parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6],
            3,
            None,
        );
        assert!(malformed.values[0].contains("Accounting"));
        assert_eq!(malformed.work, [16, 16, 16]);
    }
    #[test]
    fn extent_legacy_oracle_denial_at_every_decision_charge_preserves_mutation_order() {
        let types = types();
        let function = fixture(false);
        let seed = scratch(&types, &function);
        for deny in 0..5 {
            let actual = parity(
                &types,
                &function,
                &definitions(&function),
                &seed,
                &[7],
                3,
                Some(deny),
            );
            assert!(actual.values[0].contains("injected work denial"));
            assert_eq!(actual.next, 3);
            assert!(actual.arguments.iter().all(Option::is_none));
            assert_eq!(actual.work.len(), deny + 1);
        }
        let actual = parity(
            &types,
            &function,
            &definitions(&function),
            &seed,
            &[6, 7],
            3,
            Some(4),
        );
        assert_eq!(actual.arguments[0], Some(3));
        assert_eq!(actual.next, 4);
    }
    fn chain(links: usize, cycle: bool) -> (SemanticFunctionDeclV1, Scratch, u32) {
        let base = fixture(false);
        let mut locals = base.locals()[..4].to_vec();
        let mut statements = Vec::new();
        for i in 0..links {
            let target = 4 + i as u32;
            locals.push(local(
                100 + i as u8,
                REFERENCE,
                SemanticLocalRoleV1::Temporary,
            ));
            let source = if cycle && i == 0 {
                4 + (links - 1) as u32
            } else if i == 0 {
                1
            } else {
                target - 1
            };
            statements.push(typed_assignment(
                target,
                REFERENCE,
                SemanticRvalueKindV1::Use(typed_operand(source, REFERENCE)),
            ));
        }
        let length = 4 + links as u32;
        locals.push(local(200, USIZE, SemanticLocalRoleV1::Temporary));
        statements.push(typed_assignment(
            length,
            USIZE,
            SemanticRvalueKindV1::Length(slice_place(length - 1, None)),
        ));
        let function = rebuild(
            &base,
            locals,
            vec![block(210, statements, SemanticTerminatorKindV1::Return)],
        );
        let n = function.locals().len();
        let mut seed = Scratch {
            origins: vec![None; n],
            arguments: vec![None; n],
            definitions: vec![0; n],
            escaped: vec![false; n],
        };
        seed.origins[1] = Some(0);
        seed.origins[2] = Some(1);
        seed.origins[3] = Some(2);
        for i in 4..length as usize {
            seed.origins[i] = Some(0);
            seed.definitions[i] = 1;
        }
        seed.definitions[length as usize] = 1;
        (function, seed, length)
    }
    #[test]
    fn extent_legacy_oracle_exact_sixty_four_link_boundary_and_cycle() {
        let types = types();
        for (links, cycle, valid) in [(63, false, true), (64, false, false), (2, true, false)] {
            let (function, seed, length) = chain(links, cycle);
            let actual = parity(
                &types,
                &function,
                &definitions(&function),
                &seed,
                &[length],
                3,
                None,
            );
            assert_eq!(actual.next, if valid { 4 } else { 3 });
            assert_eq!(actual.work.iter().filter(|&&n| n == 16).count(), 65);
            if !valid {
                is_none(&actual);
            }
        }
    }
    #[test]
    fn extent_original_argument_helper_error_order_and_u32_usize_edges() {
        for next in [0, 7, u32::MAX as usize, usize::MAX] {
            for cached in [None, Some(19)] {
                let origins = [Some(0)];
                let mut old = [cached];
                let mut new = [cached];
                let mut old_next = next;
                let mut new_next = next;
                let old_result = original_project_runtime_slice_extent_argument_v1(
                    0,
                    &origins,
                    &mut old,
                    &mut old_next,
                );
                let new_result =
                    project_runtime_slice_extent_argument_v1(0, &origins, &mut new, &mut new_next);
                assert_eq!(format!("{old_result:?}"), format!("{new_result:?}"));
                assert_eq!(old, new);
                assert_eq!(old_next, new_next);
            }
        }
        for origins in [vec![], vec![None], vec![Some(9)]] {
            let mut old = [None];
            let mut new = [None];
            let mut old_next = 3;
            let mut new_next = 3;
            assert_eq!(
                format!(
                    "{:?}",
                    original_project_runtime_slice_extent_argument_v1(
                        0,
                        &origins,
                        &mut old,
                        &mut old_next
                    )
                ),
                format!(
                    "{:?}",
                    project_runtime_slice_extent_argument_v1(0, &origins, &mut new, &mut new_next)
                )
            );
            assert_eq!(old, new);
            assert_eq!(old_next, new_next);
            assert_eq!(new_next, 3);
        }
    }

    // The callback's result and storage never escape. This is synthetic component
    // scaffolding, not a source-authority constructor or an actual factory.
    macro_rules! in_paid {
        ($types:ident,$function:ident,$rich:ident,$budget:ident,$owned:ident,$storage:ident,$view:ident,$body:block) => {{
            let mut work=Work::new(LIMIT); let mut outer_budget=Budget::new(&mut work,LIMIT);
            let outcome=with_rich_tables_for_test_v1(&[],&$types,&$function,&mut outer_budget,|$rich,$budget|{
                let floor=$budget.storage(); let mut $owned=0;
                let mut $storage=BoundsSourceStorageV1::new();
                $storage.scan(&$function,&mut PreparationResourcesV1::new($budget,&mut $owned)).unwrap();
                $storage.initialize_values(&$function,&[],&[],&mut PreparationResourcesV1::new($budget,&mut $owned)).unwrap();
                let $view=$storage.view(&$function,&mut PreparationResourcesV1::new($budget,&mut $owned)).unwrap();
                let _ = &$view;
                $body
                drop($view); drop($storage);
                assert_eq!($budget.storage(),floor+$owned);
                $budget.release_storage($owned).unwrap(); assert_eq!($budget.storage(),floor);
                Ok(())
            });
            assert_eq!(outer_budget.storage(),0);
            outcome
        }};
    }
    #[test]
    fn extent_paid_exact_header_slots_work_and_borrowed_query_debits() {
        let types = types();
        let function = fixture(false);
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let initial = vec![None; function.locals().len()];
            let mut state = BoundsExtentArgumentsV1::new();
            let before_work = budget.work();
            let before_storage = budget.storage();
            let before_owned = owned;
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let frame = extent::extent_frame_v1().unwrap();
            assert_eq!(budget.work() - before_work, frame + initial.len() + 2);
            assert_eq!(
                budget.storage() - before_storage,
                frame + initial.len() * std::mem::size_of::<Option<u32>>()
            );
            assert_eq!(owned - before_owned, budget.storage() - before_storage);
            assert_eq!(
                extent::test_access::state(&state),
                (true, true, initial.len(), initial.len(), 3)
            );
            for (length, value, work) in [(6, 3, 63), (7, 3, 81), (8, 4, 45)] {
                let before = budget.work();
                assert_eq!(
                    state
                        .extent(
                            &types,
                            &function,
                            rich,
                            &view,
                            SemanticLocalIdV1::from_index(length),
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .unwrap(),
                    Some(ProductionRankedValueV1::Argument(value))
                );
                assert_eq!(budget.work() - before, work);
            }
            let before = budget.work();
            let result = state
                .view(
                    &types,
                    &function,
                    rich,
                    &view,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            assert_eq!(budget.work() - before, 2);
            assert_eq!(result.argument_slots[0], Some(3));
            assert_eq!(result.argument_slots[1], Some(4));
            assert_eq!(result.next_argument, 5);
            assert_eq!(
                budget.storage() - before_storage,
                frame + initial.len() * std::mem::size_of::<Option<u32>>()
            );
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_matches_frozen_decisions_for_copy_move_and_cast() {
        for cast in [false, true] {
            let types = types();
            let function = fixture(cast);
            let seed = scratch(&types, &function);
            let expected = parity(
                &types,
                &function,
                &definitions(&function),
                &seed,
                &[6, 7, 8],
                7,
                None,
            );
            in_paid!(types, function, rich, budget, owned, storage, view, {
                let mut state = BoundsExtentArgumentsV1::new();
                state
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &seed.arguments,
                        7,
                        &mut PreparationResourcesV1::new(budget, &mut owned),
                    )
                    .unwrap();
                let mut results = Vec::new();
                for length in [6, 7, 8] {
                    results.push(format!(
                        "{:?}",
                        state.extent(
                            &types,
                            &function,
                            rich,
                            &view,
                            SemanticLocalIdV1::from_index(length),
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                    ));
                }
                assert_eq!(results, expected.values);
                assert_eq!(extent::test_access::copy_rows(&state), expected.arguments);
                assert_eq!(extent::test_access::state(&state).4, expected.next);
                drop(state);
            })
            .unwrap();
        }
    }
    #[test]
    fn extent_paid_preserves_supplied_nonempty_slots_and_counter() {
        let types = types();
        let function = fixture(false);
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut state = BoundsExtentArgumentsV1::new();
            let mut initial = vec![None; function.locals().len()];
            initial[0] = Some(11);
            initial[5] = Some(23);
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    24,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            assert_eq!(
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(6),
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .unwrap(),
                Some(ProductionRankedValueV1::Argument(11))
            );
            assert_eq!(
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(8),
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .unwrap(),
                Some(ProductionRankedValueV1::Argument(24))
            );
            let rows = extent::test_access::copy_rows(&state);
            assert_eq!(rows[5], Some(23));
            assert_eq!(rows[1], Some(24));
            assert_eq!(extent::test_access::state(&state).4, 25);
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_unmetered_reused_and_capacity_only_owners_refuse() {
        let types = types();
        let function = fixture(false);
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let initial = vec![None; function.locals().len()];
            let mut state = BoundsExtentArgumentsV1::new();
            assert!(
                state
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::unmetered()
                    )
                    .is_err()
            );
            assert_eq!(extent::test_access::state(&state), (false, false, 0, 0, 0));
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let before = (
                budget.work(),
                budget.storage(),
                owned,
                extent::test_access::state(&state),
            );
            assert!(
                state
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            assert_eq!(
                before,
                (
                    budget.work(),
                    budget.storage(),
                    owned,
                    extent::test_access::state(&state)
                )
            );
            let mut capacity = BoundsExtentArgumentsV1::new();
            extent::test_access::occupy_capacity(&mut capacity);
            assert!(
                capacity
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            assert!(!extent::test_access::state(&capacity).0);
            drop(capacity);
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_malformed_roster_retains_header_and_started_state() {
        let types = types();
        let function = fixture(false);
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut state = BoundsExtentArgumentsV1::new();
            let before = owned;
            assert!(matches!(
                state.initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &[],
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned)
                ),
                Err(Error::Unsupported(
                    "bounds extent argument slots do not match the semantic local table"
                ))
            ));
            assert_eq!(owned - before, extent::extent_frame_v1().unwrap());
            assert_eq!(extent::test_access::state(&state), (true, false, 0, 0, 0));
            assert!(
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(6),
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            assert!(
                state
                    .view(
                        &types,
                        &function,
                        rich,
                        &view,
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_foreign_b0_same_content_function_is_rejected() {
        let types = types();
        let function = fixture(false);
        let foreign = function.clone();
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut other = BoundsSourceStorageV1::new();
            other
                .scan(
                    &foreign,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            other
                .initialize_values(
                    &foreign,
                    &[],
                    &[],
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let foreign_view = other
                .view(
                    &foreign,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let mut state = BoundsExtentArgumentsV1::new();
            let initial = vec![None; function.locals().len()];
            let result = state.initialize(
                &types,
                &function,
                rich,
                &foreign_view,
                &initial,
                3,
                &mut PreparationResourcesV1::new(budget, &mut owned),
            );
            assert!(matches!(
                result,
                Err(Error::Unsupported(
                    "bounds source view differs from the requested lexical source"
                ))
            ));
            assert_eq!(extent::test_access::state(&state), (true, false, 0, 0, 0));
            drop(state);
            drop(foreign_view);
            drop(other);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_foreign_function_and_same_content_type_slice_rejected_at_query() {
        let types = types();
        let function = fixture(false);
        let foreign = function.clone();
        let foreign_types = types.clone();
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut state = BoundsExtentArgumentsV1::new();
            let initial = vec![None; function.locals().len()];
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let before = extent::test_access::copy_rows(&state);
            assert!(
                state
                    .extent(
                        &types,
                        &foreign,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(6),
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            assert!(
                state
                    .extent(
                        &foreign_types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(6),
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            assert!(
                state
                    .view(
                        &types[..types.len() - 1],
                        &function,
                        rich,
                        &view,
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            assert_eq!(extent::test_access::copy_rows(&state), before);
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_foreign_budget_and_work_query_and_constructor_refuse() {
        let types = types();
        let function = fixture(false);
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut state = BoundsExtentArgumentsV1::new();
            let initial = vec![None; function.locals().len()];
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let mut other_work = Work::new(LIMIT);
            let mut other_budget = Budget::new(&mut other_work, LIMIT);
            let mut other_owned = 0;
            assert!(
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(6),
                        &mut PreparationResourcesV1::new(&mut other_budget, &mut other_owned)
                    )
                    .is_err()
            );
            let mut foreign = BoundsExtentArgumentsV1::new();
            assert!(
                foreign
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::new(&mut other_budget, &mut other_owned)
                    )
                    .is_err()
            );
            assert_eq!(extent::test_access::state(&foreign), (true, false, 0, 0, 0));
            assert_eq!(other_owned, extent::extent_frame_v1().unwrap());
            drop(foreign);
            other_budget.release_storage(other_owned).unwrap();
            assert_eq!(other_budget.storage(), 0);
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_foreign_rich_lexical_function_and_ledger_refuse() {
        let types = types();
        let function = fixture(false);
        let foreign = function.clone();
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let initial = vec![None; function.locals().len()];
            let mut state = BoundsExtentArgumentsV1::new();
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            with_rich_tables_for_test_v1(&[], &types, &foreign, budget, |foreign_rich, budget| {
                assert!(
                    state
                        .extent(
                            &types,
                            &function,
                            foreign_rich,
                            &view,
                            SemanticLocalIdV1::from_index(6),
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                Ok(())
            })
            .unwrap();
            let mut other_work = Work::new(LIMIT);
            let mut other_budget = Budget::new(&mut other_work, LIMIT);
            with_rich_tables_for_test_v1(
                &[],
                &types,
                &function,
                &mut other_budget,
                |foreign_rich, _| {
                    assert!(
                        state
                            .extent(
                                &types,
                                &function,
                                foreign_rich,
                                &view,
                                SemanticLocalIdV1::from_index(6),
                                &mut PreparationResourcesV1::new(budget, &mut owned)
                            )
                            .is_err()
                    );
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(other_budget.storage(), 0);
            assert_eq!(extent::test_access::state(&state).4, 3);
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_header_work_one_short_retains_started_not_credit() {
        let types = types();
        let function = fixture(false);
        assert!(
            in_paid!(types, function, rich, budget, owned, storage, view, {
                let mut state = BoundsExtentArgumentsV1::new();
                let initial = vec![None; function.locals().len()];
                let h = extent::extent_frame_v1().unwrap();
                budget.charge_work(LIMIT - budget.work() - (h - 1)).unwrap();
                let before = owned;
                assert!(
                    state
                        .initialize(
                            &types,
                            &function,
                            rich,
                            &view,
                            &initial,
                            3,
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                assert_eq!(owned, before);
                assert_eq!(extent::test_access::state(&state), (true, false, 0, 0, 0));
                assert!(budget.failed_work().is_some());
                drop(state);
            })
            .is_err()
        );
    }
    #[test]
    fn extent_paid_header_storage_one_short_retains_started_not_credit() {
        let types = types();
        let function = fixture(false);
        assert!(
            in_paid!(types, function, rich, budget, owned, storage, view, {
                let mut state = BoundsExtentArgumentsV1::new();
                let initial = vec![None; function.locals().len()];
                let h = extent::extent_frame_v1().unwrap();
                let burn = LIMIT - budget.storage() - (h - 1);
                budget.reserve_storage(burn).unwrap();
                let before = owned;
                assert!(
                    state
                        .initialize(
                            &types,
                            &function,
                            rich,
                            &view,
                            &initial,
                            3,
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                assert_eq!(owned, before);
                assert_eq!(extent::test_access::state(&state), (true, false, 0, 0, 0));
                assert!(budget.failed_storage().is_some());
                drop(state);
                budget.release_storage(burn).unwrap();
            })
            .is_err()
        );
    }
    #[test]
    fn extent_paid_slot_storage_one_short_retains_accepted_header() {
        let types = types();
        let function = fixture(false);
        assert!(
            in_paid!(types, function, rich, budget, owned, storage, view, {
                let mut state = BoundsExtentArgumentsV1::new();
                let initial = vec![None; function.locals().len()];
                let h = extent::extent_frame_v1().unwrap();
                let slot_bytes = initial.len() * std::mem::size_of::<Option<u32>>();
                let burn = LIMIT - budget.storage() - (h + slot_bytes - 1);
                budget.reserve_storage(burn).unwrap();
                let before = owned;
                assert!(
                    state
                        .initialize(
                            &types,
                            &function,
                            rich,
                            &view,
                            &initial,
                            3,
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                assert_eq!(owned - before, h);
                assert_eq!(extent::test_access::state(&state), (true, false, 0, 0, 0));
                assert!(budget.failed_storage().is_some());
                drop(state);
                budget.release_storage(burn).unwrap();
            })
            .is_err()
        );
    }
    #[test]
    fn extent_paid_query_work_denial_preserves_prior_slot_and_sticky_refusal() {
        let types = types();
        let function = fixture(false);
        assert!(
            in_paid!(types, function, rich, budget, owned, storage, view, {
                let mut state = BoundsExtentArgumentsV1::new();
                let initial = vec![None; function.locals().len()];
                state
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::new(budget, &mut owned),
                    )
                    .unwrap();
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(6),
                        &mut PreparationResourcesV1::new(budget, &mut owned),
                    )
                    .unwrap();
                let before = extent::test_access::copy_rows(&state);
                let credits = owned;
                // Second origin costs 45; one short refuses the final 8 before any slot mutation.
                budget.charge_work(LIMIT - budget.work() - 44).unwrap();
                assert!(
                    state
                        .extent(
                            &types,
                            &function,
                            rich,
                            &view,
                            SemanticLocalIdV1::from_index(8),
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                assert_eq!(extent::test_access::copy_rows(&state), before);
                assert_eq!(extent::test_access::state(&state).4, 4);
                assert_eq!(owned, credits);
                assert!(
                    state
                        .view(
                            &types,
                            &function,
                            rich,
                            &view,
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                drop(state);
            })
            .is_err()
        );
    }
    #[test]
    fn extent_paid_unwind_retains_owner_payload_and_credit_until_outer_drop() {
        let types = types();
        let function = fixture(false);
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut state = BoundsExtentArgumentsV1::new();
            let initial = vec![None; function.locals().len()];
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                state
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::new(budget, &mut owned),
                    )
                    .unwrap();
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(6),
                        &mut PreparationResourcesV1::new(budget, &mut owned),
                    )
                    .unwrap();
                panic!("intentional caller unwind after extent mutation");
            }));
            assert!(caught.is_err());
            assert_eq!(extent::test_access::state(&state).4, 4);
            assert_eq!(extent::test_access::copy_rows(&state)[0], Some(3));
            assert!(owned >= extent::extent_frame_v1().unwrap());
            assert!(
                state
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .is_err()
            );
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_frame_checked_arithmetic_refuses_overflow() {
        extent::test_access::checked_frame_overflow();
    }

    #[test]
    fn extent_legacy_oracle_raw_receiver_and_abi_type_mismatch() {
        let function = fixture(false);
        let reference_types = types();
        let seed = scratch(&reference_types, &function);
        let mut raw_types = volatile_load_source_types_v1(
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
        );
        raw_types.extend_from_slice(&reference_types[3..]);
        is_none(&parity(
            &raw_types,
            &function,
            &definitions(&function),
            &seed,
            &[6],
            3,
            None,
        ));
        let mut locals = function.locals().to_vec();
        locals[1] = local(235, USIZE, SemanticLocalRoleV1::Argument(0));
        let mismatched = rebuild(&function, locals, function.blocks().to_vec());
        is_none(&parity(
            &reference_types,
            &mismatched,
            &definitions(&mismatched),
            &seed,
            &[6],
            3,
            None,
        ));
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(233)),
            SemanticLayoutIdentityV1::from_sha256(bytes(233)),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            vec![SemanticAbiValueV1::new(
                USIZE,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )],
            SemanticAbiValueV1::new(SCALAR_TYPE, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let mismatched = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            function.locals().to_vec(),
            function.entry(),
            function.blocks().to_vec(),
        )
        .unwrap();
        is_none(&parity(
            &reference_types,
            &mismatched,
            &definitions(&mismatched),
            &seed,
            &[6],
            3,
            None,
        ));
    }
    #[test]
    fn extent_paid_foreign_b0_query_and_unmetered_view_do_not_mutate() {
        let types = types();
        let function = fixture(false);
        let foreign = function.clone();
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut state = BoundsExtentArgumentsV1::new();
            let initial = vec![None; function.locals().len()];
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let mut other = BoundsSourceStorageV1::new();
            other
                .scan(
                    &foreign,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            other
                .initialize_values(
                    &foreign,
                    &[],
                    &[],
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let foreign_view = other
                .view(
                    &foreign,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            assert!(matches!(
                state.extent(
                    &types,
                    &function,
                    rich,
                    &foreign_view,
                    SemanticLocalIdV1::from_index(6),
                    &mut PreparationResourcesV1::new(budget, &mut owned)
                ),
                Err(Error::Unsupported(
                    "bounds source view differs from the requested lexical source"
                ))
            ));
            assert!(
                state
                    .view(
                        &types,
                        &function,
                        rich,
                        &view,
                        &mut PreparationResourcesV1::unmetered()
                    )
                    .is_err()
            );
            assert_eq!(extent::test_access::copy_rows(&state), initial);
            assert_eq!(extent::test_access::state(&state).4, 3);
            drop(foreign_view);
            drop(other);
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_preexisting_denial_refuses_without_starting() {
        let types = types();
        let function = fixture(false);
        assert!(
            in_paid!(types, function, rich, budget, owned, storage, view, {
                let mut state = BoundsExtentArgumentsV1::new();
                let initial = vec![None; function.locals().len()];
                assert!(budget.charge_work(LIMIT).is_err());
                let before = owned;
                assert!(
                    state
                        .initialize(
                            &types,
                            &function,
                            rich,
                            &view,
                            &initial,
                            3,
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                assert_eq!(extent::test_access::state(&state), (false, false, 0, 0, 0));
                assert_eq!(owned, before);
                drop(state);
            })
            .is_err()
        );
    }

    #[test]
    fn extent_frame_roster_has_separate_nested_call_envelopes() {
        extent::test_access::audit_frame_rows();
    }
    #[test]
    fn extent_frame_roster_and_b0_query_checked_overflow() {
        extent::test_access::frame_call_overflow();
    }

    fn projected_metadata_function(projections: usize) -> SemanticFunctionDeclV1 {
        let base = fixture(false);
        let mut blocks = base.blocks().to_vec();
        let mut statements = blocks[0].statements().to_vec();
        let operand = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(5),
                (0..projections)
                    .map(|_| {
                        SemanticProjectionV1::new(SemanticProjectionKindV1::OpaqueCast, REFERENCE)
                            .unwrap()
                    })
                    .collect(),
                REFERENCE,
            )
            .unwrap(),
        );
        statements[3] = typed_assignment(
            7,
            USIZE,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand,
            },
        );
        blocks[0] = block(246, statements, blocks[0].terminator().kind().clone());
        rebuild(&base, base.locals().to_vec(), blocks)
    }
    #[test]
    fn extent_legacy_long_projection_scan_keeps_exact_old_work_and_result() {
        let types = types();
        for count in [1, 64, 2048] {
            let function = projected_metadata_function(count);
            let seed = scratch(&types, &function);
            let actual = parity(
                &types,
                &function,
                &definitions(&function),
                &seed,
                &[7],
                3,
                None,
            );
            is_none(&actual);
            assert_eq!(actual.work, [16]);
            assert_eq!(actual.next, 3);
        }
    }
    #[test]
    fn extent_paid_long_projection_scan_prepays_every_visit_without_mutation() {
        let types = types();
        let count = 2048;
        let function = projected_metadata_function(count);
        in_paid!(types, function, rich, budget, owned, storage, view, {
            let mut state = BoundsExtentArgumentsV1::new();
            let initial = vec![None; function.locals().len()];
            state
                .initialize(
                    &types,
                    &function,
                    rich,
                    &view,
                    &initial,
                    3,
                    &mut PreparationResourcesV1::new(budget, &mut owned),
                )
                .unwrap();
            let before = budget.work();
            let credits = owned;
            assert_eq!(
                state
                    .extent(
                        &types,
                        &function,
                        rich,
                        &view,
                        SemanticLocalIdV1::from_index(7),
                        &mut PreparationResourcesV1::new(budget, &mut owned)
                    )
                    .unwrap(),
                None
            );
            assert_eq!(budget.work() - before, 20 + count);
            assert_eq!(extent::test_access::copy_rows(&state), initial);
            assert_eq!(extent::test_access::state(&state).4, 3);
            assert_eq!(owned, credits);
            drop(state);
        })
        .unwrap();
    }
    #[test]
    fn extent_paid_projection_denial_precedes_the_existing_scan() {
        let types = types();
        let count = 2048;
        let function = projected_metadata_function(count);
        assert!(
            in_paid!(types, function, rich, budget, owned, storage, view, {
                let mut state = BoundsExtentArgumentsV1::new();
                let initial = vec![None; function.locals().len()];
                state
                    .initialize(
                        &types,
                        &function,
                        rich,
                        &view,
                        &initial,
                        3,
                        &mut PreparationResourcesV1::new(budget, &mut owned),
                    )
                    .unwrap();
                budget
                    .charge_work(LIMIT - budget.work() - (20 + count - 1))
                    .unwrap();
                let before = budget.work();
                let credits = owned;
                assert!(
                    state
                        .extent(
                            &types,
                            &function,
                            rich,
                            &view,
                            SemanticLocalIdV1::from_index(7),
                            &mut PreparationResourcesV1::new(budget, &mut owned)
                        )
                        .is_err()
                );
                assert!(budget.failed_work().is_some());
                assert_eq!(budget.work() - before, 20);
                assert_eq!(extent::test_access::copy_rows(&state), initial);
                assert_eq!(extent::test_access::state(&state).4, 3);
                assert_eq!(owned, credits);
                drop(state);
            })
            .is_err()
        );
    }
    #[test]
    fn extent_projection_helpers_have_independent_nested_frames() {
        extent::test_access::audit_projection_frame_rows();
    }
}
