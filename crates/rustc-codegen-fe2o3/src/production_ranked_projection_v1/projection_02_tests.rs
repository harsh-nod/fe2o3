    #[test]
    fn neutral_scan_projection_and_lowering_bind_kind_and_complete_effect_stream() {
        let mut recipe_identities = Vec::new();
        for elements in [1_u32, 3, 65, 255] {
            for kind in [
                SemanticWorkgroupScanKindV1::Inclusive,
                SemanticWorkgroupScanKindV1::Exclusive,
            ] {
                let program = neutral_scan_ranked_program_v1(kind, elements);
                let sources = &program.roots[0].executable_effect_sources;
                assert_eq!(sources.len(), (5 * scan_rounds_v1(elements) + 4) as usize);
                let recipe = sources[0].recipe_identity();
                assert_ne!(recipe, [0; 32]);
                assert!(
                    sources
                        .iter()
                        .all(|source| source.recipe_identity() == recipe)
                );
                recipe_identities.push(recipe);

                let ProductionRankedSemanticProgramV1 {
                    semantic_ssa_owner,
                    roots,
                } = program;
                let semantic_owner = semantic_ssa_owner.into_source_owner().unwrap();
                let root = roots.into_vec().into_iter().next().unwrap();
                let receipt = ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate_with_generated_effects(
                    semantic_owner,
                    root.lowering,
                    root.ranked_ir,
                    root.access_sources,
                    root.executable_effect_sources,
                )
                .unwrap();
                let _ = fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
                    receipt,
                    fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                    1,
                )
                .expect("the exact scan projection and generated KIR recipe must validate");
            }
        }
        assert_eq!(
            recipe_identities.iter().collect::<BTreeSet<_>>().len(),
            recipe_identities.len(),
            "kind and exact extent must both contribute to recipe identity"
        );
    }

    #[test]
    fn neutral_scan_full_path_rejects_kind_recipe_and_partial_effect_substitution() {
        let exclusive = neutral_scan_ranked_program_v1(SemanticWorkgroupScanKindV1::Exclusive, 65);
        let exclusive_recipe = exclusive.roots[0].executable_effect_sources[0].recipe_identity();
        let inclusive = neutral_scan_ranked_program_v1(SemanticWorkgroupScanKindV1::Inclusive, 65);
        let ProductionRankedSemanticProgramV1 {
            semantic_ssa_owner,
            roots,
        } = inclusive;
        let semantic_owner = semantic_ssa_owner.into_source_owner().unwrap();
        let mut root = roots.into_vec().into_iter().next().unwrap();
        root.executable_effect_sources = root
            .executable_effect_sources
            .into_iter()
            .map(|source| {
                ProductionRankedExecutableEffectSourceV1::new(
                    source.semantic_block(),
                    source.semantic_effect_ordinal(),
                    source.ranked_block(),
                    source.ranked_operation(),
                    source.origin(),
                    exclusive_recipe,
                )
            })
            .collect();
        let receipt = ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate_with_generated_effects(
            semantic_owner,
            root.lowering,
            root.ranked_ir,
            root.access_sources,
            root.executable_effect_sources,
        )
        .unwrap();
        assert_neutral_mutated_projection_rejected_v1(receipt);

        let ProductionRankedSemanticProgramV1 {
            semantic_ssa_owner,
            roots,
        } = neutral_scan_ranked_program_v1(SemanticWorkgroupScanKindV1::Exclusive, 255);
        let semantic_owner = semantic_ssa_owner.into_source_owner().unwrap();
        let mut root = roots.into_vec().into_iter().next().unwrap();
        root.executable_effect_sources
            .pop()
            .expect("scan projection has a final barrier effect");
        let receipt = ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate_with_generated_effects(
            semantic_owner,
            root.lowering,
            root.ranked_ir,
            root.access_sources,
            root.executable_effect_sources,
        )
        .unwrap();
        assert_neutral_mutated_projection_rejected_v1(receipt);
    }

    fn neutral_mutated_projection_receipt_v1(
        mutate: impl FnOnce(&mut Vec<ProductionRankedExecutableEffectSourceV1>),
    ) -> ProductionRankedSemanticProjectionReceiptV1 {
        let ProductionRankedSemanticProgramV1 {
            semantic_ssa_owner,
            roots,
        } = neutral_ranked_program_v1();
        let semantic_owner = semantic_ssa_owner.into_source_owner().unwrap();
        let mut root = roots
            .into_vec()
            .into_iter()
            .next()
            .expect("neutral fixture has one projected root");
        mutate(&mut root.executable_effect_sources);
        ProductionRankedSemanticProjectionReceiptV1::from_unvalidated_projection_candidate_with_generated_effects(
            semantic_owner,
            root.lowering,
            root.ranked_ir,
            root.access_sources,
            root.executable_effect_sources,
        )
        .expect("the hostile relation remains structurally inert")
    }

    fn assert_neutral_mutated_projection_rejected_v1(
        receipt: ProductionRankedSemanticProjectionReceiptV1,
    ) {
        assert!(matches!(
            fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_lower_after_ranked_checks(
                receipt,
                fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
                1,
            ),
            Err(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::MirPlironTranslation(
                fe2o3_lower_mir_kernel::ProductionMirPlironTranslationErrorV1::GeneratedEffectRecipeMismatch { .. }
            ))
        ));
    }

    #[test]
    fn neutral_full_path_rejects_nonzero_recipe_identity_substitution() {
        let receipt = neutral_mutated_projection_receipt_v1(|sources| {
            let source = sources[0];
            let mut recipe = source.recipe_identity();
            recipe[0] ^= 0x80;
            sources[0] = ProductionRankedExecutableEffectSourceV1::new(
                source.semantic_block(),
                source.semantic_effect_ordinal(),
                source.ranked_block(),
                source.ranked_operation(),
                source.origin(),
                recipe,
            );
        });
        assert_neutral_mutated_projection_rejected_v1(receipt);
    }

    #[test]
    fn neutral_full_path_rejects_missing_final_ranked_generated_effect_receipt() {
        let receipt = neutral_mutated_projection_receipt_v1(|sources| {
            sources
                .pop()
                .expect("neutral fixture has generated effects");
        });
        assert_neutral_mutated_projection_rejected_v1(receipt);
    }

    #[test]
    fn neutral_full_path_rejects_ranked_generated_effect_reordering() {
        let receipt = neutral_mutated_projection_receipt_v1(|sources| {
            let first = sources[0];
            let second = sources[1];
            sources[0] = ProductionRankedExecutableEffectSourceV1::new(
                first.semantic_block(),
                first.semantic_effect_ordinal(),
                second.ranked_block(),
                second.ranked_operation(),
                first.origin(),
                first.recipe_identity(),
            );
            sources[1] = ProductionRankedExecutableEffectSourceV1::new(
                second.semantic_block(),
                second.semantic_effect_ordinal(),
                first.ranked_block(),
                first.ranked_operation(),
                second.origin(),
                second.recipe_identity(),
            );
        });
        assert_neutral_mutated_projection_rejected_v1(receipt);
    }

    fn ranked_roster_identity_record(
        logical_name: &'static str,
        export_symbol: &'static [u8],
        binding: u8,
        semantic_root: u32,
        semantic_identity: u8,
        source_rank: u8,
        middle_end_identity: u8,
    ) -> RankedRosterIdentityRecordV1<'static> {
        RankedRosterIdentityRecordV1 {
            logical_name,
            export_symbol,
            semantic_root: SemanticFunctionIdV1::from_index(semantic_root),
            semantic_root_identity: SemanticFunctionIdentityV1::from_sha256(bytes(
                semantic_identity,
            )),
            kernel_binding: bytes(binding),
            source_rank,
            middle_end_identity_sha256: bytes(middle_end_identity),
            middle_end_identity_byte_len: 1_024 + u64::from(middle_end_identity),
            induction_semantic_mir_sha256: bytes(middle_end_identity.wrapping_add(0x40)),
            induction_function: SemanticFunctionIdV1::from_index(semantic_root + 1),
            induction_function_identity: SemanticFunctionIdentityV1::from_sha256(bytes(
                semantic_identity.wrapping_add(0x40),
            )),
            induction_checked_additions_examined: 7 + u64::from(middle_end_identity),
            induction_certificate_count: 2,
            induction_work_units: 19 + u64::from(middle_end_identity),
        }
    }

    #[test]
    fn ranked_roster_identity_is_kernel_id_ordered_and_retains_source_order_separately() {
        let source_order = [
            ranked_roster_identity_record("alpha", b"kernel_alpha", 0xa1, 4, 0x14, 1, 0x31),
            ranked_roster_identity_record("zeta", b"kernel_zeta", 0x7a, 9, 0x19, 3, 0x32),
        ];
        let (identity, canonical_order) =
            derive_ranked_kernel_roster_identity_v1(&source_order).unwrap();
        assert_eq!(canonical_order.as_ref(), &[1, 0]);
        assert_ne!(identity.as_bytes(), &[0; 32]);

        let reversed_source_order = [source_order[1], source_order[0]];
        let (reversed_identity, reversed_canonical_order) =
            derive_ranked_kernel_roster_identity_v1(&reversed_source_order).unwrap();
        assert_eq!(reversed_identity, identity);
        assert_eq!(reversed_canonical_order.as_ref(), &[0, 1]);
        assert!(matches!(
            require_exact_ranked_kernel_roster_identity_v1(
                &reversed_source_order,
                identity,
                &canonical_order,
            ),
            Err(ProductionRankedVerificationErrorV1::RosterIdentity)
        ));
    }

    #[test]
    fn ranked_roster_identity_rejects_missing_extra_and_substituted_records() {
        let exact = [
            ranked_roster_identity_record("alpha", b"kernel_alpha", 0xa1, 4, 0x14, 1, 0x31),
            ranked_roster_identity_record("zeta", b"kernel_zeta", 0x7a, 9, 0x19, 3, 0x32),
        ];
        let (identity, canonical_order) = derive_ranked_kernel_roster_identity_v1(&exact).unwrap();
        let extra = [
            exact[0],
            exact[1],
            ranked_roster_identity_record("omega", b"kernel_omega", 0xcc, 12, 0x1c, 2, 0x33),
        ];
        for hostile in [&exact[..1], &extra[..]] {
            assert!(matches!(
                require_exact_ranked_kernel_roster_identity_v1(hostile, identity, &canonical_order,),
                Err(ProductionRankedVerificationErrorV1::RosterIdentity)
            ));
        }

        let mut substitutions = Vec::new();
        let mut logical = exact;
        logical[1].logical_name = "substituted";
        substitutions.push(logical);
        let mut export = exact;
        export[1].export_symbol = b"kernel_substituted";
        substitutions.push(export);
        let mut semantic_root = exact;
        semantic_root[1].semantic_root = SemanticFunctionIdV1::from_index(8);
        substitutions.push(semantic_root);
        let mut semantic_identity = exact;
        semantic_identity[1].semantic_root_identity =
            SemanticFunctionIdentityV1::from_sha256(bytes(0xee));
        substitutions.push(semantic_identity);
        let mut binding = exact;
        binding[1].kernel_binding = bytes(0xfe);
        substitutions.push(binding);
        let mut rank = exact;
        rank[1].source_rank = 2;
        substitutions.push(rank);
        let mut middle_end = exact;
        middle_end[1].middle_end_identity_sha256 = bytes(0xef);
        substitutions.push(middle_end);
        let mut induction_semantic = exact;
        induction_semantic[1].induction_semantic_mir_sha256 = bytes(0xed);
        substitutions.push(induction_semantic);
        let mut induction_function = exact;
        induction_function[1].induction_function = SemanticFunctionIdV1::from_index(22);
        substitutions.push(induction_function);
        let mut induction_identity = exact;
        induction_identity[1].induction_function_identity =
            SemanticFunctionIdentityV1::from_sha256(bytes(0xec));
        substitutions.push(induction_identity);
        let mut induction_checked = exact;
        induction_checked[1].induction_checked_additions_examined += 1;
        substitutions.push(induction_checked);
        let mut induction_certificates = exact;
        induction_certificates[1].induction_certificate_count += 1;
        substitutions.push(induction_certificates);
        let mut induction_work = exact;
        induction_work[1].induction_work_units += 1;
        substitutions.push(induction_work);

        for hostile in substitutions {
            assert!(matches!(
                require_exact_ranked_kernel_roster_identity_v1(
                    &hostile,
                    identity,
                    &canonical_order,
                ),
                Err(ProductionRankedVerificationErrorV1::RosterIdentity)
            ));
        }
    }

    #[test]
    fn ranked_roster_identity_rejects_duplicate_or_invalid_identity_axes() {
        let exact = [
            ranked_roster_identity_record("alpha", b"kernel_alpha", 0xa1, 4, 0x14, 1, 0x31),
            ranked_roster_identity_record("zeta", b"kernel_zeta", 0x7a, 9, 0x19, 3, 0x32),
        ];
        let mut hostile_rosters = Vec::new();
        let mut logical = exact;
        logical[1].logical_name = logical[0].logical_name;
        hostile_rosters.push(logical);
        let mut export = exact;
        export[1].export_symbol = export[0].export_symbol;
        hostile_rosters.push(export);
        let mut semantic_root = exact;
        semantic_root[1].semantic_root = semantic_root[0].semantic_root;
        hostile_rosters.push(semantic_root);
        let mut semantic_identity = exact;
        semantic_identity[1].semantic_root_identity = semantic_identity[0].semantic_root_identity;
        hostile_rosters.push(semantic_identity);
        let mut binding = exact;
        binding[1].kernel_binding = binding[0].kernel_binding;
        hostile_rosters.push(binding);
        let mut invalid_rank = exact;
        invalid_rank[1].source_rank = 0;
        hostile_rosters.push(invalid_rank);

        for hostile in hostile_rosters {
            assert!(matches!(
                derive_ranked_kernel_roster_identity_v1(&hostile),
                Err(ProductionRankedVerificationErrorV1::RosterMetadata(_))
            ));
        }
        assert!(matches!(
            derive_ranked_kernel_roster_identity_v1(&[]),
            Err(ProductionRankedVerificationErrorV1::RosterMetadata(
                "an empty ranked root roster"
            ))
        ));
    }

    #[test]
    fn ranked_root_roster_requires_pairwise_semantic_order_with_per_root_rank() {
        let inputs = [
            ranked_root_input("alpha", 0xa1, 1),
            ranked_root_input("zeta", 0x7a, 3),
        ];
        let semantic_roots = [
            (bytes(0xa1), SemanticFunctionIdV1::from_index(4)),
            (bytes(0x7a), SemanticFunctionIdV1::from_index(9)),
        ];

        assert_eq!(
            match_ranked_root_bindings_v1(&inputs, &semantic_roots).unwrap(),
            vec![
                SemanticFunctionIdV1::from_index(4),
                SemanticFunctionIdV1::from_index(9),
            ],
        );
        assert_eq!(inputs[0].source_launch.rank(), 1);
        assert_eq!(inputs[1].source_launch.rank(), 3);

        let reordered = [semantic_roots[1], semantic_roots[0]];
        assert!(matches!(
            match_ranked_root_bindings_v1(&inputs, &reordered),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a reordered or substituted typed/semantic kernel binding in the ranked roster"
            ))
        ));
    }

    #[test]
    fn ranked_root_roster_rejects_count_duplicate_and_substituted_hostility() {
        let exact = [
            ranked_root_input("alpha", 0xa1, 1),
            ranked_root_input("zeta", 0x7a, 2),
        ];
        let semantic_roots = [
            (bytes(0xa1), SemanticFunctionIdV1::from_index(0)),
            (bytes(0x7a), SemanticFunctionIdV1::from_index(1)),
        ];
        assert!(matches!(
            match_ranked_root_bindings_v1(&exact, &semantic_roots[..1]),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an incomplete typed/semantic ranked root roster"
            ))
        ));
        let extra_semantic_root = [
            semantic_roots[0],
            semantic_roots[1],
            (bytes(0xcc), SemanticFunctionIdV1::from_index(2)),
        ];
        assert!(matches!(
            match_ranked_root_bindings_v1(&exact, &extra_semantic_root),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an incomplete typed/semantic ranked root roster"
            ))
        ));

        let duplicate_logical = [
            ranked_root_input("alpha", 0xa1, 1),
            ranked_root_input("alpha", 0x7a, 2),
        ];
        assert!(matches!(
            match_ranked_root_bindings_v1(&duplicate_logical, &semantic_roots),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "duplicate typed logical roots in the ranked roster"
            ))
        ));

        let duplicate_semantic = [
            (bytes(0xa1), SemanticFunctionIdV1::from_index(0)),
            (bytes(0xa1), SemanticFunctionIdV1::from_index(1)),
        ];
        assert!(matches!(
            match_ranked_root_bindings_v1(&exact, &duplicate_semantic),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "duplicate semantic kernel bindings in the ranked roster"
            ))
        ));

        let substituted = [
            ranked_root_input("alpha", 0xa1, 1),
            ranked_root_input("zeta", 0xfe, 2),
        ];
        assert!(matches!(
            match_ranked_root_bindings_v1(&substituted, &semantic_roots),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "a reordered or substituted typed/semantic kernel binding in the ranked roster"
            ))
        ));
        let duplicate_typed_binding = [
            ranked_root_input("alpha", 0xa1, 1),
            ranked_root_input("zeta", 0xa1, 2),
        ];
        assert!(matches!(
            match_ranked_root_bindings_v1(&duplicate_typed_binding, &semantic_roots),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "duplicate typed kernel bindings in the ranked roster"
            ))
        ));
    }

    #[test]
    fn reference_effect_partition_assigns_nonempty_two_root_bindings_in_root_order() {
        assert_eq!(
            partition_reference_effect_binding_indices_v1(&["alpha", "zeta"], &["zeta", "alpha"],)
                .unwrap(),
            vec![vec![1], vec![0]],
        );
        assert_eq!(
            partition_reference_effect_binding_indices_v1(&["alpha", "zeta"], &[]).unwrap(),
            vec![Vec::<usize>::new(), Vec::<usize>::new()],
        );
    }
