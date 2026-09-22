use super::*;
use crate::ProductionSemanticLoadV2;
use fe2o3_functional_proof::SafeReferenceKindV2;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

pub(super) fn digest(value: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([value; 32])
}
pub(super) fn subjects() -> FunctionalRefinementSubjectsV2 {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::SourceAndMir,
        digest(1),
        digest(2),
        digest(3),
        digest(4),
        digest(5),
    )
    .unwrap()
}
pub(super) fn proof(identity: u8) -> ProductionReferenceProofV2 {
    ProductionReferenceProofV2::request_exact(
        FunctionalRefinementReceiptIdentityV2::from_untrusted_digest(digest(identity)),
        FunctionalRefinementBindingV2::from_subjects(subjects(), digest(6)).unwrap(),
    )
}
fn scalar() -> Scalar {
    Scalar::Integer {
        signed: false,
        bits: 64,
    }
}
fn numerical() -> Numerical {
    Numerical::ExactBitVectorOperatorCongruence
}
fn value() -> Value {
    Value::Argument(0x1020_3040)
}
fn effect() -> ProductionEffectRefinementContractV2 {
    ProductionEffectRefinementContractV2::new(
        9,
        ProductionGpuWriteSiteV2::new(10, 11),
        ProductionReferenceOutputSiteV2::new(12, 13, 14),
        value(),
        vec![value()],
        vec![value()],
        vec![value()],
        value(),
        value(),
        value(),
        value(),
        value(),
        value(),
    )
    .unwrap()
}
fn numerical_refinement() -> ProductionNumericalRefinementContractV2 {
    ProductionNumericalRefinementContractV2::new(
        17,
        value(),
        value(),
        value(),
        value(),
        0.25f64.to_bits(),
        0.5f64.to_bits(),
    )
    .unwrap()
}
fn tensor_refinement() -> ProductionTensorRefinementContractV1 {
    ProductionTensorRefinementContractV1::new(
        19,
        ProductionTensorInstructionSiteV1::new(20, 21),
        digest(22),
        value(),
        value(),
        value(),
        scalar(),
        numerical(),
        vec![
            ProductionTensorResultComponentV1::new(
                0,
                ProductionGpuWriteSiteV2::new(23, 24),
                vec![value()],
                value(),
                value(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
fn collective() -> ProductionCollectiveSemanticContractV1 {
    ProductionCollectiveSemanticContractV1::new(
        ProductionCollectiveSemanticKindV1::FiniteFold,
        [1; 4],
        [2; 4],
        [2; 4],
        8,
        7,
        SemanticEvaluationOrderAttr::Ascending,
        numerical(),
        SemanticCoverageBindingAttr::TotalView,
    )
    .unwrap()
}
fn cooperative() -> ProductionCooperativeTensorBindingV1 {
    ProductionCooperativeTensorBindingV1::new(
        digest(1),
        digest(2),
        digest(3),
        digest(4),
        digest(5),
        digest(6),
        7,
    )
    .unwrap()
}
fn expression() -> Expr {
    Expr::Constant {
        scalar: scalar(),
        bits: 0x8877_6655_4433_2211,
    }
}

fn operation_cases() -> Vec<(u16, Op)> {
    vec![
        (
            1,
            Op::ExecutionLayout {
                grid_identity: 0x1122_3344_5566_7788,
                global_extents: [3, 5, 7],
                workgroup_extents: [3, 5, 7],
                subgroup_size: 0x1122_3344_5566_7788,
                full_physical_workgroups: true,
            },
        ),
        (
            2,
            Op::View {
                result: Id::new(0x7654_3210),
                element_width: 0x1020_3040,
                writable: true,
                shape: vec![3, 5],
                dynamic_extents: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                allocation_origin: 0x1122_3344_5566_7788,
                noalias_class: 0x1122_3344_5566_7788,
            },
        ),
        (
            3,
            Op::ViewInSpace {
                result: Id::new(0x7654_3210),
                element_width: 0x1020_3040,
                writable: true,
                shape: vec![3, 5],
                dynamic_extents: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                memory_space: MemorySpaceAttr::Workgroup,
                allocation_origin: 0x1122_3344_5566_7788,
                noalias_class: 0x1122_3344_5566_7788,
            },
        ),
        (
            4,
            Op::PipelineCreate {
                result: Id::new(0x7654_3210),
                view: value(),
                buffers: 0x1020_3040,
                prefetch_distance: 0x1020_3040,
            },
        ),
        (
            5,
            Op::PipelineEvent {
                pipeline: value(),
                epoch: value(),
                slot: value(),
                kind: PipelineEventKindAttr::Consume,
            },
        ),
        (
            6,
            Op::IndexConstant {
                result: Id::new(0x7654_3210),
                value: 0x1122_3344_5566_7788,
            },
        ),
        (
            7,
            Op::IndexUnsignedCast {
                result: Id::new(0x7654_3210),
                source: value(),
                bit_width: 7,
            },
        ),
        (
            8,
            Op::IndexUnknown {
                result: Id::new(0x7654_3210),
            },
        ),
        (
            9,
            Op::InvocationIndex {
                result: Id::new(0x7654_3210),
                dimension: 0x1020_3040,
                launch_extent: 0x1122_3344_5566_7788,
            },
        ),
        (
            10,
            Op::IndexBinary {
                result: Id::new(0x7654_3210),
                kind: IndexBinaryKindAttr::Remainder,
                lhs: value(),
                rhs: value(),
            },
        ),
        (
            11,
            Op::DeterministicJoin {
                result: Id::new(0x7654_3210),
                dependencies: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
            },
        ),
        (
            12,
            Op::CheckedTiledIndex2D {
                result: Id::new(0x7654_3210),
                invocation: value(),
                component: value(),
                rows: value(),
                columns: value(),
                row_stride: value(),
                lanes_per_tile: 0x1122_3344_5566_7788,
                tile_rows: 0x1122_3344_5566_7788,
                tile_columns: 0x1122_3344_5566_7788,
                elements_per_lane: 0x1122_3344_5566_7788,
            },
        ),
        (
            13,
            Op::CheckedRowStripedIndex2D {
                result: Id::new(0x7654_3210),
                invocation: value(),
                component: value(),
                rows: value(),
                columns: value(),
                row_stride: value(),
                lanes_per_row: 0x1122_3344_5566_7788,
                elements_per_lane: 0x1122_3344_5566_7788,
            },
        ),
        (
            14,
            Op::PredicatedCheckedTiledIndex2D {
                result: Id::new(0x7654_3210),
                success: Id::new(0x7654_3210),
                invocation: value(),
                component: value(),
                rows: value(),
                columns: value(),
                row_stride: value(),
                physical_extent: value(),
                lanes_per_tile: 0x1122_3344_5566_7788,
                tile_rows: 0x1122_3344_5566_7788,
                tile_columns: 0x1122_3344_5566_7788,
                elements_per_lane: 0x1122_3344_5566_7788,
            },
        ),
        (
            15,
            Op::PredicatedCheckedRowStripedIndex2D {
                result: Id::new(0x7654_3210),
                success: Id::new(0x7654_3210),
                invocation: value(),
                component: value(),
                rows: value(),
                columns: value(),
                row_stride: value(),
                physical_extent: value(),
                lanes_per_row: 0x1122_3344_5566_7788,
                elements_per_lane: 0x1122_3344_5566_7788,
            },
        ),
        (
            16,
            Op::Dimension {
                result: Id::new(0x7654_3210),
                view: value(),
                dimension: 0x1020_3040,
            },
        ),
        (
            17,
            Op::Access {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                view: value(),
                indices: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
            },
        ),
        (
            18,
            Op::PredicatedAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                view: value(),
                index: value(),
                success: value(),
            },
        ),
        (
            19,
            Op::ValueAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                view: value(),
                indices: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                value: value(),
            },
        ),
        (
            20,
            Op::AtomicAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                ordering: AtomicOrderingAttr::AcquireRelease,
                scope: AtomicScopeAttr::System,
                view: value(),
                indices: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
            },
        ),
        (
            21,
            Op::AtomicValueAccess {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                ordering: AtomicOrderingAttr::AcquireRelease,
                scope: AtomicScopeAttr::System,
                view: value(),
                indices: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                value: value(),
            },
        ),
        (
            22,
            Op::OwnershipContract {
                view: value(),
                coverage: OwnershipCoverageAttr::ExactEffectDomain,
                partition: OwnershipPartitionAttr::DenseRectangles,
            },
        ),
        (
            23,
            Op::AllocationEffect {
                kind: AccessKindAttr::AtomicReadModifyWrite,
                memory_space: MemorySpaceAttr::Workgroup,
                allocation_origin: 0x1122_3344_5566_7788,
                noalias_class: 0x1122_3344_5566_7788,
            },
        ),
        (
            24,
            Op::Barrier {
                execution_scope: HierarchyAttr::Subgroup,
                memory_scope: MemoryScopeAttr::Device,
                address_space: AddressSpaceAttr::Generic,
                order: MemoryOrderAttr::SequentiallyConsistent,
            },
        ),
        (
            25,
            Op::Fence {
                memory_scope: MemoryScopeAttr::Device,
                address_space: AddressSpaceAttr::Generic,
                order: MemoryOrderAttr::SequentiallyConsistent,
            },
        ),
        (
            26,
            Op::TensorLayout {
                contract: TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                convergence: TensorConvergenceAttr::UniformSubgroup,
                active_lanes: 0x1020_3040,
                binding: Some(cooperative()),
            },
        ),
        (
            27,
            Op::TensorResultComponent {
                result: Id::new(0x7654_3210),
                tensor_result_root: digest(31),
                component: 7,
                scalar: scalar(),
                numerical_contract: numerical(),
            },
        ),
        (
            28,
            Op::SemanticSymbol {
                result: Id::new(0x7654_3210),
                symbol: 0x1020_3040,
            },
        ),
        (
            29,
            Op::SemanticConstant {
                result: Id::new(0x7654_3210),
                value: 0x1122_3344_5566_7788,
            },
        ),
        (
            30,
            Op::SemanticBinary {
                result: Id::new(0x7654_3210),
                kind: SemanticBinaryKindAttr::Multiply,
                lhs: value(),
                rhs: value(),
            },
        ),
        (
            31,
            Op::SemanticExpression {
                result: Id::new(0x7654_3210),
                expression: expression(),
                numerical_contract: numerical(),
            },
        ),
        (
            32,
            Op::CollectiveSemantics {
                contract: collective(),
                view: value(),
                actual: value(),
                expected: value(),
                witness0: value(),
                witness1: value(),
            },
        ),
        (
            33,
            Op::RequireEquivalent {
                actual: value(),
                expected: value(),
            },
        ),
        (
            34,
            Op::RequireAuthenticatedReferenceEquivalent {
                actual: value(),
                expected: value(),
                proof: proof(0),
            },
        ),
        (
            35,
            Op::RequestAuthenticatedReferenceEquivalent {
                actual: value(),
                expected: value(),
                subjects: subjects(),
            },
        ),
        (
            36,
            Op::RequireEffectRefinement {
                contract: effect(),
                proof: proof(0),
            },
        ),
        (
            37,
            Op::RequestEffectRefinement {
                contract: effect(),
                subjects: subjects(),
            },
        ),
        (
            38,
            Op::RequireNumericalRefinement {
                contract: numerical_refinement(),
                proof: proof(0),
            },
        ),
        (
            39,
            Op::RequestNumericalRefinement {
                contract: numerical_refinement(),
                subjects: subjects(),
            },
        ),
        (
            40,
            Op::RequireTensorRefinement {
                contract: tensor_refinement(),
                proof: proof(0),
            },
        ),
        (
            41,
            Op::RequestTensorRefinement {
                contract: tensor_refinement(),
                subjects: subjects(),
            },
        ),
    ]
}
fn terminator_cases() -> Vec<(u16, Term)> {
    vec![
        (
            1,
            Term::IndexLessThan {
                lhs: value(),
                rhs: value(),
                true_block: 0x1020_3040,
                false_block: 0x1020_3040,
            },
        ),
        (
            2,
            Term::IndexLessThanArgs {
                lhs: value(),
                rhs: value(),
                true_arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                false_arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                true_block: 0x1020_3040,
                false_block: 0x1020_3040,
            },
        ),
        (
            3,
            Term::IndexEqual {
                lhs: value(),
                rhs: value(),
                true_block: 0x1020_3040,
                false_block: 0x1020_3040,
            },
        ),
        (
            4,
            Term::IndexEqualArgs {
                lhs: value(),
                rhs: value(),
                true_arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                false_arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                true_block: 0x1020_3040,
                false_block: 0x1020_3040,
            },
        ),
        (
            5,
            Term::AnalysisSplit {
                control_dependencies: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                first_block: 0x1020_3040,
                second_block: 0x1020_3040,
            },
        ),
        (
            6,
            Term::AnalysisSplitArgs {
                control_dependencies: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                first_arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                second_arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                first_block: 0x1020_3040,
                second_block: 0x1020_3040,
            },
        ),
        (
            7,
            Term::Branch {
                target: 0x1020_3040,
            },
        ),
        (
            8,
            Term::BranchArgs {
                arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                target: 0x1020_3040,
            },
        ),
        (
            9,
            Term::BranchArgsAdd {
                value: value(),
                step: value(),
                target: 0x1020_3040,
            },
        ),
        (
            10,
            Term::BranchArgsAddAt {
                arguments: vec![
                    value(),
                    Value::Local(Id::new(9)),
                    Value::BlockArgument {
                        block: 11,
                        argument: 13,
                    },
                ],
                add_argument: 0x1020_3040,
                step: value(),
                target: 0x1020_3040,
            },
        ),
        (11, Term::Return),
        (12, Term::Trap),
    ]
}

#[test]
fn all_forty_one_operations_roundtrip_the_full_typed_payload_without_admission() {
    let cases = operation_cases();
    assert_eq!(cases.len(), 41);
    for (expected_tag, expected) in cases {
        let mut work = Work::new(100_000_000);
        let mut budget = Budget::new(&mut work, 8 * tests::FIXTURE_FLOOR);
        budget.reserve_storage(tests::FIXTURE_FLOOR).unwrap();
        let mut counter = Writer::counter(&mut budget);
        operations::operation(&mut counter, &expected).unwrap();
        let length = counter.length;
        let shape = counter.shape;
        drop(counter);
        let mut writer = Writer::owned(length, &mut budget).unwrap();
        operations::operation(&mut writer, &expected).unwrap();
        let bytes = writer.into_bytes().unwrap();
        assert_eq!(&bytes[..4], &[expected_tag as u8, 0, 0, 0]);
        let mut reader = Reader::new(&bytes, false, &mut budget);
        assert!(decode::operation(&mut reader).unwrap().is_none());
        assert_eq!(reader.position, bytes.len());
        assert_eq!(reader.shape, shape);
        drop(reader);
        let mut reader = Reader::new(&bytes, true, &mut budget);
        let actual = decode::operation(&mut reader).unwrap().unwrap();
        assert_eq!(actual, expected, "wire tag {expected_tag}");
        assert_eq!(reader.position, bytes.len());
        assert_eq!(reader.shape, shape);
        // This does NOT call Kernel::new or claim that each standalone op is a valid kernel.
        drop(actual);
        drop(reader);
        drop(bytes);
    }
}

#[test]
fn all_twelve_terminators_preserve_every_ordered_argument_and_coordinate() {
    let cases = terminator_cases();
    assert_eq!(cases.len(), 12);
    for (tag, expected) in cases {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, tests::FIXTURE_FLOOR * 2);
        budget.reserve_storage(tests::FIXTURE_FLOOR).unwrap();
        let mut counter = Writer::counter(&mut budget);
        operations::terminator(&mut counter, &expected).unwrap();
        let length = counter.length;
        drop(counter);
        let mut writer = Writer::owned(length, &mut budget).unwrap();
        operations::terminator(&mut writer, &expected).unwrap();
        let bytes = writer.into_bytes().unwrap();
        assert_eq!(&bytes[..4], &[tag as u8, 0, 0, 0]);
        let mut reader = Reader::new(&bytes, true, &mut budget);
        let actual = decode::terminator(&mut reader).unwrap().unwrap();
        assert_eq!(actual, expected);
        assert_eq!(reader.position, bytes.len());
    }
}

#[test]
fn all_eight_expression_variants_and_typed_policy_fields_roundtrip() {
    let cases = vec![
        Expr::Symbol {
            symbol: 123,
            scalar: scalar(),
        },
        expression(),
        Expr::Load(ProductionSemanticLoadV2 {
            block: 4,
            operation: 5,
            scalar: scalar(),
            allocation_origin: 7,
            view: value(),
            indices: vec![value()].into_boxed_slice(),
        }),
        Expr::Unary {
            operation: ProductionSemanticUnaryOpV2::Negate,
            scalar: scalar(),
            operand: Box::new(expression()),
        },
        Expr::Binary {
            operation: ProductionSemanticBinaryOpV2::ShiftRight,
            scalar: scalar(),
            overflow: ProductionOverflowContractV2::Checked,
            lhs: Box::new(expression()),
            rhs: Box::new(expression()),
        },
        Expr::Compare {
            operation: ProductionSemanticComparisonV2::GreaterOrEqual,
            operand_scalar: scalar(),
            lhs: Box::new(expression()),
            rhs: Box::new(expression()),
        },
        Expr::Select {
            scalar: scalar(),
            condition: Box::new(Expr::Constant {
                scalar: Scalar::Bool,
                bits: 1,
            }),
            when_true: Box::new(expression()),
            when_false: Box::new(expression()),
        },
        Expr::Cast {
            kind: ProductionSemanticCastV2::FloatToIntegerSaturating,
            source: Scalar::Float { bits: 32 },
            target: scalar(),
            operand: Box::new(expression()),
        },
    ];
    for (index, expected) in cases.into_iter().enumerate() {
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, tests::FIXTURE_FLOOR * 2);
        budget.reserve_storage(tests::FIXTURE_FLOOR).unwrap();
        let mut counter = Writer::counter(&mut budget);
        counter.expression(&expected).unwrap();
        let length = counter.length;
        drop(counter);
        let mut writer = Writer::owned(length, &mut budget).unwrap();
        writer.expression(&expected).unwrap();
        let bytes = writer.into_bytes().unwrap();
        assert_eq!(&bytes[..2], &[(index + 1) as u8, 0]);
        let mut reader = Reader::new(&bytes, true, &mut budget);
        let actual = reader.expression().unwrap().unwrap();
        assert_eq!(actual, expected);
        assert_eq!(reader.position, bytes.len());
    }
}

macro_rules! enum_tags {
    ($method:ident, [$($value:expr),+ $(,)?]) => {{
        let values = [$($value),+];
        for (index, value) in values.into_iter().enumerate() {
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, tests::FIXTURE_FLOOR);
            let mut writer = Writer::owned(2, &mut budget).unwrap();
            writer.$method(value).unwrap(); let bytes = writer.into_bytes().unwrap();
            assert_eq!(bytes, vec![(index + 1) as u8, 0]);
            let mut reader = Reader::new(&bytes, false, &mut budget);
            assert_eq!(reader.$method().unwrap(), value);
        }
        for tag in [0u16, (values.len() + 1) as u16, u16::MAX] {
            let bytes = tag.to_le_bytes(); let mut work = Work::new(100);
            let mut budget = Budget::new(&mut work, 100_000);
            let mut reader = Reader::new(&bytes, false, &mut budget);
            assert!(matches!(reader.$method(), Err(E::Tag { tag: actual, .. }) if actual == tag));
        }
    }};
}
#[test]
fn all_closed_metadata_tags_are_explicit_and_unknown_values_refuse() {
    enum_tags!(
        access,
        [
            AccessKindAttr::Read,
            AccessKindAttr::Write,
            AccessKindAttr::AtomicRead,
            AccessKindAttr::AtomicWrite,
            AccessKindAttr::AtomicReadModifyWrite
        ]
    );
    enum_tags!(
        coverage,
        [
            OwnershipCoverageAttr::ExactView,
            OwnershipCoverageAttr::ExactEffectDomain,
            OwnershipCoverageAttr::TotalView,
            OwnershipCoverageAttr::CollectiveContributions
        ]
    );
    enum_tags!(
        partition,
        [
            OwnershipPartitionAttr::ExactSets,
            OwnershipPartitionAttr::DenseRectangles
        ]
    );
    enum_tags!(
        ordering,
        [
            AtomicOrderingAttr::Relaxed,
            AtomicOrderingAttr::Acquire,
            AtomicOrderingAttr::Release,
            AtomicOrderingAttr::AcquireRelease,
            AtomicOrderingAttr::SequentiallyConsistent
        ]
    );
    enum_tags!(
        atomic_scope,
        [
            AtomicScopeAttr::SingleThread,
            AtomicScopeAttr::Workgroup,
            AtomicScopeAttr::Agent,
            AtomicScopeAttr::Device,
            AtomicScopeAttr::System
        ]
    );
    enum_tags!(
        memory_space,
        [
            MemorySpaceAttr::Private,
            MemorySpaceAttr::Workgroup,
            MemorySpaceAttr::Global
        ]
    );
    enum_tags!(
        index_kind,
        [
            IndexBinaryKindAttr::Add,
            IndexBinaryKindAttr::Multiply,
            IndexBinaryKindAttr::Remainder,
            IndexBinaryKindAttr::Divide
        ]
    );
    enum_tags!(
        pipeline_event,
        [
            PipelineEventKindAttr::Stage,
            PipelineEventKindAttr::Commit,
            PipelineEventKindAttr::Wait,
            PipelineEventKindAttr::Consume,
            PipelineEventKindAttr::Discard,
            PipelineEventKindAttr::Release
        ]
    );
    enum_tags!(
        convergence,
        [
            TensorConvergenceAttr::UniformSubgroup,
            TensorConvergenceAttr::Divergent,
            TensorConvergenceAttr::UniformWorkgroup,
            TensorConvergenceAttr::Opaque
        ]
    );
    enum_tags!(
        evaluation_order,
        [
            SemanticEvaluationOrderAttr::Ascending,
            SemanticEvaluationOrderAttr::Descending,
            SemanticEvaluationOrderAttr::Lexicographic,
            SemanticEvaluationOrderAttr::Explicit
        ]
    );
    enum_tags!(
        semantic_coverage,
        [
            SemanticCoverageBindingAttr::TotalView,
            SemanticCoverageBindingAttr::CollectiveContributions
        ]
    );
    enum_tags!(
        semantic_binary,
        [
            SemanticBinaryKindAttr::Add,
            SemanticBinaryKindAttr::Multiply
        ]
    );
    enum_tags!(
        hierarchy,
        [
            HierarchyAttr::Grid,
            HierarchyAttr::Workgroup,
            HierarchyAttr::Subgroup,
            HierarchyAttr::Lane
        ]
    );
    enum_tags!(
        address_space,
        [
            AddressSpaceAttr::Private,
            AddressSpaceAttr::Workgroup,
            AddressSpaceAttr::Global,
            AddressSpaceAttr::Constant,
            AddressSpaceAttr::Generic
        ]
    );
    enum_tags!(
        memory_scope,
        [
            MemoryScopeAttr::Subgroup,
            MemoryScopeAttr::Workgroup,
            MemoryScopeAttr::Device,
            MemoryScopeAttr::System
        ]
    );
    enum_tags!(
        memory_order,
        [
            MemoryOrderAttr::Acquire,
            MemoryOrderAttr::Release,
            MemoryOrderAttr::AcquireRelease,
            MemoryOrderAttr::SequentiallyConsistent
        ]
    );
    enum_tags!(
        rounding,
        [
            ProductionIeeeRoundingModeV2::NearestTiesToEven,
            ProductionIeeeRoundingModeV2::TowardZero,
            ProductionIeeeRoundingModeV2::TowardPositive,
            ProductionIeeeRoundingModeV2::TowardNegative
        ]
    );
    enum_tags!(
        exceptional,
        [
            ProductionIeeeExceptionalValuePolicyV2::PreserveExactBits,
            ProductionIeeeExceptionalValuePolicyV2::CanonicalNan
        ]
    );
    enum_tags!(
        overflow,
        [
            ProductionOverflowContractV2::Wrapping,
            ProductionOverflowContractV2::Checked
        ]
    );
    enum_tags!(
        unary,
        [
            ProductionSemanticUnaryOpV2::Not,
            ProductionSemanticUnaryOpV2::Negate
        ]
    );
    enum_tags!(
        binary,
        [
            ProductionSemanticBinaryOpV2::Add,
            ProductionSemanticBinaryOpV2::Subtract,
            ProductionSemanticBinaryOpV2::Multiply,
            ProductionSemanticBinaryOpV2::Divide,
            ProductionSemanticBinaryOpV2::Remainder,
            ProductionSemanticBinaryOpV2::BitXor,
            ProductionSemanticBinaryOpV2::BitAnd,
            ProductionSemanticBinaryOpV2::BitOr,
            ProductionSemanticBinaryOpV2::ShiftLeft,
            ProductionSemanticBinaryOpV2::ShiftRight
        ]
    );
    enum_tags!(
        comparison,
        [
            ProductionSemanticComparisonV2::Equal,
            ProductionSemanticComparisonV2::LessThan,
            ProductionSemanticComparisonV2::LessOrEqual,
            ProductionSemanticComparisonV2::NotEqual,
            ProductionSemanticComparisonV2::GreaterOrEqual,
            ProductionSemanticComparisonV2::GreaterThan
        ]
    );
    enum_tags!(
        cast,
        [
            ProductionSemanticCastV2::Integer,
            ProductionSemanticCastV2::IntegerToFloat,
            ProductionSemanticCastV2::FloatToFloat,
            ProductionSemanticCastV2::FloatToIntegerSaturating
        ]
    );
    enum_tags!(
        collective_kind,
        [
            ProductionCollectiveSemanticKindV1::FiniteFold,
            ProductionCollectiveSemanticKindV1::FiniteRecurrence,
            ProductionCollectiveSemanticKindV1::PermutationGather
        ]
    );
}
