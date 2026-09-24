use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use wire_tests::NoProof;

type O = ProductionRankedOperationV1;
type T = ProductionRankedTerminatorV1;
type V = ProductionRankedValueV1;
type X = ProductionSemanticExpressionV2;
const I: ProductionRankedValueIdV1 = ProductionRankedValueIdV1::new(0);
const A: V = V::Argument(3);
const B: V = V::BlockArgument {
    block: 2,
    argument: 1,
};
const C: V = V::Local(I);
const S: ProductionSemanticScalarTypeV2 = ProductionSemanticScalarTypeV2::Integer {
    signed: false,
    bits: 32,
};

fn digest(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

fn subjects() -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(4),
    )
    .unwrap()
}

fn bytes<T: Wire>(value: &T) -> Vec<u8> {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut count = Encoder::new(Mode::Count, &mut budget);
    value.emit(&mut count).unwrap();
    let mut bytes = vec![0; count.offset];
    value
        .emit(&mut Encoder::new(Mode::Fill(&mut bytes), &mut budget))
        .unwrap();
    bytes
}

fn round_trip<T: Wire + Eq + fmt::Debug, R: Resolver>(value: &T, resolver: &mut R)
where
    R::Error: fmt::Debug,
{
    let bytes = bytes(value);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut input = Decoder::new(&bytes, resolver, &mut budget);
    let decoded = T::read(&mut input).unwrap();
    assert_eq!(input.offset, bytes.len());
    assert_eq!(&decoded, value);
    let mut canonical = Encoder::new(Mode::Compare(&bytes), &mut budget);
    decoded.emit(&mut canonical).unwrap();
    assert!(canonical.matches);
    assert_eq!(canonical.offset, bytes.len());
}

fn effect() -> ProductionEffectRefinementContractV2 {
    ProductionEffectRefinementContractV2::new(
        7,
        ProductionGpuWriteSiteV2::new(2, 3),
        ProductionReferenceOutputSiteV2::new(4, 5, 6),
        A,
        vec![B],
        vec![C],
        vec![A],
        B,
        C,
        A,
        B,
        C,
        A,
    )
    .unwrap()
}

fn numerical() -> ProductionNumericalRefinementContractV2 {
    ProductionNumericalRefinementContractV2::new(
        8,
        A,
        B,
        C,
        A,
        0.001f64.to_bits(),
        0.01f64.to_bits(),
    )
    .unwrap()
}

fn tensor() -> ProductionTensorRefinementContractV1 {
    ProductionTensorRefinementContractV1::new(
        9,
        ProductionTensorInstructionSiteV1::new(2, 4),
        digest(5),
        A,
        B,
        C,
        S,
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        vec![
            ProductionTensorResultComponentV1::new(
                0,
                ProductionGpuWriteSiteV2::new(2, 5),
                vec![B],
                C,
                A,
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn operations() -> Vec<O> {
    vec![
        O::ExecutionLayout {
            grid_identity: 3,
            global_extents: [128, 2, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        },
        O::View {
            result: I,
            element_width: 32,
            writable: true,
            shape: vec![3, 5],
            dynamic_extents: vec![A],
            allocation_origin: 7,
            noalias_class: 8,
        },
        O::ViewInSpace {
            result: I,
            element_width: 16,
            writable: false,
            shape: vec![4],
            dynamic_extents: vec![],
            memory_space: MemorySpaceAttr::Workgroup,
            allocation_origin: 9,
            noalias_class: 10,
        },
        O::PipelineCreate {
            result: I,
            view: A,
            buffers: 3,
            prefetch_distance: 2,
        },
        O::PipelineEvent {
            pipeline: A,
            epoch: B,
            slot: C,
            kind: PipelineEventKindAttr::Consume,
        },
        O::IndexConstant {
            result: I,
            value: 0x0123456789abcdef,
        },
        O::IndexUnsignedCast {
            result: I,
            source: A,
            bit_width: 16,
        },
        O::IndexUnknown { result: I },
        O::InvocationIndex {
            result: I,
            dimension: 2,
            launch_extent: 192,
        },
        O::IndexBinary {
            result: I,
            kind: IndexBinaryKindAttr::Remainder,
            lhs: A,
            rhs: B,
        },
        O::DeterministicJoin {
            result: I,
            dependencies: vec![A, B, C],
        },
        O::CheckedTiledIndex2D {
            result: I,
            invocation: A,
            component: B,
            rows: C,
            columns: A,
            row_stride: B,
            lanes_per_tile: 64,
            tile_rows: 16,
            tile_columns: 8,
            elements_per_lane: 2,
        },
        O::CheckedRowStripedIndex2D {
            result: I,
            invocation: A,
            component: B,
            rows: C,
            columns: A,
            row_stride: B,
            lanes_per_row: 32,
            elements_per_lane: 4,
        },
        O::PredicatedCheckedTiledIndex2D {
            result: I,
            success: ProductionRankedValueIdV1::new(1),
            invocation: A,
            component: B,
            rows: C,
            columns: A,
            row_stride: B,
            physical_extent: C,
            lanes_per_tile: 64,
            tile_rows: 16,
            tile_columns: 8,
            elements_per_lane: 2,
        },
        O::PredicatedCheckedRowStripedIndex2D {
            result: I,
            success: ProductionRankedValueIdV1::new(1),
            invocation: A,
            component: B,
            rows: C,
            columns: A,
            row_stride: B,
            physical_extent: C,
            lanes_per_row: 32,
            elements_per_lane: 4,
        },
        O::Dimension {
            result: I,
            view: A,
            dimension: 1,
        },
        O::Access {
            kind: AccessKindAttr::Read,
            view: A,
            indices: vec![B, C],
        },
        O::PredicatedAccess {
            kind: AccessKindAttr::Write,
            view: A,
            index: B,
            success: C,
        },
        O::ValueAccess {
            kind: AccessKindAttr::Write,
            view: A,
            indices: vec![B],
            value: C,
        },
        O::AtomicAccess {
            kind: AccessKindAttr::AtomicRead,
            ordering: AtomicOrderingAttr::Acquire,
            scope: AtomicScopeAttr::Agent,
            view: A,
            indices: vec![B],
        },
        O::AtomicValueAccess {
            kind: AccessKindAttr::AtomicReadModifyWrite,
            ordering: AtomicOrderingAttr::AcquireRelease,
            scope: AtomicScopeAttr::System,
            view: A,
            indices: vec![B],
            value: C,
        },
        O::OwnershipContract {
            view: A,
            coverage: OwnershipCoverageAttr::ExactEffectDomain,
            partition: OwnershipPartitionAttr::DenseRectangles,
        },
        O::AllocationEffect {
            kind: AccessKindAttr::Write,
            memory_space: MemorySpaceAttr::Private,
            allocation_origin: 8,
            noalias_class: 9,
        },
        O::Barrier {
            execution_scope: HierarchyAttr::Workgroup,
            memory_scope: MemoryScopeAttr::Workgroup,
            address_space: AddressSpaceAttr::Workgroup,
            order: MemoryOrderAttr::AcquireRelease,
        },
        O::Fence {
            memory_scope: MemoryScopeAttr::Device,
            address_space: AddressSpaceAttr::Global,
            order: MemoryOrderAttr::SequentiallyConsistent,
        },
        O::TensorLayout {
            contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
            convergence: TensorConvergenceAttr::UniformSubgroup,
            active_lanes: 64,
            binding: Some(
                ProductionCooperativeTensorBindingV1::new(
                    digest(1),
                    digest(2),
                    digest(3),
                    digest(4),
                    digest(5),
                    digest(6),
                    256,
                )
                .unwrap(),
            ),
        },
        O::TensorResultComponent {
            result: I,
            tensor_result_root: digest(5),
            component: 2,
            scalar: S,
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        },
        O::SemanticSymbol {
            result: I,
            symbol: 13,
        },
        O::SemanticConstant {
            result: I,
            value: 17,
        },
        O::SemanticBinary {
            result: I,
            kind: SemanticBinaryKindAttr::Multiply,
            lhs: A,
            rhs: B,
        },
        O::SemanticExpression {
            result: I,
            expression: X::Constant {
                scalar: S,
                bits: 31,
            },
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        },
        O::CollectiveSemantics {
            contract: ProductionCollectiveSemanticContractV1::new(
                ProductionCollectiveSemanticKindV1::FiniteFold,
                [1; 4],
                [2; 4],
                [2; 4],
                16,
                8,
                SemanticEvaluationOrderAttr::Ascending,
                ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                SemanticCoverageBindingAttr::CollectiveContributions,
            )
            .unwrap(),
            view: A,
            actual: B,
            expected: C,
            witness0: A,
            witness1: B,
        },
        O::RequireEquivalent {
            actual: A,
            expected: B,
        },
        O::RequestAuthenticatedReferenceEquivalent {
            actual: A,
            expected: B,
            subjects: subjects(),
        },
        O::RequestEffectRefinement {
            contract: effect(),
            subjects: subjects(),
        },
        O::RequestNumericalRefinement {
            contract: numerical(),
            subjects: subjects(),
        },
        O::RequestTensorRefinement {
            contract: tensor(),
            subjects: subjects(),
        },
    ]
}

#[test]
fn every_unbound_operation_has_lossless_wire_fields() {
    let values = operations();
    assert_eq!(values.len(), 37);
    for (value, tag) in values.iter().zip((1..=33).chain([35, 37, 39, 41])) {
        assert_eq!(bytes(value)[0], tag);
        round_trip(value, &mut NoProof::default());
    }
}

#[test]
fn every_terminator_has_lossless_ordered_successors() {
    let values = [
        T::IndexLessThan {
            lhs: A,
            rhs: B,
            true_block: 2,
            false_block: 3,
        },
        T::IndexLessThanArgs {
            lhs: A,
            rhs: B,
            true_arguments: vec![A, B],
            false_arguments: vec![C],
            true_block: 2,
            false_block: 3,
        },
        T::IndexEqual {
            lhs: A,
            rhs: B,
            true_block: 2,
            false_block: 3,
        },
        T::IndexEqualArgs {
            lhs: A,
            rhs: B,
            true_arguments: vec![A, B],
            false_arguments: vec![C],
            true_block: 2,
            false_block: 3,
        },
        T::AnalysisSplit {
            control_dependencies: vec![A, B, C],
            first_block: 2,
            second_block: 3,
        },
        T::AnalysisSplitArgs {
            control_dependencies: vec![C],
            first_arguments: vec![A, B],
            second_arguments: vec![C],
            first_block: 2,
            second_block: 3,
        },
        T::Branch { target: 2 },
        T::BranchArgs {
            arguments: vec![A, B, C],
            target: 3,
        },
        T::BranchArgsAdd {
            value: A,
            step: B,
            target: 2,
        },
        T::BranchArgsAddAt {
            arguments: vec![A, B],
            add_argument: 1,
            step: C,
            target: 3,
        },
        T::Return,
        T::Trap,
    ];
    for (index, value) in values.iter().enumerate() {
        assert_eq!(bytes(value)[0], index as u8 + 1);
        round_trip(value, &mut NoProof::default());
    }
}

#[test]
fn scratch_tensor_storage_is_required_in_every_encoder_mode() {
    let tensor = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
    let bytes = bytes(&tensor);
    for quota in [116, 117] {
        let mut buffer = vec![0; bytes.len()];
        for mode in [Mode::Count, Mode::Fill(&mut buffer), Mode::Compare(&bytes)] {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, quota + 13);
            budget.reserve_storage(13).unwrap();
            let mut out = Encoder::new(mode, &mut budget);
            assert_eq!(tensor.emit(&mut out).is_ok(), quota == 117);
            assert_eq!(out.heap, 0);
            assert_eq!(budget.storage(), 13);
        }
    }
    for quota in [1, 105, 107] {
        let mut buffer = vec![0; bytes.len()];
        for mode in [Mode::Count, Mode::Fill(&mut buffer), Mode::Compare(&bytes)] {
            let mut work = Work::new(quota);
            let mut budget = Budget::new(&mut work, 130);
            budget.reserve_storage(13).unwrap();
            assert!(tensor.emit(&mut Encoder::new(mode, &mut budget)).is_err());
            assert_eq!(budget.storage(), 13);
            assert_eq!(budget.peak_storage(), 130);
        }
    }
}

#[test]
fn normalized_owner_reservation_is_never_released_during_comparison() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 42);
    budget.reserve_storage(13 + 19).unwrap();
    let mut out = Encoder::new(Mode::Compare(&[]), &mut budget);
    out.coverage = Some(19);
    out.retain(12).unwrap();
    assert_eq!(out.budget.storage(), 32);
    out.retain(17).unwrap();
    assert_eq!(out.budget.storage(), 42);
    assert!(out.retain(1).is_err());
    assert_eq!(out.budget.storage(), 42);
}

#[test]
fn oversized_successor_and_entry_argument_counts_reject_before_payload() {
    let mut branch = vec![8];
    branch.extend_from_slice(&65u32.to_le_bytes());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 0);
    let mut resolver = NoProof::default();
    assert!(matches!(
        T::read(&mut Decoder::new(&branch, &mut resolver, &mut budget)),
        Err(DecodeError::Wire(WireError::Invalid("sequence count")))
    ));
    let mut recipe = wire_tests::encode(&wire_tests::kernel());
    recipe[25..29].copy_from_slice(&1u32.to_le_bytes());
    let mut budget = Budget::new(&mut work, usize::MAX);
    assert!(matches!(
        decode_production_ranked_recipe_v1(&recipe, &mut resolver, &mut budget),
        Err(DecodeError::Wire(WireError::Invalid("sequence count")))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn materialization_census_uses_fresh_expression_nodes_and_expansion_weights() {
    let expression = O::SemanticExpression {
        result: I,
        expression: X::Constant { scalar: S, bits: 1 },
        numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
    };
    let mut census = RecipeCensus::default();
    census.operation(&expression, 5, 1).unwrap();
    census
        .operation(
            &O::IndexConstant {
                result: I,
                value: 0,
            },
            99,
            1,
        )
        .unwrap();
    census.operation(&expression, 1, 1).unwrap();
    census
        .operation(
            &O::RequestEffectRefinement {
                contract: effect(),
                subjects: subjects(),
            },
            77,
            1,
        )
        .unwrap();
    assert_eq!(census.materialized, 6 + 1 + 2 + 3);
    let last = (HARD_MAX_SESSION_OPERATION_TREE_ITEMS - 7) / 2;
    census.add(last - census.materialized, 1).unwrap();
    assert!(matches!(
        census.add(1, 1),
        Err(WireError::Invalid("materialized operation limit"))
    ));
}

fn expression(nodes: usize) -> X {
    match nodes {
        1 => X::Symbol {
            symbol: 7,
            scalar: S,
        },
        n if n % 2 == 0 => X::Unary {
            operation: ProductionSemanticUnaryOpV2::Not,
            scalar: S,
            operand: Box::new(expression(n - 1)),
        },
        n => X::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar: S,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(expression(n / 2)),
            rhs: Box::new(expression(n / 2)),
        },
    }
}

#[test]
fn expression_variants_and_boundary_counts_are_lossless() {
    use crate::production::{
        MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 as DEPTH,
        MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 as NODES, ProductionSemanticLoadV2,
    };
    let values = [
        expression(1),
        X::Constant {
            scalar: S,
            bits: 17,
        },
        X::Load(ProductionSemanticLoadV2 {
            block: 2,
            operation: 3,
            scalar: S,
            allocation_origin: 4,
            view: A,
            indices: vec![B, C].into_boxed_slice(),
        }),
        expression(2),
        expression(3),
        X::Compare {
            operation: ProductionSemanticComparisonV2::GreaterOrEqual,
            operand_scalar: S,
            lhs: Box::new(expression(1)),
            rhs: Box::new(expression(1)),
        },
        X::Select {
            scalar: S,
            condition: Box::new(X::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1,
            }),
            when_true: Box::new(expression(1)),
            when_false: Box::new(expression(1)),
        },
        X::Cast {
            kind: ProductionSemanticCastV2::Integer,
            source: S,
            target: ProductionSemanticScalarTypeV2::Integer {
                signed: true,
                bits: 64,
            },
            operand: Box::new(expression(1)),
        },
    ];
    for (index, value) in values.iter().enumerate() {
        assert_eq!(bytes(value)[0], index as u8 + 1);
        round_trip(value, &mut NoProof::default());
    }
    let mut deep = expression(1);
    for _ in 1..DEPTH {
        deep = X::Unary {
            operation: ProductionSemanticUnaryOpV2::Not,
            scalar: S,
            operand: Box::new(deep),
        };
    }
    for value in [deep, expression(NODES)] {
        round_trip(&value, &mut NoProof::default());
        let encoded = bytes(&value);
        let mut oversized = vec![4, 1];
        oversized.extend(bytes(&S));
        oversized.extend(encoded);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        assert!(matches!(
            X::read(&mut Decoder::new(
                &oversized,
                &mut NoProof::default(),
                &mut budget
            )),
            Err(DecodeError::Wire(WireError::Invalid("expression limit")))
        ));
    }
}

#[test]
fn decode_expression_census_resets_between_operations() {
    let values = [
        O::SemanticExpression {
            result: I,
            expression: expression(5),
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        },
        O::IndexConstant {
            result: I,
            value: 0,
        },
        O::SemanticExpression {
            result: I,
            expression: expression(1),
            numerical_contract: ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        },
    ];
    let encoded: Vec<_> = values.iter().flat_map(bytes).collect();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut resolver = NoProof::default();
    let mut input = Decoder::new(&encoded, &mut resolver, &mut budget);
    let mut census = RecipeCensus::default();
    for _ in values {
        let op = O::read(&mut input).unwrap();
        census.operation(&op, input.nodes, 1).unwrap();
    }
    assert_eq!(census.materialized, 9);
    assert_eq!(input.offset, encoded.len());
}

fn leaf_tags<T: Wire + Eq + fmt::Debug>(last: u8) {
    for tag in 0..=last + 1 {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 0);
        let encoded = [tag];
        let mut resolver = NoProof::default();
        let result = T::read(&mut Decoder::new(&encoded, &mut resolver, &mut budget));
        if (1..=last).contains(&tag) {
            let value = result.unwrap();
            assert_eq!(bytes(&value), encoded);
            round_trip(&value, &mut resolver);
        } else {
            assert!(matches!(
                result,
                Err(DecodeError::Wire(WireError::UnknownTag { .. }))
            ));
        }
    }
}

#[test]
fn every_leaf_enum_tag_is_explicit_and_unknown_tags_are_rejected() {
    leaf_tags::<AccessKindAttr>(5);
    leaf_tags::<MemorySpaceAttr>(3);
    leaf_tags::<AtomicOrderingAttr>(5);
    leaf_tags::<AtomicScopeAttr>(5);
    leaf_tags::<OwnershipCoverageAttr>(4);
    leaf_tags::<OwnershipPartitionAttr>(2);
    leaf_tags::<HierarchyAttr>(4);
    leaf_tags::<MemoryScopeAttr>(4);
    leaf_tags::<AddressSpaceAttr>(5);
    leaf_tags::<MemoryOrderAttr>(4);
    leaf_tags::<IndexBinaryKindAttr>(4);
    leaf_tags::<SemanticBinaryKindAttr>(2);
    leaf_tags::<PipelineEventKindAttr>(6);
    leaf_tags::<TensorConvergenceAttr>(4);
    leaf_tags::<SemanticEvaluationOrderAttr>(4);
    leaf_tags::<SemanticCoverageBindingAttr>(2);
    leaf_tags::<ProductionCollectiveSemanticKindV1>(3);
    leaf_tags::<SafeReferenceKindV2>(2);
    leaf_tags::<ProductionIeeeRoundingModeV2>(4);
    leaf_tags::<ProductionIeeeExceptionalValuePolicyV2>(2);
    leaf_tags::<ProductionOverflowContractV2>(2);
    leaf_tags::<ProductionSemanticUnaryOpV2>(2);
    leaf_tags::<ProductionSemanticBinaryOpV2>(10);
    leaf_tags::<ProductionSemanticComparisonV2>(6);
    leaf_tags::<ProductionSemanticCastV2>(4);
    for policy in [
        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
        ProductionNumericalContractV2::ExactIeee754OperatorCongruence {
            rounding: ProductionIeeeRoundingModeV2::TowardNegative,
            exceptional_values: ProductionIeeeExceptionalValuePolicyV2::CanonicalNan,
        },
        ProductionNumericalContractV2::Relaxed,
        ProductionNumericalContractV2::ErrorBounded {
            absolute_error_f64_bits: 0.001f64.to_bits(),
            relative_error_f64_bits: 0.01f64.to_bits(),
        },
    ] {
        round_trip(&policy, &mut NoProof::default());
    }
}

#[test]
fn deepest_symbol_expression_has_independent_exact_work_and_storage_limits() {
    let mut encoded = Vec::new();
    for _ in 0..127 {
        encoded.extend([4, 1]);
        encoded.extend(bytes(&S));
    }
    encoded.extend(bytes(&expression(1)));
    let exact_work = 14 + 127 * 11;
    let exact_storage = 127 * size_of::<X>();
    for (work_limit, storage_limit, success) in [
        (exact_work, exact_storage, true),
        (exact_work - 1, exact_storage, false),
        (exact_work, exact_storage - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let result = X::read(&mut Decoder::new(
            &encoded,
            &mut NoProof::default(),
            &mut budget,
        ));
        assert_eq!(result.is_ok(), success);
        if success {
            assert_eq!(budget.work(), exact_work);
            assert_eq!(budget.storage(), exact_storage);
        }
    }
}

#[cfg(feature = "internal-proof-staging")]
#[test]
fn every_bound_operation_requires_exact_resolved_identity_and_binding() {
    use ed25519_dalek::{Signer, SigningKey};
    use fe2o3_functional_proof::*;
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects(), digest(9)).unwrap();
    let signing = SigningKey::from_bytes(&[91; 32]);
    let toolchain =
        VerusToolchainIdentityV2::new(digest(10), digest(11), digest(12), digest(13), digest(14))
            .unwrap();
    let boundary = FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir;
    let policy = FunctionalRefinementImportPolicyV2::new(
        signing.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
    // Synthetic signing exercises import and transport, not protected proof execution.
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        digest(20),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let imported = FunctionalRefinementReceiptImporterV2::new(policy, 1)
        .unwrap()
        .import(
            FunctionalRefinementImportExpectationV2::new(binding),
            &unsigned.attach_signature(signature),
        )
        .unwrap();
    let proof = ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding);
    struct Fixed {
        proof: ProductionReferenceProofV2,
        claims: Vec<ProductionRankedRecipeProofClaimV1>,
        swallow_denial: bool,
    }
    impl Resolver for Fixed {
        type Error = &'static str;
        fn resolve(
            &mut self,
            claim: ProductionRankedRecipeProofClaimV1,
            work: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
        ) -> Result<ProductionReferenceProofV2, Self::Error> {
            work.charge_work(1).map_err(|_| "work")?;
            if self.swallow_denial {
                let _ = work.charge_work(usize::MAX);
            }
            self.claims.push(claim);
            Ok(self.proof)
        }
        fn finish(
            &mut self,
            _: u32,
            _: &mut ProductionRankedRecipeResolverWorkV1<'_, '_>,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }
    let operations = [
        O::RequireAuthenticatedReferenceEquivalent {
            actual: A,
            expected: B,
            proof,
        },
        O::RequireEffectRefinement {
            contract: effect(),
            proof,
        },
        O::RequireNumericalRefinement {
            contract: numerical(),
            proof,
        },
        O::RequireTensorRefinement {
            contract: tensor(),
            proof,
        },
    ];
    for (operation, tag) in operations.iter().zip([34, 36, 38, 40]) {
        assert_eq!(bytes(operation)[0], tag);
        let mut resolver = Fixed {
            proof,
            claims: vec![],
            swallow_denial: false,
        };
        round_trip(operation, &mut resolver);
        assert_eq!(
            resolver.claims,
            [ProductionRankedRecipeProofClaimV1 {
                ordinal: 0,
                block: 0,
                operation: 0,
                receipt_digest: proof.receipt_identity().digest(),
                binding
            }]
        );
    }
    for offset in [0, 32 + 1] {
        let mut encoded = bytes(&proof);
        encoded[offset] ^= 1;
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let mut resolver = Fixed {
            proof,
            claims: vec![],
            swallow_denial: false,
        };
        assert!(matches!(
            ProductionReferenceProofV2::read(&mut Decoder::new(
                &encoded,
                &mut resolver,
                &mut budget
            )),
            Err(DecodeError::Wire(WireError::Invalid(
                "resolved proof claim"
            )))
        ));
    }
    let encoded = bytes(&proof);
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 13);
    budget.reserve_storage(13).unwrap();
    budget.charge_work(1).unwrap();
    assert!(budget.charge_work(usize::MAX).is_err());
    let mut resolver = Fixed {
        proof,
        claims: vec![],
        swallow_denial: true,
    };
    assert!(matches!(
        ProductionReferenceProofV2::read(&mut Decoder::new(&encoded, &mut resolver, &mut budget)),
        Err(DecodeError::Wire(WireError::Resource(Resource::Work(_))))
    ));
    assert_eq!(budget.storage(), 13);
}
