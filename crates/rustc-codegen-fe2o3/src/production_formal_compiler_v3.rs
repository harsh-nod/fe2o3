//! Crate-private three-track custody for the bounded Formal Compiler V3 claim.
//!
//! This joins lower-compiler CFG/value and byte-memory evidence with the exact
//! ranked dynamic-bounds roster. It is diagnostic evidence only: no value in
//! this module grants lowering, artifact, publication, load, or launch authority.

use std::{collections::BTreeSet, error::Error, fmt};

use dialect_kernel::AccessKindAttr;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticOperandV1, SemanticRvalueKindV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1,
};
use sha2::{Digest, Sha256};

use crate::production_ranked_projection_v1::{
    AuthenticatedRankedDynamicAffineGuardV3, AuthenticatedRankedDynamicAffineSiteViewV3,
    AuthenticatedRankedVerificationRosterV1,
};

const COMPOSITION_POLICY_VERSION_V3: u16 = 3;
const EVIDENCE_DOMAIN_V3: &[u8] = b"FE2O3/FORMAL-COMPILER/THREE-TRACK-EVIDENCE/V3\0";

const COMPOSITION_PROOF_SOURCE_SHA256_V3: [u8; 32] = [
    0x2c, 0xf2, 0x23, 0x26, 0x26, 0xb1, 0x44, 0xd9, 0x2a, 0xcf, 0xa5, 0xe5, 0x76, 0x35, 0xa8, 0x69,
    0xd9, 0x0c, 0xf2, 0x1b, 0x6d, 0x49, 0x7e, 0xc5, 0xf6, 0xa9, 0x49, 0x08, 0xf7, 0x13, 0xdc, 0xf3,
];
const COMPOSITION_VERUS_EXECUTABLE_SHA256_V3: [u8; 32] = [
    0xd9, 0x75, 0x01, 0xa8, 0x83, 0x93, 0x1d, 0x1d, 0x17, 0x3b, 0x1b, 0xf4, 0xb6, 0xcf, 0x4d, 0x97,
    0x3f, 0x16, 0xd1, 0x05, 0xdb, 0xcb, 0x46, 0x8e, 0x17, 0x7b, 0x52, 0xb2, 0x33, 0x16, 0x12, 0xd2,
];
const COMPOSITION_VERUS_CLOSURE_MANIFEST_SHA256_V3: [u8; 32] = [
    0xf0, 0x68, 0x83, 0xe4, 0xce, 0x46, 0x3b, 0xcb, 0x9a, 0x3c, 0x8f, 0x91, 0x10, 0x64, 0xac, 0x85,
    0x05, 0x4c, 0x78, 0x22, 0xdc, 0x33, 0x1d, 0xb1, 0xa7, 0x9f, 0x75, 0xf9, 0xe8, 0x87, 0x8b, 0x01,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProductionFormalCompilerStatusV3 {
    NotApplicable,
    Incomplete {
        cfg_value_verified: bool,
        memory_trace_verified: bool,
        dynamic_site_count: usize,
    },
    Proved(ProductionFormalCompilerEvidenceV3),
}

impl ProductionFormalCompilerStatusV3 {
    pub(crate) fn from_live_compilation(
        admitted: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
        ranked: &AuthenticatedRankedVerificationRosterV1,
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    ) -> Result<Self, ProductionFormalCompilerErrorV3> {
        admitted
            .verify_equivalence()
            .map_err(|error| ProductionFormalCompilerErrorV3::LiveOwner(error.to_string()))?;
        let lower =
            fe2o3_lower_mir_kernel::ProductionLowerCompilerStatusV3::from_live_owner(admitted)
                .map_err(ProductionFormalCompilerErrorV3::LowerCompiler)?;
        let dynamic_site_count = ranked
            .roots()
            .iter()
            .map(|root| root.dynamic_affine_sites_v3().len())
            .sum();
        match lower {
            fe2o3_lower_mir_kernel::ProductionLowerCompilerStatusV3::NotApplicable => {
                Ok(Self::NotApplicable)
            }
            fe2o3_lower_mir_kernel::ProductionLowerCompilerStatusV3::Incomplete {
                cfg_value_verified,
                memory_trace_verified,
            } => Ok(Self::Incomplete {
                cfg_value_verified,
                memory_trace_verified,
                dynamic_site_count,
            }),
            fe2o3_lower_mir_kernel::ProductionLowerCompilerStatusV3::Proved(lower) => {
                Self::compose_proved(admitted, ranked, typed_roots, &lower).map(Self::Proved)
            }
        }
    }

    fn compose_proved(
        admitted: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
        ranked: &AuthenticatedRankedVerificationRosterV1,
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        lower: &fe2o3_lower_mir_kernel::ProductionLowerCompilerEvidenceV3,
    ) -> Result<ProductionFormalCompilerEvidenceV3, ProductionFormalCompilerErrorV3> {
        let [ranked_root] = ranked.roots() else {
            return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "the exact fragment requires one ranked root",
            ));
        };
        let [typed_root] = typed_roots else {
            return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "the exact fragment requires one typed descriptor root",
            ));
        };
        let selector = lower.memory_trace().selector();
        let semantic = admitted.semantic_kir().semantic().semantic();
        let semantic_root = semantic
            .functions()
            .get(selector.root_function as usize)
            .ok_or(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "the lower evidence names a missing semantic root",
            ))?;
        if ranked_root.semantic_root().index() != selector.root_function
            || ranked_root.semantic_root_identity() != semantic_root.identity()
            || ranked_root.kernel_binding() != &typed_root.kernel_binding_bytes()
            || ranked_root.kernel_binding()
                != semantic_root
                    .kernel_entry()
                    .ok_or(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                        "the selected semantic root is not a kernel entry",
                    ))?
                    .kernel_binding_identity()
                    .as_bytes()
            || ranked_root.export_symbol() != typed_root.entry_symbol().as_bytes()
            || ranked_root.source_rank() != 1
        {
            return Err(ProductionFormalCompilerErrorV3::CrossOwnerBinding);
        }

        let mut sites = ranked_root
            .dynamic_affine_sites_v3()
            .map(snapshot_site_v3)
            .collect::<Vec<_>>();
        order_exact_sites_v3(&mut sites, selector)?;
        validate_exact_snapshot_v3(&sites, ranked_root.source_rank())?;
        replay_live_guard_bindings_v3(admitted, lower.memory_trace(), &sites)?;

        let identity = aggregate_identity_v3(
            lower.identity(),
            ranked.canonical_roster_identity().as_bytes(),
            ranked_root.semantic_root_identity().as_bytes(),
            ranked_root.kernel_binding(),
            &sites,
        );
        if identity == [0; 32] {
            return Err(ProductionFormalCompilerErrorV3::NonCanonicalEvidence);
        }
        Ok(ProductionFormalCompilerEvidenceV3 {
            identity,
            lower_identity: *lower.identity(),
            ranked_roster_identity: *ranked.canonical_roster_identity().as_bytes(),
            dynamic_site_count: sites.len(),
        })
    }

    pub(crate) fn revalidate_against(
        &self,
        admitted: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
        ranked: &AuthenticatedRankedVerificationRosterV1,
        typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    ) -> Result<(), ProductionFormalCompilerErrorV3> {
        (Self::from_live_compilation(admitted, ranked, typed_roots)? == *self)
            .then_some(())
            .ok_or(ProductionFormalCompilerErrorV3::NonCanonicalEvidence)
    }

    pub(crate) const fn status_name(&self) -> &'static str {
        match self {
            Self::NotApplicable => "not-applicable",
            Self::Incomplete { .. } => "incomplete",
            Self::Proved(_) => "proved",
        }
    }

    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductionFormalCompilerEvidenceV3 {
    identity: [u8; 32],
    lower_identity: [u8; 32],
    ranked_roster_identity: [u8; 32],
    dynamic_site_count: usize,
}

#[allow(dead_code)]
impl ProductionFormalCompilerEvidenceV3 {
    pub(crate) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(crate) const fn dynamic_site_count(&self) -> usize {
        self.dynamic_site_count
    }

    pub(crate) const fn claims_general_compiler_correctness(&self) -> bool {
        false
    }

    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GuardSnapshotV3 {
    branch_block: usize,
    branch_operation: usize,
    lhs: String,
    rhs: String,
    true_successor: usize,
    false_successor: usize,
    accepted_successor: usize,
    accepted_on_true: bool,
    normalized_constant: i128,
    normalized_coefficients: Vec<i128>,
    semantic_block: u32,
    semantic_statement: u32,
    semantic_condition_local: u32,
    semantic_lhs_local: u32,
    semantic_rhs_local: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct QuerySnapshotV3 {
    lower: Vec<i128>,
    upper_exclusive: Vec<i128>,
    constraints: Vec<(i128, Vec<i128>)>,
    constant: i128,
    coefficients: Vec<i128>,
    extent: u64,
    witness: Vec<i128>,
    lower_multipliers: Vec<u64>,
    upper_multipliers: Vec<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CertificateSnapshotV3 {
    extent_constant: i128,
    extent_coefficients: Vec<i128>,
    index: QuerySnapshotV3,
    slack: QuerySnapshotV3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProofSnapshotV3 {
    source: [u8; 32],
    v2_dependency: [u8; 32],
    verus: [u8; 32],
    closure: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SiteSnapshotV3 {
    kernel_binding: [u8; 32],
    semantic_root: u32,
    semantic_block: u32,
    semantic_statement: Option<u32>,
    semantic_access_ordinal: u32,
    block: usize,
    operation: usize,
    dimension: usize,
    access_kind: AccessKindAttr,
    access_view: String,
    access_index: String,
    dynamic_extent: String,
    runtime_variables: Vec<String>,
    guards: Vec<GuardSnapshotV3>,
    certificate: CertificateSnapshotV3,
    proof: ProofSnapshotV3,
}

fn snapshot_query_v3(
    certificate: &fe2o3_proof_contracts::ConstrainedAffineBoundsCertificateV2,
) -> QuerySnapshotV3 {
    let query = certificate.query();
    QuerySnapshotV3 {
        lower: query.lower().to_vec(),
        upper_exclusive: query.upper_exclusive().to_vec(),
        constraints: query
            .constraints()
            .iter()
            .map(|row| (row.constant(), row.coefficients().to_vec()))
            .collect(),
        constant: query.constant(),
        coefficients: query.coefficients().to_vec(),
        extent: query.extent(),
        witness: certificate.domain_witness().to_vec(),
        lower_multipliers: certificate.lower_multipliers().to_vec(),
        upper_multipliers: certificate.upper_multipliers().to_vec(),
    }
}

fn snapshot_guard_v3(guard: &AuthenticatedRankedDynamicAffineGuardV3) -> GuardSnapshotV3 {
    GuardSnapshotV3 {
        branch_block: guard.branch_block(),
        branch_operation: guard.branch_operation(),
        lhs: guard.lhs_value().to_owned(),
        rhs: guard.rhs_value().to_owned(),
        true_successor: guard.true_successor(),
        false_successor: guard.false_successor(),
        accepted_successor: guard.accepted_successor(),
        accepted_on_true: guard.accepted_on_true_edge(),
        normalized_constant: guard.normalized_constraint().constant(),
        normalized_coefficients: guard.normalized_constraint().coefficients().to_vec(),
        semantic_block: guard.semantic_block(),
        semantic_statement: guard.semantic_statement(),
        semantic_condition_local: guard.semantic_condition_local().index(),
        semantic_lhs_local: guard.semantic_lhs_local().index(),
        semantic_rhs_local: guard.semantic_rhs_local().index(),
    }
}

fn snapshot_site_v3(site: AuthenticatedRankedDynamicAffineSiteViewV3<'_>) -> SiteSnapshotV3 {
    let certificate = site.certificate();
    let proof = site.proof_binding();
    SiteSnapshotV3 {
        kernel_binding: *site.kernel_binding(),
        semantic_root: site.semantic_root().index(),
        semantic_block: site.semantic_block(),
        semantic_statement: site.semantic_statement(),
        semantic_access_ordinal: site.semantic_access_ordinal(),
        block: site.block(),
        operation: site.operation(),
        dimension: site.dimension(),
        access_kind: site.access_kind(),
        access_view: site.access_view().to_owned(),
        access_index: site.access_index().to_owned(),
        dynamic_extent: site.dynamic_extent().to_owned(),
        runtime_variables: site.runtime_variable_identities().to_vec(),
        guards: site.guards().iter().map(snapshot_guard_v3).collect(),
        certificate: CertificateSnapshotV3 {
            extent_constant: certificate.extent_constant(),
            extent_coefficients: certificate.extent_coefficients().to_vec(),
            index: snapshot_query_v3(certificate.index_certificate()),
            slack: snapshot_query_v3(certificate.slack_certificate()),
        },
        proof: ProofSnapshotV3 {
            source: proof.proof_source_sha256(),
            v2_dependency: proof.v2_dependency_source_sha256(),
            verus: proof.verus_executable_sha256(),
            closure: proof.verus_closure_manifest_sha256(),
        },
    }
}

fn order_exact_sites_v3(
    sites: &mut Vec<SiteSnapshotV3>,
    selector: fe2o3_lower_mir_kernel::ProductionMemoryTraceSelectorV3,
) -> Result<(), ProductionFormalCompilerErrorV3> {
    let expected = [selector.first_load, selector.second_load, selector.store];
    if sites.len() != expected.len() {
        return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
            "the exact lower fragment requires exactly three dynamic analysis sites",
        ));
    }
    sites.sort_by_key(|site| {
        expected
            .iter()
            .position(|&(block, statement)| {
                site.semantic_block == block && site.semantic_statement == Some(statement)
            })
            .unwrap_or(usize::MAX)
    });
    if sites.iter().zip(expected).any(|(site, expected)| {
        (site.semantic_block, site.semantic_statement) != (expected.0, Some(expected.1))
    }) {
        return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
            "dynamic analysis sites do not bijectively match load/load/store semantic sites",
        ));
    }
    Ok(())
}

fn validate_exact_snapshot_v3(
    sites: &[SiteSnapshotV3],
    source_rank: u8,
) -> Result<(), ProductionFormalCompilerErrorV3> {
    let [first, second, output] = sites else {
        return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
            "the exact snapshot must contain three sites",
        ));
    };
    let expected_kinds = [
        AccessKindAttr::Read,
        AccessKindAttr::Read,
        AccessKindAttr::Write,
    ];
    let expected_extent_coeff_rank = usize::from(source_rank) + sites.len();
    let common_guards = &first.guards;
    let common_index = &first.certificate.index;
    let views = [&first.access_view, &second.access_view, &output.access_view];
    let extents = [
        &first.dynamic_extent,
        &second.dynamic_extent,
        &output.dynamic_extent,
    ];
    let runtime_variables = &first.runtime_variables;
    let distinct_views = views
        .iter()
        .map(|value| value.as_str())
        .collect::<BTreeSet<_>>();
    let distinct_extents = extents
        .iter()
        .map(|value| value.as_str())
        .collect::<BTreeSet<_>>();
    let distinct_runtime_variables = runtime_variables
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if source_rank != 1
        || first.access_index.is_empty()
        || first.kernel_binding == [0; 32]
        || distinct_views.len() != 3
        || distinct_extents.len() != 3
        || runtime_variables.len() != 3
        || distinct_runtime_variables.len() != 3
        || distinct_runtime_variables != distinct_extents
        || common_guards.len() != 3
        || common_index.constraints.len() != 3
        || common_index.constant != 0
        || common_index.coefficients != [vec![1], vec![0; sites.len()]].concat()
        || common_index.lower.len() != expected_extent_coeff_rank
        || common_index.upper_exclusive.len() != expected_extent_coeff_rank
        || common_index.witness.len() != expected_extent_coeff_rank
        || !common_index.lower.iter().all(|value| *value == 0)
        || common_index.upper_exclusive[0] <= 0
        || !common_index.upper_exclusive[1..]
            .iter()
            .all(|value| *value == i128::from(u64::MAX) + 1)
    {
        return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
            "the shared one-dimensional gid certificate domain is not exact",
        ));
    }

    let mut extent_positions = BTreeSet::new();
    for (index, site) in sites.iter().enumerate() {
        let expected_proof = ProofSnapshotV3 {
            source: fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V3,
            v2_dependency:
                fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_V2_DEPENDENCY_SHA256_V3,
            verus: fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V3,
            closure:
                fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V3,
        };
        let Some(extent_position) = runtime_variables
            .iter()
            .position(|variable| variable == &site.dynamic_extent)
        else {
            return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "a dynamic extent is absent from the shared runtime-variable roster",
            ));
        };
        if !extent_positions.insert(extent_position) {
            return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "dynamic extents do not bijectively cover the shared runtime-variable roster",
            ));
        }
        let mut matching_guards = common_guards
            .iter()
            .enumerate()
            .filter(|(_, guard)| guard.rhs == site.dynamic_extent);
        let Some((guard_position, guard)) = matching_guards.next() else {
            return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "a dynamic extent has no exact guard in the shared guard roster",
            ));
        };
        if matching_guards.next().is_some() || guard_position != extent_position {
            return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "dynamic extents do not bijectively align guard rows with runtime coordinates",
            ));
        }
        let mut expected_extent = vec![0; expected_extent_coeff_rank];
        expected_extent[usize::from(source_rank) + extent_position] = 1;
        let mut expected_row = expected_extent
            .iter()
            .map(|value| -*value)
            .collect::<Vec<_>>();
        expected_row[0] = 1;
        if site.access_kind != expected_kinds[index]
            || site.dimension != 0
            || site.semantic_access_ordinal != 0
            || site.kernel_binding != first.kernel_binding
            || site.semantic_root != first.semantic_root
            || site.access_index != first.access_index
            || site.runtime_variables != *runtime_variables
            || site.guards != *common_guards
            || site.certificate.index != *common_index
            || site.certificate.extent_constant != 0
            || site.certificate.extent_coefficients != expected_extent
            || site.certificate.slack.lower != common_index.lower
            || site.certificate.slack.upper_exclusive != common_index.upper_exclusive
            || site.certificate.slack.constraints != common_index.constraints
            || site.certificate.slack.witness != common_index.witness
            || site.proof != expected_proof
            || guard.lhs != first.access_index
            || common_index.constraints[extent_position] != (1, expected_row)
            || guard.normalized_constant != common_index.constraints[extent_position].0
            || guard.normalized_coefficients != common_index.constraints[extent_position].1
        {
            return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
                "dynamic site, guard, certificate, or proof binding was substituted",
            ));
        }
    }
    if extent_positions.len() != 3
        || sites
            .iter()
            .map(|site| (site.block, site.operation, site.dimension))
            .collect::<BTreeSet<_>>()
            .len()
            != 3
        || common_guards.iter().any(|guard| {
            !guard.accepted_on_true
                || guard.accepted_successor != guard.true_successor
                || guard.true_successor == guard.false_successor
        })
        || common_guards
            .iter()
            .map(|guard| (guard.branch_block, guard.branch_operation))
            .collect::<BTreeSet<_>>()
            .len()
            != 3
        || common_guards
            .iter()
            .map(|guard| (guard.semantic_block, guard.semantic_statement))
            .collect::<BTreeSet<_>>()
            .len()
            != 3
    {
        return Err(ProductionFormalCompilerErrorV3::AnalysisInconsistency(
            "the ranked access or ordered guard roster is not exact",
        ));
    }
    Ok(())
}

fn replay_live_guard_bindings_v3(
    admitted: &fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1,
    memory: &fe2o3_lower_mir_kernel::ProductionMemoryTraceEvidenceV3,
    sites: &[SiteSnapshotV3],
) -> Result<(), ProductionFormalCompilerErrorV3> {
    let selector = memory.selector();
    let semantic_kir = admitted.semantic_kir();
    let semantic_function = semantic_kir
        .semantic()
        .semantic()
        .functions()
        .get(selector.root_function as usize)
        .ok_or(ProductionFormalCompilerErrorV3::CrossOwnerBinding)?;
    let lowered_function = semantic_kir
        .correspondence()
        .lowered_functions()
        .iter()
        .find(|function| function.semantic_function().index() == selector.root_function)
        .ok_or(ProductionFormalCompilerErrorV3::CrossOwnerBinding)?;
    let kir_function = semantic_kir
        .module()
        .functions
        .iter()
        .find(|function| &function.id == lowered_function.kernel_ir_function())
        .ok_or(ProductionFormalCompilerErrorV3::CrossOwnerBinding)?;
    let body = kir_function
        .body
        .as_ref()
        .ok_or(ProductionFormalCompilerErrorV3::CrossOwnerBinding)?;
    let guard_locations = memory.guard_locations();
    let length_values = memory.length_values();
    let guards = &sites[0].guards;

    for index in 0..3 {
        let mut matching_guards = guards
            .iter()
            .filter(|guard| guard.semantic_block == selector.guard_blocks[index]);
        let guard = matching_guards
            .next()
            .ok_or(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding)?;
        if matching_guards.next().is_some() {
            return Err(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding);
        }
        let block = semantic_function
            .blocks()
            .get(guard.semantic_block as usize)
            .ok_or(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding)?;
        let statement = block
            .statements()
            .get(guard.semantic_statement as usize)
            .ok_or(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding)?;
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Err(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding);
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        } = assignment.value().kind()
        else {
            return Err(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding);
        };
        let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } = block.terminator().kind()
        else {
            return Err(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding);
        };
        if !assignment.destination().projections().is_empty()
            || assignment.destination().local().index() != guard.semantic_condition_local
            || simple_operand_local_v3(left) != Some(guard.semantic_lhs_local)
            || simple_operand_local_v3(right) != Some(guard.semantic_rhs_local)
            || simple_operand_local_v3(discriminant) != Some(guard.semantic_condition_local)
        {
            return Err(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding);
        }

        let location = guard_locations[index];
        let mut spans = semantic_kir
            .correspondence()
            .statement_operation_spans()
            .iter()
            .filter(|span| {
                span.semantic_function().index() == selector.root_function
                    && span.semantic_block().index() == guard.semantic_block
                    && span.statement_ordinal() == guard.semantic_statement
            });
        let span = spans
            .next()
            .ok_or(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding)?;
        if spans.next().is_some()
            || span.kernel_ir_block() != location.block
            || span.first_operation_ordinal() as usize != location.operation_index
            || span.operation_count() != 1
        {
            return Err(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding);
        }
        let operation = body
            .blocks
            .iter()
            .find(|block| block.id == location.block)
            .and_then(|block| block.operations.get(location.operation_index))
            .ok_or(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding)?;
        match operation.kind {
            fe2o3_kernel_ir::OperationKind::Compare {
                predicate: fe2o3_kernel_ir::ComparePredicate::LessThan,
                lhs,
                rhs,
            } if lhs == memory.gid() && rhs == length_values[index] => {}
            _ => return Err(ProductionFormalCompilerErrorV3::CrossTrackGuardBinding),
        }
    }
    Ok(())
}

fn simple_operand_local_v3(operand: &SemanticOperandV1) -> Option<u32> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place.local().index())
        }
        SemanticOperandV1::Copy(_)
        | SemanticOperandV1::Move(_)
        | SemanticOperandV1::Constant(_) => None,
    }
}

fn aggregate_identity_v3(
    lower_identity: &[u8; 32],
    roster_identity: &[u8; 32],
    semantic_root_identity: &[u8; 32],
    kernel_binding: &[u8; 32],
    sites: &[SiteSnapshotV3],
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(EVIDENCE_DOMAIN_V3);
    hash.update(COMPOSITION_POLICY_VERSION_V3.to_le_bytes());
    hash.update(fe2o3_lower_mir_kernel::FORMAL_COMPILER_V3_CONTRACT_SHA256);
    hash.update(COMPOSITION_PROOF_SOURCE_SHA256_V3);
    hash.update(COMPOSITION_VERUS_EXECUTABLE_SHA256_V3);
    hash.update(COMPOSITION_VERUS_CLOSURE_MANIFEST_SHA256_V3);
    hash.update(crate::production_formal_compiler_v3_pin::FORMAL_COMPILER_V3_RUST_VALIDATOR_SHA256);
    hash.update(lower_identity);
    hash.update(roster_identity);
    hash.update(semantic_root_identity);
    hash.update(kernel_binding);
    hash.update((sites.len() as u64).to_le_bytes());
    for site in sites {
        hash_site_v3(&mut hash, site);
    }
    hash.finalize().into()
}

fn hash_site_v3(hash: &mut Sha256, site: &SiteSnapshotV3) {
    hash.update(site.kernel_binding);
    hash.update(site.semantic_root.to_le_bytes());
    hash.update(site.semantic_block.to_le_bytes());
    hash.update(site.semantic_statement.unwrap_or(u32::MAX).to_le_bytes());
    hash.update(site.semantic_access_ordinal.to_le_bytes());
    hash.update((site.block as u64).to_le_bytes());
    hash.update((site.operation as u64).to_le_bytes());
    hash.update((site.dimension as u64).to_le_bytes());
    hash.update([access_kind_tag_v3(site.access_kind)]);
    hash_string_v3(hash, &site.access_view);
    hash_string_v3(hash, &site.access_index);
    hash_string_v3(hash, &site.dynamic_extent);
    hash.update((site.runtime_variables.len() as u64).to_le_bytes());
    for value in &site.runtime_variables {
        hash_string_v3(hash, value);
    }
    hash.update((site.guards.len() as u64).to_le_bytes());
    for guard in &site.guards {
        hash.update((guard.branch_block as u64).to_le_bytes());
        hash.update((guard.branch_operation as u64).to_le_bytes());
        hash_string_v3(hash, &guard.lhs);
        hash_string_v3(hash, &guard.rhs);
        hash.update((guard.true_successor as u64).to_le_bytes());
        hash.update((guard.false_successor as u64).to_le_bytes());
        hash.update((guard.accepted_successor as u64).to_le_bytes());
        hash.update([u8::from(guard.accepted_on_true)]);
        hash.update(guard.normalized_constant.to_le_bytes());
        hash_i128_slice_v3(hash, &guard.normalized_coefficients);
        hash.update(guard.semantic_block.to_le_bytes());
        hash.update(guard.semantic_statement.to_le_bytes());
        hash.update(guard.semantic_condition_local.to_le_bytes());
        hash.update(guard.semantic_lhs_local.to_le_bytes());
        hash.update(guard.semantic_rhs_local.to_le_bytes());
    }
    hash_certificate_v3(hash, &site.certificate);
    hash.update(site.proof.source);
    hash.update(site.proof.v2_dependency);
    hash.update(site.proof.verus);
    hash.update(site.proof.closure);
}

fn hash_certificate_v3(hash: &mut Sha256, certificate: &CertificateSnapshotV3) {
    hash.update(certificate.extent_constant.to_le_bytes());
    hash_i128_slice_v3(hash, &certificate.extent_coefficients);
    for query in [&certificate.index, &certificate.slack] {
        hash_i128_slice_v3(hash, &query.lower);
        hash_i128_slice_v3(hash, &query.upper_exclusive);
        hash.update((query.constraints.len() as u64).to_le_bytes());
        for (constant, coefficients) in &query.constraints {
            hash.update(constant.to_le_bytes());
            hash_i128_slice_v3(hash, coefficients);
        }
        hash.update(query.constant.to_le_bytes());
        hash_i128_slice_v3(hash, &query.coefficients);
        hash.update(query.extent.to_le_bytes());
        hash_i128_slice_v3(hash, &query.witness);
        hash_u64_slice_v3(hash, &query.lower_multipliers);
        hash_u64_slice_v3(hash, &query.upper_multipliers);
    }
}

fn hash_string_v3(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

fn hash_i128_slice_v3(hash: &mut Sha256, values: &[i128]) {
    hash.update((values.len() as u64).to_le_bytes());
    for value in values {
        hash.update(value.to_le_bytes());
    }
}

fn hash_u64_slice_v3(hash: &mut Sha256, values: &[u64]) {
    hash.update((values.len() as u64).to_le_bytes());
    for value in values {
        hash.update(value.to_le_bytes());
    }
}

const fn access_kind_tag_v3(kind: AccessKindAttr) -> u8 {
    match kind {
        AccessKindAttr::Read => 0,
        AccessKindAttr::Write => 1,
        AccessKindAttr::AtomicRead => 2,
        AccessKindAttr::AtomicWrite => 3,
        AccessKindAttr::AtomicReadModifyWrite => 4,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ProductionFormalCompilerErrorV3 {
    LiveOwner(String),
    LowerCompiler(fe2o3_lower_mir_kernel::ProductionLowerCompilerErrorV3),
    AnalysisInconsistency(&'static str),
    CrossOwnerBinding,
    CrossTrackGuardBinding,
    NonCanonicalEvidence,
}

impl fmt::Display for ProductionFormalCompilerErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveOwner(error) => write!(formatter, "live owner failed: {error}"),
            Self::LowerCompiler(error) => write!(formatter, "lower compiler: {error}"),
            Self::AnalysisInconsistency(detail) => {
                write!(
                    formatter,
                    "ranked analysis is inconsistent with the proved fragment: {detail}"
                )
            }
            Self::CrossOwnerBinding => {
                formatter.write_str("lower, ranked, semantic, and descriptor owners differ")
            }
            Self::CrossTrackGuardBinding => formatter.write_str(
                "semantic MIR, KIR, and ranked guard bindings do not identify one exact comparison",
            ),
            Self::NonCanonicalEvidence => {
                formatter.write_str("Formal Compiler V3 evidence is noncanonical")
            }
        }
    }
}

impl Error for ProductionFormalCompilerErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_sites() -> Vec<SiteSnapshotV3> {
        let max_runtime = i128::from(u64::MAX) + 1;
        let constraints = vec![
            (1, vec![1, -1, 0, 0]),
            (1, vec![1, 0, -1, 0]),
            (1, vec![1, 0, 0, -1]),
        ];
        let common_index = QuerySnapshotV3 {
            lower: vec![0; 4],
            upper_exclusive: vec![256, max_runtime, max_runtime, max_runtime],
            constraints: constraints.clone(),
            constant: 0,
            coefficients: vec![1, 0, 0, 0],
            extent: u64::MAX,
            witness: vec![0, 1, 1, 1],
            lower_multipliers: vec![0; 11],
            upper_multipliers: vec![0; 11],
        };
        let guards = (0..3)
            .map(|index| GuardSnapshotV3 {
                branch_block: index + 1,
                branch_operation: 0,
                lhs: "v0".to_owned(),
                rhs: format!("v{}", index + 1),
                true_successor: index + 2,
                false_successor: 6,
                accepted_successor: index + 2,
                accepted_on_true: true,
                normalized_constant: constraints[index].0,
                normalized_coefficients: constraints[index].1.clone(),
                semantic_block: 20 + index as u32,
                semantic_statement: 0,
                semantic_condition_local: 10 + index as u32,
                semantic_lhs_local: 0,
                semantic_rhs_local: 1 + index as u32,
            })
            .collect::<Vec<_>>();
        (0..3)
            .map(|index| {
                let mut extent_coefficients = vec![0; 4];
                extent_coefficients[index + 1] = 1;
                SiteSnapshotV3 {
                    kernel_binding: [7; 32],
                    semantic_root: 0,
                    semantic_block: 10,
                    semantic_statement: Some(index as u32),
                    semantic_access_ordinal: 0,
                    block: 4,
                    operation: index,
                    dimension: 0,
                    access_kind: if index == 2 {
                        AccessKindAttr::Write
                    } else {
                        AccessKindAttr::Read
                    },
                    access_view: format!("v{}", index + 4),
                    access_index: "v0".to_owned(),
                    dynamic_extent: format!("v{}", index + 1),
                    runtime_variables: vec!["v1".to_owned(), "v2".to_owned(), "v3".to_owned()],
                    guards: guards.clone(),
                    certificate: CertificateSnapshotV3 {
                        extent_constant: 0,
                        extent_coefficients,
                        index: common_index.clone(),
                        slack: common_index.clone(),
                    },
                    proof: ProofSnapshotV3 {
                        source: fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V3,
                        v2_dependency: fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_V2_DEPENDENCY_SHA256_V3,
                        verus: fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V3,
                        closure: fe2o3_verifier::DYNAMIC_CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V3,
                    },
                }
            })
            .collect()
    }

    fn swap_affine_dimensions(values: &mut [i128], left: usize, right: usize) {
        values.swap(left + 1, right + 1);
    }

    fn swap_query_runtime_dimensions(query: &mut QuerySnapshotV3, left: usize, right: usize) {
        swap_affine_dimensions(&mut query.lower, left, right);
        swap_affine_dimensions(&mut query.upper_exclusive, left, right);
        swap_affine_dimensions(&mut query.coefficients, left, right);
        swap_affine_dimensions(&mut query.witness, left, right);
        for (_, coefficients) in &mut query.constraints {
            swap_affine_dimensions(coefficients, left, right);
        }
        query.constraints.swap(left, right);
        query.lower_multipliers.swap(left, right);
        query.upper_multipliers.swap(left, right);
    }

    fn swap_runtime_coordinate_order(sites: &mut [SiteSnapshotV3], left: usize, right: usize) {
        for site in sites {
            site.runtime_variables.swap(left, right);
            swap_affine_dimensions(&mut site.certificate.extent_coefficients, left, right);
            swap_query_runtime_dimensions(&mut site.certificate.index, left, right);
            swap_query_runtime_dimensions(&mut site.certificate.slack, left, right);
            for guard in &mut site.guards {
                swap_affine_dimensions(&mut guard.normalized_coefficients, left, right);
            }
            site.guards.swap(left, right);
        }
    }

    #[test]
    fn composition_proof_and_runtime_closure_pins_match() {
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(include_bytes!(
                "../../../formal/compiler-v3/guarded_u32_xor_helper_store_composition_v3.rs"
            ))),
            COMPOSITION_PROOF_SOURCE_SHA256_V3
        );
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(include_bytes!(
                "../../fe2o3-runtime-model/verus/pins/VERUS_CLOSURE_MANIFEST"
            ))),
            COMPOSITION_VERUS_CLOSURE_MANIFEST_SHA256_V3
        );
    }

    #[test]
    fn status_and_evidence_never_grant_authority() {
        let status = ProductionFormalCompilerStatusV3::NotApplicable;
        assert_eq!(status.status_name(), "not-applicable");
        assert!(!status.grants_artifact_or_launch_authority());
        let evidence = ProductionFormalCompilerEvidenceV3 {
            identity: [1; 32],
            lower_identity: [2; 32],
            ranked_roster_identity: [3; 32],
            dynamic_site_count: 3,
        };
        assert_eq!(evidence.identity(), &[1; 32]);
        assert_eq!(evidence.dynamic_site_count(), 3);
        assert!(!evidence.claims_general_compiler_correctness());
        assert!(!evidence.grants_artifact_or_launch_authority());
    }

    #[test]
    fn exact_three_site_snapshot_accepts_only_the_ordered_shape() {
        let sites = valid_sites();
        validate_exact_snapshot_v3(&sites, 1).unwrap();

        let mut mutations = Vec::new();
        let mut wrong_kind = sites.clone();
        wrong_kind[2].access_kind = AccessKindAttr::Read;
        mutations.push(wrong_kind);
        let mut wrong_dimension = sites.clone();
        wrong_dimension[0].dimension = 1;
        mutations.push(wrong_dimension);
        let mut wrong_ordinal = sites.clone();
        wrong_ordinal[1].semantic_access_ordinal = 1;
        mutations.push(wrong_ordinal);
        let mut duplicate_view = sites.clone();
        duplicate_view[1].access_view = duplicate_view[0].access_view.clone();
        mutations.push(duplicate_view);
        let mut inconsistent_runtime_roster = sites.clone();
        inconsistent_runtime_roster[0].runtime_variables.swap(0, 1);
        mutations.push(inconsistent_runtime_roster);
        let mut mismatched_runtime_coordinates = sites.clone();
        for site in &mut mismatched_runtime_coordinates {
            site.runtime_variables.swap(0, 1);
        }
        mutations.push(mismatched_runtime_coordinates);
        let mut mismatched_access_extents = sites.clone();
        let original_extents = mismatched_access_extents
            .iter()
            .map(|site| site.dynamic_extent.clone())
            .collect::<Vec<_>>();
        for (index, site) in mismatched_access_extents.iter_mut().enumerate() {
            site.dynamic_extent = original_extents[(index + 1) % original_extents.len()].clone();
        }
        mutations.push(mismatched_access_extents);
        let mut duplicate_runtime_variable = sites.clone();
        for site in &mut duplicate_runtime_variable {
            site.runtime_variables[1] = site.runtime_variables[0].clone();
        }
        mutations.push(duplicate_runtime_variable);
        let mut duplicate_access_site = sites.clone();
        duplicate_access_site[1].block = duplicate_access_site[0].block;
        duplicate_access_site[1].operation = duplicate_access_site[0].operation;
        duplicate_access_site[1].dimension = duplicate_access_site[0].dimension;
        mutations.push(duplicate_access_site);
        let mut bypass = sites.clone();
        for site in &mut bypass {
            site.guards[0].accepted_successor = 4;
        }
        mutations.push(bypass);
        let mut reversed_edge = sites.clone();
        for site in &mut reversed_edge {
            site.guards[1].accepted_on_true = false;
        }
        mutations.push(reversed_edge);
        let mut duplicate_guard_origin = sites.clone();
        for site in &mut duplicate_guard_origin {
            site.guards[1].branch_block = site.guards[0].branch_block;
            site.guards[1].branch_operation = site.guards[0].branch_operation;
        }
        mutations.push(duplicate_guard_origin);
        let mut changed_row = sites.clone();
        for site in &mut changed_row {
            site.certificate.index.constraints[1].0 = 0;
        }
        mutations.push(changed_row);
        let mut stale_pin = sites.clone();
        stale_pin[0].proof.source[0] ^= 1;
        mutations.push(stale_pin);

        for mutation in mutations {
            assert!(validate_exact_snapshot_v3(&mutation, 1).is_err());
        }
        assert!(validate_exact_snapshot_v3(&sites[..2], 1).is_err());
        assert!(validate_exact_snapshot_v3(&sites, 2).is_err());
    }

    #[test]
    fn accepts_authenticated_runtime_coordinate_and_cfg_layout_variants() {
        let mut permuted_runtime_roster = valid_sites();
        swap_runtime_coordinate_order(&mut permuted_runtime_roster, 0, 1);
        validate_exact_snapshot_v3(&permuted_runtime_roster, 1).unwrap();

        let mut permuted_access_extents = valid_sites();
        let original = permuted_access_extents.clone();
        for (index, source) in [2, 0, 1].into_iter().enumerate() {
            permuted_access_extents[index].dynamic_extent = original[source].dynamic_extent.clone();
            permuted_access_extents[index].certificate = original[source].certificate.clone();
        }
        validate_exact_snapshot_v3(&permuted_access_extents, 1).unwrap();

        let mut multiple_access_blocks = valid_sites();
        for (index, site) in multiple_access_blocks.iter_mut().enumerate() {
            site.block = 4 + index * 2;
            site.operation = 0;
        }
        validate_exact_snapshot_v3(&multiple_access_blocks, 1).unwrap();

        let mut effect_free_guard_bridges = valid_sites();
        for site in &mut effect_free_guard_bridges {
            for (index, guard) in site.guards.iter_mut().enumerate() {
                guard.true_successor = 20 + index;
                guard.accepted_successor = guard.true_successor;
            }
        }
        validate_exact_snapshot_v3(&effect_free_guard_bridges, 1).unwrap();
    }

    #[test]
    fn semantic_certificate_and_roster_substitutions_change_the_identity() {
        let sites = valid_sites();
        let identity = aggregate_identity_v3(&[1; 32], &[2; 32], &[3; 32], &[4; 32], &sites);
        let mut semantic = sites.clone();
        semantic[0].guards[0].semantic_condition_local ^= 1;
        assert_ne!(
            identity,
            aggregate_identity_v3(&[1; 32], &[2; 32], &[3; 32], &[4; 32], &semantic)
        );
        let mut witness = sites.clone();
        witness[2].certificate.slack.witness[0] ^= 1;
        assert_ne!(
            identity,
            aggregate_identity_v3(&[1; 32], &[2; 32], &[3; 32], &[4; 32], &witness)
        );
        assert_ne!(
            identity,
            aggregate_identity_v3(&[1; 32], &[9; 32], &[3; 32], &[4; 32], &sites)
        );
    }

    #[test]
    fn semantic_sites_are_bijectively_reordered_or_rejected() {
        let selector = fe2o3_lower_mir_kernel::ProductionMemoryTraceSelectorV3 {
            root_function: 0,
            guard_blocks: [20, 21, 22],
            enabled_block: 10,
            first_load: (10, 0),
            second_load: (10, 1),
            helper_call_block: 10,
            store: (10, 2),
            helper_function: 1,
        };
        let mut reordered = valid_sites();
        reordered.swap(0, 2);
        order_exact_sites_v3(&mut reordered, selector).unwrap();
        assert_eq!(reordered, valid_sites());

        let mut duplicate = valid_sites();
        duplicate[1].semantic_statement = duplicate[0].semantic_statement;
        assert!(order_exact_sites_v3(&mut duplicate, selector).is_err());
    }
}
