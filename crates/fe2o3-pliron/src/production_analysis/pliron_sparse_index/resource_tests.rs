#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;
    use dialect_kernel::{DIALECT_NAME, register_dialect};
    use pliron::dialect::DialectName;

    #[test]
    fn borrowed_sparse_facts_preserve_every_owned_variant_without_cloning_payloads() {
        let context = &mut Context::new();
        register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let operation = IndexConstantOp::new(context, 0);
        let key = operation.result(context);
        let missing = IndexConstantOp::new(context, 1).result(context);
        let affine = SparseAffineIndexV1::constant(7);
        let variants = [
            SparseIndexFactV1::Unknown,
            SparseIndexFactV1::Affine(affine.clone()),
            SparseIndexFactV1::Remainder {
                dividend: affine.clone(),
                modulus: 3,
            },
            SparseIndexFactV1::MachineOverflow(SparseMachineOverflowV1 {
                operation: IndexBinaryKindAttr::Add,
                invocation: vec![3, 4, 5],
                lhs: u64::MAX,
                rhs: 1,
            }),
            SparseIndexFactV1::CheckedTiled2D(SparseCheckedTiledIndex2DV1 {
                invocation: affine.clone(),
                component: key,
                rows: key,
                columns: key,
                row_stride: key,
                geometry: [1, 2, 3, 4],
            }),
            SparseIndexFactV1::CheckedRowStriped2D(SparseCheckedRowStripedIndex2DV1 {
                invocation: affine,
                component: key,
                rows: key,
                columns: key,
                row_stride: key,
                geometry: [1, 2],
            }),
        ];
        for fact in variants {
            let analysis = SparseIndexAnalysisV1 {
                owner: operation.get_operation(),
                facts: HashMap::from([(
                    key,
                    SparseValueFactsV1 {
                        numeric: fact,
                        stable: None,
                    },
                )]),
                launch_extents: vec![],
                declared_launch_extents: vec![],
            };
            assert_eq!(analysis.fact_ref(key), &analysis.fact(key));
            assert!(std::ptr::eq(
                analysis.fact_ref(key),
                &analysis.facts.get(&key).unwrap().numeric
            ));
            assert_eq!(analysis.fact_ref(missing), &SparseIndexFactV1::Unknown);
            assert_eq!(analysis.fact_ref(missing), &analysis.fact(missing));
        }
    }

    fn census() -> ProductionAnalysisInputCensusV1 {
        ProductionAnalysisInputCensusV1 {
            blocks: 4,
            operations: 17,
            operands: 31,
            results: 15,
            successors: 6,
            block_arguments: 5,
            ..ProductionAnalysisInputCensusV1::default()
        }
    }

    #[test]
    fn sparse_index_bound_accepts_exact_limits_and_rejects_one_under() {
        let census = census();
        let merges = SparseIndexMergeCensusV1 {
            inputs: 5,
            squared_inputs: 5,
            argument_type_work: 9,
        };
        let exact = sparse_index_resource_upper_bound_v1(
            census,
            merges,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        // Structural624 + census5 + propagation2996 + argument types9.
        // Dense exact types add 2*20+3 peak cells, including the vector header.
        assert_eq!(exact.work_upper_bound(), 3_634);
        assert_eq!(exact.peak_storage_upper_bound(), 1_984);
        assert_eq!(
            sparse_index_resource_upper_bound_v1(
                census,
                merges,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Ok(exact)
        );
        assert!(
            sparse_index_resource_upper_bound_v1(
                census,
                merges,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            )
            .is_err()
        );
        assert!(
            sparse_index_resource_upper_bound_v1(
                census,
                merges,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn sparse_index_bound_rejects_overflow_and_internal_caps() {
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        assert!(
            sparse_index_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    results: usize::MAX,
                    block_arguments: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                SparseIndexMergeCensusV1::default(),
                unlimited,
            )
            .is_err()
        );
        assert!(
            sparse_index_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    results: MAX_SPARSE_INDEX_VALUES_V1 + 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                SparseIndexMergeCensusV1::default(),
                unlimited,
            )
            .is_err()
        );
    }

    #[test]
    fn staggered_updates_into_a_wide_merge_have_a_quadratic_exact_boundary() {
        const DEGREE: usize = 32;
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let inputs = (0..DEGREE)
            .map(|value| IndexConstantOp::new(&mut context, value as u64).result(&context))
            .collect::<Vec<_>>();
        let definition_indices = inputs
            .iter()
            .copied()
            .enumerate()
            .map(|(index, value)| (value, index))
            .collect::<HashMap<_, _>>();
        let mut lattice = vec![SparseIndexLatticeV1::Pending; DEGREE];
        let known = SparseIndexLatticeV1::Known(SparseIndexFactV1::Affine(
            SparseAffineIndexV1::constant(7),
        ));
        let mut work = 0;
        for index in 0..DEGREE {
            lattice[index] = known.clone();
            let _ = merge_facts(&inputs, &lattice, &definition_indices, &mut work).unwrap();
        }
        assert_eq!(work, DEGREE * DEGREE);

        let census = ProductionAnalysisInputCensusV1 {
            blocks: 17,
            operations: 16,
            operands: DEGREE,
            results: 16,
            successors: 16,
            block_arguments: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // D=81,V=17,structural=98. Propagation is
        // 17+16+62*17+20*81+2*32+8*1024+16=10979; scan=18.
        // Root payloads and owner/header add 16*17+11 to the old peak1549.
        let merges = SparseIndexMergeCensusV1 {
            inputs: DEGREE,
            squared_inputs: DEGREE * DEGREE,
            argument_type_work: 1,
        };
        const EXACT_WORK: usize = 11_782;
        const EXACT_PEAK: usize = 1_869;
        let exact = sparse_index_resource_upper_bound_v1(
            census,
            merges,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert!(
            sparse_index_resource_upper_bound_v1(
                census,
                merges,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK),
            )
            .is_err()
        );
        assert!(
            sparse_index_resource_upper_bound_v1(
                census,
                merges,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            )
            .is_err()
        );
    }
}
