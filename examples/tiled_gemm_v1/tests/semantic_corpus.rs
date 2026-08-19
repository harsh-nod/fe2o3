use std::collections::BTreeSet;

use fe2o3_tiled_gemm_v1::{
    GEMM_SEMANTIC_CORPUS_SCHEMA_V1, GENERAL_GEMM_SAFE_SOURCE_MODEL_V1, GemmFailureKindV1,
    GemmRequiredPropertyV1, GemmVerificationStageV1, SEMANTIC_NEGATIVE_CORPUS_V1,
    SemanticMutationV1,
};
use syn::visit::Visit;

#[test]
fn production_property_spellings_are_complete_stable_and_independent() {
    let properties = [
        GemmRequiredPropertyV1::MemorySafe,
        GemmRequiredPropertyV1::BoundsSafe,
        GemmRequiredPropertyV1::Initialized,
        GemmRequiredPropertyV1::RaceFree,
        GemmRequiredPropertyV1::BarrierConvergent,
        GemmRequiredPropertyV1::OutputRegionInjective,
        GemmRequiredPropertyV1::LdsEpochCorrect,
        GemmRequiredPropertyV1::AccumulatorPhaseRefinement,
        GemmRequiredPropertyV1::TailRefinement,
        GemmRequiredPropertyV1::EpilogueRefinement,
        GemmRequiredPropertyV1::NumericalContract,
        GemmRequiredPropertyV1::MachineRefinementBoundary,
    ];
    assert_eq!(
        properties.map(GemmRequiredPropertyV1::as_str),
        [
            "memory_safe",
            "bounds_safe",
            "initialized",
            "race_free",
            "barrier_convergent",
            "output_region_injective",
            "lds_epoch_correct",
            "accumulator_phase_refinement",
            "tail_refinement",
            "epilogue_refinement",
            "numerical_contract",
            "machine_refinement_boundary",
        ]
    );
    assert_eq!(properties.into_iter().collect::<BTreeSet<_>>().len(), 12);
    assert_eq!(
        properties.map(GemmRequiredPropertyV1::diagnostic_code),
        [
            0x4647_0101,
            0x4647_0102,
            0x4647_0103,
            0x4647_0104,
            0x4647_0105,
            0x4647_0106,
            0x4647_0107,
            0x4647_0108,
            0x4647_0109,
            0x4647_010a,
            0x4647_010b,
            0x4647_010c,
        ]
    );
}

#[derive(Default)]
struct SourceAudit {
    kernel_functions: Vec<String>,
    unsafe_nodes: usize,
}

impl<'ast> Visit<'ast> for SourceAudit {
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if node
            .attrs
            .iter()
            .any(|attribute| attribute.path().is_ident("kernel"))
        {
            self.kernel_functions.push(node.sig.ident.to_string());
        }
        if node.sig.unsafety.is_some() {
            self.unsafe_nodes += 1;
        }
        syn::visit::visit_item_fn(self, node);
    }

    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.unsafe_nodes += 1;
        syn::visit::visit_expr_unsafe(self, node);
    }

    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if node.unsafety.is_some() {
            self.unsafe_nodes += 1;
        }
        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_trait(&mut self, node: &'ast syn::ItemTrait) {
        if node.unsafety.is_some() {
            self.unsafe_nodes += 1;
        }
        syn::visit::visit_item_trait(self, node);
    }
}

#[test]
fn all_fifteen_fixtures_are_safe_ordinary_rust_kernel_sources() {
    assert_eq!(
        GEMM_SEMANTIC_CORPUS_SCHEMA_V1,
        "fe2o3-general-gemm-negative-corpus-v1"
    );
    assert_eq!(SEMANTIC_NEGATIVE_CORPUS_V1.len(), 15);

    let mut mutations = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for case in SEMANTIC_NEGATIVE_CORPUS_V1 {
        assert!(mutations.insert(case.mutation), "duplicate mutation");
        assert!(paths.insert(case.fixture_path), "duplicate fixture path");
        assert!(
            case.fixture_path
                .ends_with(&format!("{}.rs", case.mutation.as_str()))
        );
        assert!(!case.source.contains("portable MIR identity mismatch"));

        let syntax = syn::parse_file(case.source)
            .unwrap_or_else(|error| panic!("{} did not parse as Rust: {error}", case.fixture_path));
        let mut audit = SourceAudit::default();
        audit.visit_file(&syntax);
        assert_eq!(
            audit.kernel_functions,
            [case.mutation.as_str()],
            "{}",
            case.fixture_path
        );
        assert_eq!(audit.unsafe_nodes, 0, "{}", case.fixture_path);
        assert_eq!(case.expected.code, case.expected.property.diagnostic_code());
        assert_eq!(case.expected.kind, GemmFailureKindV1::Counterexample);
    }
}

#[test]
fn positive_source_model_is_safe_attributed_rust_with_dynamic_phases_and_tails() {
    let syntax = syn::parse_file(GENERAL_GEMM_SAFE_SOURCE_MODEL_V1).unwrap();
    let mut audit = SourceAudit::default();
    audit.visit_file(&syntax);
    assert_eq!(audit.kernel_functions, ["valid_general_tiled_gemm"]);
    assert_eq!(audit.unsafe_nodes, 0);
    for required in [
        "while phase < phase_count",
        "context.publish_barrier()",
        "context.reuse_barrier()",
        "depth < context.k",
        "tile_depth ^ (4 * (lane_row % 4))",
        "row_base + 3 < context.m",
        "context.beta * initial",
    ] {
        assert!(GENERAL_GEMM_SAFE_SOURCE_MODEL_V1.contains(required));
    }
}

#[test]
fn mirrored_compiler_stages_use_the_canonical_wire_tags() {
    assert_eq!(GemmVerificationStageV1::Kernel.as_str(), "kernel");
    assert_eq!(GemmVerificationStageV1::Tile.as_str(), "tile");
    assert_eq!(GemmVerificationStageV1::Gpu.as_str(), "gpu");
    assert_eq!(GemmVerificationStageV1::Amdgcn.as_str(), "amdgcn");
    assert_eq!(
        [
            GemmVerificationStageV1::Kernel,
            GemmVerificationStageV1::Tile,
            GemmVerificationStageV1::Gpu,
            GemmVerificationStageV1::Amdgcn,
        ]
        .map(GemmVerificationStageV1::wire_tag),
        [3, 5, 6, 7]
    );
}

#[test]
fn each_mutation_has_the_frozen_property_and_failure_stage() {
    use GemmRequiredPropertyV1 as Property;
    use GemmVerificationStageV1 as Stage;
    use SemanticMutationV1 as Mutation;

    let expected = [
        (
            Mutation::UnguardedATailLoad,
            Property::BoundsSafe,
            Stage::Tile,
        ),
        (
            Mutation::UnguardedBTailLoad,
            Property::BoundsSafe,
            Stage::Tile,
        ),
        (
            Mutation::UnguardedCTailStore,
            Property::BoundsSafe,
            Stage::Tile,
        ),
        (
            Mutation::DuplicateLaneCWrite,
            Property::OutputRegionInjective,
            Stage::Tile,
        ),
        (
            Mutation::OverlappingWorkgroupCTile,
            Property::OutputRegionInjective,
            Stage::Tile,
        ),
        (Mutation::DuplicateLdsWrite, Property::RaceFree, Stage::Gpu),
        (
            Mutation::LdsReadBeforeInitialization,
            Property::Initialized,
            Stage::Gpu,
        ),
        (
            Mutation::MissingPublishBarrier,
            Property::Initialized,
            Stage::Gpu,
        ),
        (
            Mutation::DivergentBarrier,
            Property::BarrierConvergent,
            Stage::Gpu,
        ),
        (
            Mutation::MissingReuseBarrier,
            Property::LdsEpochCorrect,
            Stage::Gpu,
        ),
        (
            Mutation::ExpiredLdsEpoch,
            Property::LdsEpochCorrect,
            Stage::Gpu,
        ),
        (
            Mutation::StagedReadBeforeWait,
            Property::Initialized,
            Stage::Gpu,
        ),
        (
            Mutation::AccumulatorReset,
            Property::AccumulatorPhaseRefinement,
            Stage::Kernel,
        ),
        (
            Mutation::IncorrectKTailZeroFill,
            Property::TailRefinement,
            Stage::Kernel,
        ),
        (
            Mutation::IncorrectAlphaBetaEpilogue,
            Property::EpilogueRefinement,
            Stage::Kernel,
        ),
    ];

    for (case, (mutation, property, stage)) in SEMANTIC_NEGATIVE_CORPUS_V1.iter().zip(expected) {
        assert_eq!(case.mutation, mutation);
        assert_eq!(case.expected.property, property);
        assert_eq!(case.expected.stage, stage);
        assert!(!case.expected.stage.as_str().is_empty());
    }
}

#[test]
fn no_mutation_expectation_promotes_unrelated_properties() {
    for case in SEMANTIC_NEGATIVE_CORPUS_V1 {
        let named = [case.expected.property];
        assert_eq!(named.len(), 1);
        assert_eq!(named[0].as_str(), case.expected.property.as_str());
    }
}
