//! Bounded, inert no-overflow facts for canonical `u32` induction loops.
//!
//! This analysis recognizes identities and structure, never source names. Its
//! certificates describe semantic MIR facts only: they authenticate neither
//! the MIR producer nor any lowering, artifact, compiler action, or launch.

use std::{error::Error, fmt};

use crate::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, InertSemanticMirSha256V1, SemanticAssertMessageV1,
    SemanticBasicBlockV1, SemanticBinaryOpV1, SemanticBlockIdV1, SemanticBlockIdentityV1,
    SemanticCheckedBinaryOpV1, SemanticFunctionDeclV1, SemanticFunctionIdV1,
    SemanticFunctionIdentityV1, SemanticLocalIdV1, SemanticLocalIdentityV1, SemanticLocalRoleV1,
    SemanticOperandV1, SemanticPlaceV1, SemanticProjectionKindV1, SemanticRvalueKindV1,
    SemanticScalarTypeV1, SemanticScalarValueV1, SemanticStatementKindV1, SemanticTerminatorKindV1,
    SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeIdentityV1, SemanticTypeShapeV1,
    SemanticUnwindActionV1,
};

/// Maximum independently charged CFG, inventory, candidate, and reachability work.
pub const MAX_SEMANTIC_U32_INDUCTION_WORK_V1: usize = 4_000_000;

/// Maximum certificates retained by one function analysis.
pub const MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1: usize = 65_536;

/// Caller-selectable limits bounded by this analysis version's hard caps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionAnalysisLimitsV1 {
    work_units: usize,
    certificates: usize,
}

impl SemanticU32InductionAnalysisLimitsV1 {
    pub const fn new(work_units: usize, certificates: usize) -> Self {
        Self {
            work_units,
            certificates,
        }
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }

    pub const fn certificates(self) -> usize {
        self.certificates
    }
}

impl Default for SemanticU32InductionAnalysisLimitsV1 {
    fn default() -> Self {
        Self::new(
            MAX_SEMANTIC_U32_INDUCTION_WORK_V1,
            MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1,
        )
    }
}

/// Terminal failure of the bounded analysis itself.
///
/// Unsupported or hostile induction shapes are ordinary unproved candidates,
/// not analysis failures; they never produce a certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticU32InductionAnalysisErrorV1 {
    InvalidLimits {
        requested_work: usize,
        maximum_work: usize,
        requested_certificates: usize,
        maximum_certificates: usize,
    },
    InvalidControlFlow(&'static str),
    InvalidModel(&'static str),
    WorkLimit {
        actual: usize,
        limit: usize,
    },
    CertificateLimit {
        actual: usize,
        limit: usize,
    },
    Storage,
}

impl fmt::Display for SemanticU32InductionAnalysisErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits {
                requested_work,
                maximum_work,
                requested_certificates,
                maximum_certificates,
            } => write!(
                formatter,
                "semantic u32 induction limits request work {requested_work}/{maximum_work} and certificates {requested_certificates}/{maximum_certificates}"
            ),
            Self::InvalidControlFlow(detail) => {
                write!(
                    formatter,
                    "invalid semantic induction control flow: {detail}"
                )
            }
            Self::InvalidModel(detail) => {
                write!(formatter, "invalid semantic induction model: {detail}")
            }
            Self::WorkLimit { actual, limit } => write!(
                formatter,
                "semantic u32 induction analysis work {actual} exceeds {limit}"
            ),
            Self::CertificateLimit { actual, limit } => write!(
                formatter,
                "semantic u32 induction certificate count {actual} exceeds {limit}"
            ),
            Self::Storage => formatter.write_str("semantic u32 induction analysis storage failed"),
        }
    }
}

impl Error for SemanticU32InductionAnalysisErrorV1 {}

/// Exact identity of one semantic block occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionBlockSiteV1 {
    block: SemanticBlockIdV1,
    identity: SemanticBlockIdentityV1,
}

impl SemanticU32InductionBlockSiteV1 {
    pub const fn block(self) -> SemanticBlockIdV1 {
        self.block
    }

    pub const fn identity(self) -> SemanticBlockIdentityV1 {
        self.identity
    }
}

/// Exact statement position inside an identity-bound semantic block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionStatementSiteV1 {
    block: SemanticU32InductionBlockSiteV1,
    statement: u32,
}

impl SemanticU32InductionStatementSiteV1 {
    pub const fn block(self) -> SemanticU32InductionBlockSiteV1 {
        self.block
    }

    pub const fn statement(self) -> u32 {
        self.statement
    }
}

/// Exact unprojected place used by a recognized induction shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionPlaceBindingV1 {
    local: SemanticLocalIdV1,
    local_identity: SemanticLocalIdentityV1,
    ty: SemanticTypeIdV1,
    type_identity: SemanticTypeIdentityV1,
}

impl SemanticU32InductionPlaceBindingV1 {
    pub const fn local(self) -> SemanticLocalIdV1 {
        self.local
    }

    pub const fn local_identity(self) -> SemanticLocalIdentityV1 {
        self.local_identity
    }

    pub const fn ty(self) -> SemanticTypeIdV1 {
        self.ty
    }

    pub const fn type_identity(self) -> SemanticTypeIdentityV1 {
        self.type_identity
    }
}

/// Inert semantic fact that one exact checked `u32` addition cannot overflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionNoOverflowCertificateV1 {
    semantic_mir_sha256: InertSemanticMirSha256V1,
    function: SemanticFunctionIdV1,
    function_identity: SemanticFunctionIdentityV1,
    induction: SemanticU32InductionPlaceBindingV1,
    guard_induction: SemanticU32InductionPlaceBindingV1,
    bound: SemanticU32InductionPlaceBindingV1,
    predicate: SemanticU32InductionPlaceBindingV1,
    checked_result: SemanticU32InductionPlaceBindingV1,
    preheader: SemanticU32InductionBlockSiteV1,
    header: SemanticU32InductionBlockSiteV1,
    body_entry: SemanticU32InductionBlockSiteV1,
    exit: SemanticU32InductionBlockSiteV1,
    initialization: SemanticU32InductionStatementSiteV1,
    guard_induction_snapshot: Option<SemanticU32InductionStatementSiteV1>,
    guard: SemanticU32InductionStatementSiteV1,
    checked_addition: SemanticU32InductionStatementSiteV1,
    update: SemanticU32InductionStatementSiteV1,
}

impl SemanticU32InductionNoOverflowCertificateV1 {
    pub const fn semantic_mir_sha256(self) -> InertSemanticMirSha256V1 {
        self.semantic_mir_sha256
    }

    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }

    pub const fn function_identity(self) -> SemanticFunctionIdentityV1 {
        self.function_identity
    }

    pub const fn induction(self) -> SemanticU32InductionPlaceBindingV1 {
        self.induction
    }

    /// Exact value compared against the bound. This is either the induction
    /// local itself or a uniquely defined, single-use snapshot in the header.
    pub const fn guard_induction(self) -> SemanticU32InductionPlaceBindingV1 {
        self.guard_induction
    }

    pub const fn bound(self) -> SemanticU32InductionPlaceBindingV1 {
        self.bound
    }

    pub const fn predicate(self) -> SemanticU32InductionPlaceBindingV1 {
        self.predicate
    }

    pub const fn checked_result(self) -> SemanticU32InductionPlaceBindingV1 {
        self.checked_result
    }

    pub const fn preheader(self) -> SemanticU32InductionBlockSiteV1 {
        self.preheader
    }

    pub const fn header(self) -> SemanticU32InductionBlockSiteV1 {
        self.header
    }

    pub const fn body_entry(self) -> SemanticU32InductionBlockSiteV1 {
        self.body_entry
    }

    pub const fn exit(self) -> SemanticU32InductionBlockSiteV1 {
        self.exit
    }

    pub const fn initialization(self) -> SemanticU32InductionStatementSiteV1 {
        self.initialization
    }

    pub const fn guard_induction_snapshot(self) -> Option<SemanticU32InductionStatementSiteV1> {
        self.guard_induction_snapshot
    }

    pub const fn guard(self) -> SemanticU32InductionStatementSiteV1 {
        self.guard
    }

    pub const fn checked_addition(self) -> SemanticU32InductionStatementSiteV1 {
        self.checked_addition
    }

    pub const fn update(self) -> SemanticU32InductionStatementSiteV1 {
        self.update
    }

    /// The exact semantic checked addition is unreachable when `i >= bound`.
    /// Since both values are `u32`, `i < bound` implies `i < u32::MAX`.
    pub const fn establishes_semantic_no_overflow(self) -> bool {
        true
    }

    /// This read-only fact never grants compiler, artifact, or launch authority.
    pub const fn grants_authority(self) -> bool {
        false
    }

    /// A consumer must separately authenticate lineage and lowering semantics.
    pub const fn authorizes_compiler_transform(self) -> bool {
        false
    }
}

/// Bounded result for one exact semantic function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticU32InductionNoOverflowReportV1 {
    semantic_mir_sha256: InertSemanticMirSha256V1,
    function: SemanticFunctionIdV1,
    function_identity: SemanticFunctionIdentityV1,
    checked_additions_examined: usize,
    certificates: Box<[SemanticU32InductionNoOverflowCertificateV1]>,
    work_units: usize,
}

impl SemanticU32InductionNoOverflowReportV1 {
    pub const fn semantic_mir_sha256(&self) -> InertSemanticMirSha256V1 {
        self.semantic_mir_sha256
    }

    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }

    pub const fn function_identity(&self) -> SemanticFunctionIdentityV1 {
        self.function_identity
    }

    pub const fn checked_additions_examined(&self) -> usize {
        self.checked_additions_examined
    }

    pub fn certificates(&self) -> &[SemanticU32InductionNoOverflowCertificateV1] {
        &self.certificates
    }

    pub const fn work_units(&self) -> usize {
        self.work_units
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub const fn authorizes_compiler_transform(&self) -> bool {
        false
    }
}

pub fn analyze_semantic_u32_induction_no_overflow_v1(
    semantic_mir: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
) -> Result<SemanticU32InductionNoOverflowReportV1, SemanticU32InductionAnalysisErrorV1> {
    analyze_semantic_u32_induction_no_overflow_with_limits_v1(
        semantic_mir,
        function,
        SemanticU32InductionAnalysisLimitsV1::default(),
    )
}

pub fn analyze_semantic_u32_induction_no_overflow_with_limits_v1(
    semantic_mir: &AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    limits: SemanticU32InductionAnalysisLimitsV1,
) -> Result<SemanticU32InductionNoOverflowReportV1, SemanticU32InductionAnalysisErrorV1> {
    let declaration = semantic_mir
        .functions()
        .get(function.index() as usize)
        .ok_or(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "the requested semantic function is outside the admitted function table",
        ))?;
    analyze_function_with_limits_v1(
        semantic_mir.types(),
        declaration,
        semantic_mir.semantic_sha256(),
        function,
        limits,
    )
}

fn analyze_function_with_limits_v1(
    types: &[SemanticTypeDeclV1],
    declaration: &SemanticFunctionDeclV1,
    semantic_mir_sha256: InertSemanticMirSha256V1,
    function: SemanticFunctionIdV1,
    limits: SemanticU32InductionAnalysisLimitsV1,
) -> Result<SemanticU32InductionNoOverflowReportV1, SemanticU32InductionAnalysisErrorV1> {
    if limits.work_units > MAX_SEMANTIC_U32_INDUCTION_WORK_V1
        || limits.certificates > MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1
    {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidLimits {
            requested_work: limits.work_units,
            maximum_work: MAX_SEMANTIC_U32_INDUCTION_WORK_V1,
            requested_certificates: limits.certificates,
            maximum_certificates: MAX_SEMANTIC_U32_INDUCTION_CERTIFICATES_V1,
        });
    }

    let mut budget = WorkBudgetV1::new(limits.work_units);
    let graph = SemanticCfgV1::analyze(declaration, &mut budget)?;
    let inventory = SemanticInventoryV1::analyze(declaration, &mut budget)?;
    let mut certificates = Vec::new();
    certificates
        .try_reserve(inventory.checked_additions.len().min(limits.certificates))
        .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
    let context = CandidateProofContextV1 {
        types,
        function: declaration,
        semantic_mir_sha256,
        function_id: function,
        graph: &graph,
        inventory: &inventory,
    };
    for candidate in &inventory.checked_additions {
        budget.charge(1)?;
        if let Some(certificate) = prove_candidate_v1(&context, *candidate, &mut budget)? {
            let actual = certificates.len().saturating_add(1);
            if actual > limits.certificates {
                return Err(SemanticU32InductionAnalysisErrorV1::CertificateLimit {
                    actual,
                    limit: limits.certificates,
                });
            }
            certificates.push(certificate);
        }
    }
    Ok(SemanticU32InductionNoOverflowReportV1 {
        semantic_mir_sha256,
        function,
        function_identity: declaration.identity(),
        checked_additions_examined: inventory.checked_additions.len(),
        certificates: certificates.into_boxed_slice(),
        work_units: budget.used,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DefinitionPositionV1 {
    Statement(usize),
    Terminator,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DefinitionSiteV1 {
    block: usize,
    position: DefinitionPositionV1,
}

#[derive(Clone, Copy, Debug, Default)]
struct DefinitionSummaryV1 {
    count: u8,
    first: Option<DefinitionSiteV1>,
    second: Option<DefinitionSiteV1>,
}

impl DefinitionSummaryV1 {
    fn record(&mut self, site: DefinitionSiteV1) {
        self.count = self.count.saturating_add(1);
        if self.first.is_none() {
            self.first = Some(site);
        } else if self.second.is_none() {
            self.second = Some(site);
        }
    }

    fn is_unique_at(self, site: DefinitionSiteV1) -> bool {
        self.count == 1 && self.first == Some(site)
    }

    fn is_exact_pair(self, left: DefinitionSiteV1, right: DefinitionSiteV1) -> bool {
        self.count == 2
            && matches!(
                (self.first, self.second),
                (Some(first), Some(second))
                    if (first == left && second == right) || (first == right && second == left)
            )
    }
}

#[derive(Clone, Copy, Debug)]
struct CandidateSiteV1 {
    block: usize,
    statement: usize,
}

#[derive(Clone, Copy)]
struct CandidateProofContextV1<'a> {
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
    semantic_mir_sha256: InertSemanticMirSha256V1,
    function_id: SemanticFunctionIdV1,
    graph: &'a SemanticCfgV1,
    inventory: &'a SemanticInventoryV1,
}

struct SemanticInventoryV1 {
    definitions: Vec<DefinitionSummaryV1>,
    address_or_projection_hazard: Vec<bool>,
    direct_copy_alias: Vec<bool>,
    use_counts: Vec<usize>,
    checked_additions: Vec<CandidateSiteV1>,
}

impl SemanticInventoryV1 {
    fn analyze(
        function: &SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticU32InductionAnalysisErrorV1> {
        let local_count = function.locals().len();
        let mut definitions = fallible_filled_vec(local_count, DefinitionSummaryV1::default())?;
        let mut address_or_projection_hazard = fallible_filled_vec(local_count, false)?;
        let mut direct_copy_alias = fallible_filled_vec(local_count, false)?;
        let mut use_counts = fallible_filled_vec(local_count, 0_usize)?;
        let mut checked_additions = Vec::new();
        budget.charge(local_count)?;

        for (block_index, block) in function.blocks().iter().enumerate() {
            budget.charge(1)?;
            for (statement_index, statement) in block.statements().iter().enumerate() {
                budget.charge(1)?;
                let site = DefinitionSiteV1 {
                    block: block_index,
                    position: DefinitionPositionV1::Statement(statement_index),
                };
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        record_definition(
                            assignment.destination(),
                            site,
                            &mut definitions,
                            &mut address_or_projection_hazard,
                            budget,
                        )?;
                        if let SemanticRvalueKindV1::CheckedBinary(checked) =
                            assignment.value().kind()
                            && checked.operation() == SemanticCheckedBinaryOpV1::Add
                        {
                            checked_additions
                                .try_reserve(1)
                                .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
                            checked_additions.push(CandidateSiteV1 {
                                block: block_index,
                                statement: statement_index,
                            });
                        }
                        if let SemanticRvalueKindV1::Use(source) = assignment.value().kind()
                            && let Some(place) = operand_place(source)
                            && place.projections().is_empty()
                            && place.local() != assignment.destination().local()
                        {
                            let slot = local_slot_mut(
                                &mut direct_copy_alias,
                                place.local(),
                                "a copied semantic local is outside the local table",
                            )?;
                            *slot = true;
                        }
                        inspect_rvalue(
                            assignment.value().kind(),
                            &mut address_or_projection_hazard,
                            &mut use_counts,
                            budget,
                        )?;
                    }
                    SemanticStatementKindV1::Store(store) => {
                        record_definition(
                            store.destination(),
                            site,
                            &mut definitions,
                            &mut address_or_projection_hazard,
                            budget,
                        )?;
                        mark_address_hazard(
                            store.destination(),
                            &mut address_or_projection_hazard,
                            budget,
                        )?;
                        inspect_operand(
                            store.value(),
                            &mut address_or_projection_hazard,
                            &mut use_counts,
                            budget,
                        )?;
                    }
                    SemanticStatementKindV1::AtomicRmw(atomic) => {
                        record_definition(
                            atomic.destination(),
                            site,
                            &mut definitions,
                            &mut address_or_projection_hazard,
                            budget,
                        )?;
                        mark_address_hazard(
                            atomic.address(),
                            &mut address_or_projection_hazard,
                            budget,
                        )?;
                        inspect_operand(
                            atomic.value(),
                            &mut address_or_projection_hazard,
                            &mut use_counts,
                            budget,
                        )?;
                    }
                    SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                        record_definition(
                            atomic.destination(),
                            site,
                            &mut definitions,
                            &mut address_or_projection_hazard,
                            budget,
                        )?;
                        mark_address_hazard(
                            atomic.address(),
                            &mut address_or_projection_hazard,
                            budget,
                        )?;
                        inspect_operand(
                            atomic.expected(),
                            &mut address_or_projection_hazard,
                            &mut use_counts,
                            budget,
                        )?;
                        inspect_operand(
                            atomic.replacement(),
                            &mut address_or_projection_hazard,
                            &mut use_counts,
                            budget,
                        )?;
                    }
                    SemanticStatementKindV1::SetDiscriminant { place, .. }
                    | SemanticStatementKindV1::Deinitialize(place) => record_definition(
                        place,
                        site,
                        &mut definitions,
                        &mut address_or_projection_hazard,
                        budget,
                    )?,
                    SemanticStatementKindV1::Assume(operand) => inspect_operand(
                        operand,
                        &mut address_or_projection_hazard,
                        &mut use_counts,
                        budget,
                    )?,
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop => {}
                }
            }
            inspect_terminator(
                block,
                block_index,
                &mut definitions,
                &mut address_or_projection_hazard,
                &mut use_counts,
                budget,
            )?;
        }
        Ok(Self {
            definitions,
            address_or_projection_hazard,
            direct_copy_alias,
            use_counts,
            checked_additions,
        })
    }
}

fn prove_candidate_v1(
    context: &CandidateProofContextV1<'_>,
    candidate: CandidateSiteV1,
    budget: &mut WorkBudgetV1,
) -> Result<Option<SemanticU32InductionNoOverflowCertificateV1>, SemanticU32InductionAnalysisErrorV1>
{
    let CandidateProofContextV1 {
        types,
        function,
        semantic_mir_sha256,
        function_id,
        graph,
        inventory,
    } = *context;
    let Some(candidate_block) = function.blocks().get(candidate.block) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked-add candidate is outside the block table",
        ));
    };
    let Some(candidate_statement) = candidate_block.statements().get(candidate.statement) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked-add candidate is outside its block statement table",
        ));
    };
    let SemanticStatementKindV1::Assign(checked_assignment) = candidate_statement.kind() else {
        return Ok(None);
    };
    let SemanticRvalueKindV1::CheckedBinary(checked) = checked_assignment.value().kind() else {
        return Ok(None);
    };
    if checked.operation() != SemanticCheckedBinaryOpV1::Add
        || !checked_assignment.destination().projections().is_empty()
        || checked_assignment.value().result_type() != checked_assignment.destination().ty()
    {
        return Ok(None);
    }
    let result_local = checked_assignment.destination().local();
    let result_ty = checked_assignment.destination().ty();
    let Some(result_decl) = function.locals().get(result_local.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked-result local is outside the local table",
        ));
    };
    let Some(induction_place) = exact_operand_place(checked.left()) else {
        return Ok(None);
    };
    let induction = induction_place.local();
    let induction_ty = induction_place.ty();
    if !is_exact_u32(types, induction_ty)
        || !is_exact_u32_constant(checked.right(), induction_ty, 1)
        || !is_exact_checked_u32_tuple(types, result_ty, induction_ty)
        || checked.left().ty() != induction_ty
        || checked.right().ty() != induction_ty
        || result_decl.ty() != result_ty
        || result_decl.role() != SemanticLocalRoleV1::Temporary
    {
        return Ok(None);
    }

    let candidate_definition = DefinitionSiteV1 {
        block: candidate.block,
        position: DefinitionPositionV1::Statement(candidate.statement),
    };
    if !definition(inventory, result_local)?.is_unique_at(candidate_definition)
        || use_count(inventory, result_local)? != 2
        || local(inventory.address_or_projection_hazard.as_slice(), induction)?
    {
        return Ok(None);
    }
    let Some(induction_decl) = function.locals().get(induction.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "an induction local is outside the local table",
        ));
    };
    if induction_decl.ty() != induction_ty
        || induction_decl.role() != SemanticLocalRoleV1::Temporary
    {
        return Ok(None);
    }

    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        target,
        unwind,
    } = candidate_block.terminator().kind()
    else {
        return Ok(None);
    };
    let SemanticAssertMessageV1::Overflow {
        operation,
        left: message_left,
        right: message_right,
    } = message
    else {
        return Ok(None);
    };
    if *expected
        || *operation != SemanticBinaryOpV1::Add
        || !same_operand_value(message_left, checked.left())
        || !same_operand_value(message_right, checked.right())
        || !field_operand_matches(
            condition,
            result_local,
            1,
            bool_type_of_checked_tuple(types, result_ty)?,
        )
        || target.role() != crate::semantic_mir_v1::SemanticEdgeRoleV1::AssertSuccess
        || !matches!(unwind, SemanticUnwindActionV1::Unreachable)
    {
        return Ok(None);
    }
    let update_block_index = target.target().index() as usize;
    if !graph.has_unique_predecessor(update_block_index, candidate.block) {
        return Ok(None);
    }
    let Some(update_block) = function.blocks().get(update_block_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an assertion-success edge is outside the block table",
        ));
    };
    let SemanticTerminatorKindV1::Goto(backedge) = update_block.terminator().kind() else {
        return Ok(None);
    };
    if backedge.role() != crate::semantic_mir_v1::SemanticEdgeRoleV1::Goto {
        return Ok(None);
    }
    let header_index = backedge.target().index() as usize;
    let Some(header) = function.blocks().get(header_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction backedge is outside the block table",
        ));
    };

    let mut update_site = None;
    for site in [
        definition(inventory, induction)?.first,
        definition(inventory, induction)?.second,
    ]
    .into_iter()
    .flatten()
    {
        budget.charge(1)?;
        if site.block != update_block_index {
            continue;
        }
        let DefinitionPositionV1::Statement(statement_index) = site.position else {
            return Ok(None);
        };
        let Some(statement) = update_block.statements().get(statement_index) else {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                "an induction definition is outside its block statement table",
            ));
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(None);
        };
        if !is_exact_destination(assignment.destination(), induction, induction_ty)
            || assignment.value().result_type() != induction_ty
            || !matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Use(operand)
                    if field_operand_matches(operand, result_local, 0, induction_ty)
            )
            || update_site.replace(site).is_some()
        {
            return Ok(None);
        }
    }
    let Some(update_site) = update_site else {
        return Ok(None);
    };

    let mut guard_match = None;
    for (statement_index, statement) in header.statements().iter().enumerate() {
        budget.charge(1)?;
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            continue;
        };
        let SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::LessThan,
            left,
            right,
        } = assignment.value().kind()
        else {
            continue;
        };
        let Some(left_place) = exact_operand_place(left) else {
            continue;
        };
        let guard_induction = if left_place.local() == induction && left_place.ty() == induction_ty
        {
            Some((induction, None))
        } else {
            match_guard_induction_snapshot_v1(
                types,
                function,
                header,
                header_index,
                statement_index,
                induction,
                induction_ty,
                left_place,
                inventory,
            )?
        };
        if let Some((guard_induction, guard_snapshot)) = guard_induction
            && guard_match
                .replace((
                    statement_index,
                    assignment,
                    right,
                    guard_induction,
                    guard_snapshot,
                ))
                .is_some()
        {
            return Ok(None);
        }
    }
    let Some((
        guard_statement,
        guard_assignment,
        bound_operand,
        guard_induction,
        guard_induction_snapshot,
    )) = guard_match
    else {
        return Ok(None);
    };
    let Some(bound_place) = exact_operand_place(bound_operand) else {
        return Ok(None);
    };
    let bound = bound_place.local();
    let bound_ty = bound_place.ty();
    let predicate = guard_assignment.destination().local();
    let predicate_ty = guard_assignment.destination().ty();
    let Some(predicate_decl) = function.locals().get(predicate.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "an induction predicate is outside the local table",
        ));
    };
    let guard_definition = DefinitionSiteV1 {
        block: header_index,
        position: DefinitionPositionV1::Statement(guard_statement),
    };
    let Some(bound_decl) = function.locals().get(bound.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "an induction bound is outside the local table",
        ));
    };
    if induction == bound
        || !is_exact_u32(types, bound_ty)
        || bound_ty != induction_ty
        || bound_decl.ty() != bound_ty
        || !matches!(bound_decl.role(), SemanticLocalRoleV1::Argument(_))
        || definition(inventory, bound)?.count != 0
        || local(inventory.address_or_projection_hazard.as_slice(), bound)?
        || local(inventory.direct_copy_alias.as_slice(), bound)?
        || !guard_assignment.destination().projections().is_empty()
        || guard_assignment.value().result_type() != predicate_ty
        || !is_exact_bool(types, predicate_ty)
        || predicate_decl.ty() != predicate_ty
        || predicate_decl.role() != SemanticLocalRoleV1::Temporary
        || !definition(inventory, predicate)?.is_unique_at(guard_definition)
        || use_count(inventory, predicate)? != 1
    {
        return Ok(None);
    }

    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = header.terminator().kind()
    else {
        return Ok(None);
    };
    if !exact_operand_place(discriminant)
        .is_some_and(|place| place.local() == predicate && place.ty() == predicate_ty)
        || targets.values().len() != 1
        || targets.values()[0].value() != 0
        || targets.values()[0].edge().role()
            != crate::semantic_mir_v1::SemanticEdgeRoleV1::SwitchValue
        || targets.otherwise().role() != crate::semantic_mir_v1::SemanticEdgeRoleV1::SwitchOtherwise
    {
        return Ok(None);
    }
    let exit_index = targets.values()[0].edge().target().index() as usize;
    let body_entry_index = targets.otherwise().target().index() as usize;
    if exit_index == body_entry_index
        || !graph.has_unique_predecessor(body_entry_index, header_index)
        || !graph.has_unique_predecessor(exit_index, header_index)
        || !graph.has_unique_predecessor(update_block_index, candidate.block)
        || !graph.dominates(body_entry_index, candidate.block, budget)?
        || !graph.dominates(header_index, update_block_index, budget)?
    {
        return Ok(None);
    }

    let header_predecessors = graph.predecessors(header_index)?;
    if header_predecessors.len() != 2 || !header_predecessors.contains(&update_block_index) {
        return Ok(None);
    }
    let Some(preheader_index) = header_predecessors
        .iter()
        .copied()
        .find(|predecessor| *predecessor != update_block_index)
    else {
        return Ok(None);
    };
    let Some(preheader) = function.blocks().get(preheader_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction preheader is outside the block table",
        ));
    };
    if !matches!(
        preheader.terminator().kind(),
        SemanticTerminatorKindV1::Goto(edge)
            if edge.role() == crate::semantic_mir_v1::SemanticEdgeRoleV1::Goto
                && edge.target().index() as usize == header_index
    ) || !graph.dominates(preheader_index, header_index, budget)?
        || graph.dominates(header_index, preheader_index, budget)?
    {
        return Ok(None);
    }

    let mut initialization_site = None;
    for site in [
        definition(inventory, induction)?.first,
        definition(inventory, induction)?.second,
    ]
    .into_iter()
    .flatten()
    {
        budget.charge(1)?;
        if site.block != preheader_index {
            continue;
        }
        let DefinitionPositionV1::Statement(statement_index) = site.position else {
            return Ok(None);
        };
        let Some(statement) = preheader.statements().get(statement_index) else {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                "an induction initialization is outside its block statement table",
            ));
        };
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Ok(None);
        };
        if !is_exact_destination(assignment.destination(), induction, induction_ty)
            || assignment.value().result_type() != induction_ty
            || !matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Use(operand)
                    if is_exact_u32_constant(operand, induction_ty, 0)
            )
            || initialization_site.replace(site).is_some()
        {
            return Ok(None);
        }
    }
    let Some(initialization_site) = initialization_site else {
        return Ok(None);
    };
    if !definition(inventory, induction)?.is_exact_pair(initialization_site, update_site) {
        return Ok(None);
    }

    let Some(exit) = function.blocks().get(exit_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction exit is outside the block table",
        ));
    };
    let Some(body_entry) = function.blocks().get(body_entry_index) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
            "an induction body entry is outside the block table",
        ));
    };
    Ok(Some(SemanticU32InductionNoOverflowCertificateV1 {
        semantic_mir_sha256,
        function: function_id,
        function_identity: function.identity(),
        induction: place_binding(types, function, induction)?,
        guard_induction: place_binding(types, function, guard_induction)?,
        bound: place_binding(types, function, bound)?,
        predicate: place_binding(types, function, predicate)?,
        checked_result: place_binding(types, function, result_local)?,
        preheader: block_site(preheader_index, preheader)?,
        header: block_site(header_index, header)?,
        body_entry: block_site(body_entry_index, body_entry)?,
        exit: block_site(exit_index, exit)?,
        initialization: statement_site(preheader_index, preheader, initialization_site)?,
        guard_induction_snapshot: guard_induction_snapshot
            .map(|site| statement_site(header_index, header, site))
            .transpose()?,
        guard: statement_site(header_index, header, guard_definition)?,
        checked_addition: statement_site(candidate.block, candidate_block, candidate_definition)?,
        update: statement_site(update_block_index, update_block, update_site)?,
    }))
}

#[allow(clippy::too_many_arguments)]
fn match_guard_induction_snapshot_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    header: &SemanticBasicBlockV1,
    header_index: usize,
    guard_statement: usize,
    induction: SemanticLocalIdV1,
    induction_ty: SemanticTypeIdV1,
    snapshot_place: &SemanticPlaceV1,
    inventory: &SemanticInventoryV1,
) -> Result<
    Option<(SemanticLocalIdV1, Option<DefinitionSiteV1>)>,
    SemanticU32InductionAnalysisErrorV1,
> {
    let snapshot = snapshot_place.local();
    let snapshot_ty = snapshot_place.ty();
    if snapshot == induction || snapshot_ty != induction_ty || !is_exact_u32(types, snapshot_ty) {
        return Ok(None);
    }
    let Some(snapshot_decl) = function.locals().get(snapshot.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a guard induction snapshot is outside the local table",
        ));
    };
    let Some(snapshot_definition) = definition(inventory, snapshot)?.first else {
        return Ok(None);
    };
    let expected_definition = DefinitionSiteV1 {
        block: header_index,
        position: DefinitionPositionV1::Statement(match snapshot_definition.position {
            DefinitionPositionV1::Statement(statement) => statement,
            DefinitionPositionV1::Terminator => return Ok(None),
        }),
    };
    let DefinitionPositionV1::Statement(snapshot_statement) = snapshot_definition.position else {
        return Ok(None);
    };
    if snapshot_statement >= guard_statement
        || snapshot_decl.ty() != snapshot_ty
        || snapshot_decl.role() != SemanticLocalRoleV1::Temporary
        || !definition(inventory, snapshot)?.is_unique_at(expected_definition)
        || use_count(inventory, snapshot)? != 1
        || local(inventory.address_or_projection_hazard.as_slice(), snapshot)?
    {
        return Ok(None);
    }
    let Some(statement) = header.statements().get(snapshot_statement) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a guard induction snapshot definition is outside the header statement table",
        ));
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Ok(None);
    };
    if !is_exact_destination(assignment.destination(), snapshot, snapshot_ty)
        || assignment.value().result_type() != snapshot_ty
        || !matches!(
            assignment.value().kind(),
            SemanticRvalueKindV1::Use(operand)
                if exact_operand_place(operand).is_some_and(|place| {
                    place.local() == induction && place.ty() == induction_ty
                })
        )
    {
        return Ok(None);
    }
    Ok(Some((snapshot, Some(snapshot_definition))))
}

struct SemanticCfgV1 {
    entry: usize,
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
}

impl SemanticCfgV1 {
    fn analyze(
        function: &SemanticFunctionDeclV1,
        budget: &mut WorkBudgetV1,
    ) -> Result<Self, SemanticU32InductionAnalysisErrorV1> {
        let block_count = function.blocks().len();
        let entry = function.entry().index() as usize;
        if block_count == 0 || entry >= block_count {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                "the semantic function has no valid entry block",
            ));
        }
        let mut successors = fallible_nested_vec(block_count)?;
        let mut predecessors = fallible_nested_vec(block_count)?;
        budget.charge(block_count)?;
        for (source, block) in function.blocks().iter().enumerate() {
            block.terminator().kind().try_for_each_edge(|edge| {
                budget.charge(1)?;
                let target = edge.target().index() as usize;
                if target >= block_count {
                    return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                        "a semantic CFG edge is outside the block table",
                    ));
                }
                successors[source]
                    .try_reserve(1)
                    .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
                predecessors[target]
                    .try_reserve(1)
                    .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
                successors[source].push(target);
                predecessors[target].push(source);
                Ok(())
            })?;
        }
        let graph = Self {
            entry,
            successors,
            predecessors,
        };
        let reachable = graph.reachable_avoiding(None, budget)?;
        if reachable.iter().any(|reachable| !reachable) {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                "the semantic CFG contains an unreachable block",
            ));
        }
        Ok(graph)
    }

    fn predecessors(&self, block: usize) -> Result<&[usize], SemanticU32InductionAnalysisErrorV1> {
        self.predecessors.get(block).map(Vec::as_slice).ok_or(
            SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                "a predecessor query is outside the block table",
            ),
        )
    }

    fn has_unique_predecessor(&self, block: usize, predecessor: usize) -> bool {
        matches!(self.predecessors.get(block).map(Vec::as_slice), Some([exact]) if *exact == predecessor)
    }

    fn dominates(
        &self,
        dominator: usize,
        block: usize,
        budget: &mut WorkBudgetV1,
    ) -> Result<bool, SemanticU32InductionAnalysisErrorV1> {
        if dominator >= self.successors.len() || block >= self.successors.len() {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidControlFlow(
                "a dominance query is outside the block table",
            ));
        }
        if dominator == block {
            return Ok(true);
        }
        Ok(!self.reachable_avoiding(Some(dominator), budget)?[block])
    }

    fn reachable_avoiding(
        &self,
        avoided: Option<usize>,
        budget: &mut WorkBudgetV1,
    ) -> Result<Vec<bool>, SemanticU32InductionAnalysisErrorV1> {
        let mut visited = fallible_filled_vec(self.successors.len(), false)?;
        if avoided == Some(self.entry) {
            return Ok(visited);
        }
        let mut pending = Vec::new();
        pending
            .try_reserve(self.successors.len())
            .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
        visited[self.entry] = true;
        pending.push(self.entry);
        while let Some(block) = pending.pop() {
            budget.charge(1)?;
            for successor in &self.successors[block] {
                budget.charge(1)?;
                if !visited[*successor] && avoided != Some(*successor) {
                    visited[*successor] = true;
                    pending.push(*successor);
                }
            }
        }
        Ok(visited)
    }
}

#[derive(Default)]
struct WorkBudgetV1 {
    used: usize,
    limit: usize,
}

impl WorkBudgetV1 {
    const fn new(limit: usize) -> Self {
        Self { used: 0, limit }
    }

    fn charge(&mut self, amount: usize) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
        self.used = self.used.checked_add(amount).ok_or(
            SemanticU32InductionAnalysisErrorV1::WorkLimit {
                actual: usize::MAX,
                limit: self.limit,
            },
        )?;
        if self.used > self.limit {
            return Err(SemanticU32InductionAnalysisErrorV1::WorkLimit {
                actual: self.used,
                limit: self.limit,
            });
        }
        Ok(())
    }
}

fn inspect_rvalue(
    value: &SemanticRvalueKindV1,
    hazards: &mut [bool],
    uses: &mut [usize],
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
    value.try_visit_operands(|operand| inspect_operand(operand, hazards, uses, budget))?;
    match value {
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => {
            mark_address_hazard(place, hazards, budget)
        }
        SemanticRvalueKindV1::Load(load) => mark_address_hazard(load.source(), hazards, budget),
        SemanticRvalueKindV1::Length(place) | SemanticRvalueKindV1::Discriminant(place) => {
            inspect_place(place, hazards, uses, budget)
        }
        SemanticRvalueKindV1::Use(_)
        | SemanticRvalueKindV1::Unary { .. }
        | SemanticRvalueKindV1::Binary { .. }
        | SemanticRvalueKindV1::CheckedBinary(_)
        | SemanticRvalueKindV1::UncheckedBinary(_)
        | SemanticRvalueKindV1::Cast { .. }
        | SemanticRvalueKindV1::Aggregate(_) => Ok(()),
    }
}

fn inspect_terminator(
    block: &SemanticBasicBlockV1,
    block_index: usize,
    definitions: &mut [DefinitionSummaryV1],
    hazards: &mut [bool],
    uses: &mut [usize],
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
    budget.charge(1)?;
    let site = DefinitionSiteV1 {
        block: block_index,
        position: DefinitionPositionV1::Terminator,
    };
    match block.terminator().kind() {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            inspect_operand(discriminant, hazards, uses, budget)?
        }
        SemanticTerminatorKindV1::Call(call) => {
            for argument in call.arguments() {
                inspect_operand(argument, hazards, uses, budget)?;
            }
            if let Some(destination) = call.destination() {
                record_definition(destination.place(), site, definitions, hazards, budget)?;
            }
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in call.arguments() {
                inspect_operand(argument, hazards, uses, budget)?;
            }
        }
        SemanticTerminatorKindV1::Drop { place, .. } => {
            inspect_place(place, hazards, uses, budget)?
        }
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            inspect_operand(condition, hazards, uses, budget)?;
            match message {
                SemanticAssertMessageV1::BoundsCheck { length, index }
                | SemanticAssertMessageV1::Overflow {
                    left: length,
                    right: index,
                    ..
                } => {
                    inspect_operand(length, hazards, uses, budget)?;
                    inspect_operand(index, hazards, uses, budget)?;
                }
                SemanticAssertMessageV1::DivisionByZero(operand)
                | SemanticAssertMessageV1::RemainderByZero(operand) => {
                    inspect_operand(operand, hazards, uses, budget)?
                }
                SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment,
                    found_alignment,
                } => {
                    inspect_operand(required_alignment, hazards, uses, budget)?;
                    inspect_operand(found_alignment, hazards, uses, budget)?;
                }
                SemanticAssertMessageV1::NullPointerDereference
                | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => {}
            }
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
    Ok(())
}

fn record_definition(
    place: &SemanticPlaceV1,
    site: DefinitionSiteV1,
    definitions: &mut [DefinitionSummaryV1],
    hazards: &mut [bool],
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
    budget.charge(place.projections().len().saturating_add(1))?;
    definitions
        .get_mut(place.local().index() as usize)
        .ok_or(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a semantic definition is outside the local table",
        ))?
        .record(site);
    if !place.projections().is_empty() {
        *local_slot_mut(
            hazards,
            place.local(),
            "a projected definition is outside the local table",
        )? = true;
    }
    Ok(())
}

fn mark_address_hazard(
    place: &SemanticPlaceV1,
    hazards: &mut [bool],
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
    budget.charge(place.projections().len().saturating_add(1))?;
    for projection in place.projections() {
        if let SemanticProjectionKindV1::Index(local) = projection.kind()
            && local.index() as usize >= hazards.len()
        {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                "a projection index is outside the local table",
            ));
        }
    }
    *local_slot_mut(
        hazards,
        place.local(),
        "an address-exposed local is outside the local table",
    )? = true;
    Ok(())
}

fn inspect_operand(
    operand: &SemanticOperandV1,
    hazards: &mut [bool],
    uses: &mut [usize],
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
    if let Some(place) = operand_place(operand) {
        inspect_place(place, hazards, uses, budget)?;
    } else {
        budget.charge(1)?;
    }
    Ok(())
}

fn inspect_place(
    place: &SemanticPlaceV1,
    hazards: &mut [bool],
    uses: &mut [usize],
    budget: &mut WorkBudgetV1,
) -> Result<(), SemanticU32InductionAnalysisErrorV1> {
    budget.charge(place.projections().len().saturating_add(1))?;
    if let Some(slot) = uses.get_mut(place.local().index() as usize) {
        *slot = slot.saturating_add(1);
    } else if !uses.is_empty() {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a semantic use is outside the local table",
        ));
    }
    if !place.projections().is_empty() {
        *local_slot_mut(
            hazards,
            place.local(),
            "a projected use is outside the local table",
        )? = true;
    }
    for projection in place.projections() {
        if let SemanticProjectionKindV1::Index(local) = projection.kind()
            && local.index() as usize >= hazards.len()
        {
            return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                "a projection index is outside the local table",
            ));
        }
    }
    Ok(())
}

fn operand_place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => Some(place),
        SemanticOperandV1::Constant(_) => None,
    }
}

fn exact_operand_place(operand: &SemanticOperandV1) -> Option<&SemanticPlaceV1> {
    operand_place(operand).filter(|place| place.projections().is_empty())
}

fn same_operand_value(left: &SemanticOperandV1, right: &SemanticOperandV1) -> bool {
    match (left, right) {
        (
            SemanticOperandV1::Copy(left) | SemanticOperandV1::Move(left),
            SemanticOperandV1::Copy(right) | SemanticOperandV1::Move(right),
        ) => left == right,
        (SemanticOperandV1::Constant(left), SemanticOperandV1::Constant(right)) => left == right,
        _ => false,
    }
}

fn field_operand_matches(
    operand: &SemanticOperandV1,
    local: SemanticLocalIdV1,
    field: u32,
    ty: SemanticTypeIdV1,
) -> bool {
    operand_place(operand).is_some_and(|place| {
        place.local() == local
            && place.ty() == ty
            && matches!(
                place.projections(),
                [projection]
                    if projection.kind() == SemanticProjectionKindV1::Field(field)
                        && projection.result_type() == ty
            )
    })
}

fn is_exact_destination(
    place: &SemanticPlaceV1,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
) -> bool {
    place.local() == local && place.ty() == ty && place.projections().is_empty()
}

fn is_exact_u32(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|decl| {
        matches!(
            decl.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        )
    })
}

fn is_exact_bool(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|decl| {
        matches!(
            decl.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
        )
    })
}

fn is_exact_checked_u32_tuple(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    u32_ty: SemanticTypeIdV1,
) -> bool {
    let Some(decl) = types.get(ty.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Tuple(tuple) = decl.shape() else {
        return false;
    };
    matches!(tuple.fields(), [value, boolean] if *value == u32_ty && is_exact_bool(types, *boolean))
}

fn bool_type_of_checked_tuple(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<SemanticTypeIdV1, SemanticU32InductionAnalysisErrorV1> {
    let Some(decl) = types.get(ty.index() as usize) else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked result type is outside the type table",
        ));
    };
    let SemanticTypeShapeV1::Tuple(tuple) = decl.shape() else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked result is not a tuple",
        ));
    };
    tuple
        .fields()
        .get(1)
        .copied()
        .ok_or(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a checked result tuple has no overflow field",
        ))
}

fn is_exact_u32_constant(
    operand: &SemanticOperandV1,
    ty: SemanticTypeIdV1,
    expected: u128,
) -> bool {
    matches!(
        operand,
        SemanticOperandV1::Constant(constant)
            if constant.ty() == ty
                && matches!(
                    constant.value(),
                    crate::semantic_mir_v1::SemanticConstantValueV1::Scalar(value)
                        if *value == SemanticScalarValueV1::new(expected, 4)
                            .expect("a four-byte u32 constant is valid")
                )
    )
}

fn definition(
    inventory: &SemanticInventoryV1,
    local: SemanticLocalIdV1,
) -> Result<DefinitionSummaryV1, SemanticU32InductionAnalysisErrorV1> {
    inventory
        .definitions
        .get(local.index() as usize)
        .copied()
        .ok_or(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a definition query is outside the local table",
        ))
}

fn use_count(
    inventory: &SemanticInventoryV1,
    local: SemanticLocalIdV1,
) -> Result<usize, SemanticU32InductionAnalysisErrorV1> {
    inventory
        .use_counts
        .get(local.index() as usize)
        .copied()
        .ok_or(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a use query is outside the local table",
        ))
}

fn local<T: Copy>(
    values: &[T],
    local: SemanticLocalIdV1,
) -> Result<T, SemanticU32InductionAnalysisErrorV1> {
    values.get(local.index() as usize).copied().ok_or(
        SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a semantic local query is outside the local table",
        ),
    )
}

fn local_slot_mut<'a, T>(
    values: &'a mut [T],
    local: SemanticLocalIdV1,
    detail: &'static str,
) -> Result<&'a mut T, SemanticU32InductionAnalysisErrorV1> {
    values
        .get_mut(local.index() as usize)
        .ok_or(SemanticU32InductionAnalysisErrorV1::InvalidModel(detail))
}

fn place_binding(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    local: SemanticLocalIdV1,
) -> Result<SemanticU32InductionPlaceBindingV1, SemanticU32InductionAnalysisErrorV1> {
    let decl = function.locals().get(local.index() as usize).ok_or(
        SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a certificate local is outside the local table",
        ),
    )?;
    let ty = decl.ty();
    let ty_decl =
        types
            .get(ty.index() as usize)
            .ok_or(SemanticU32InductionAnalysisErrorV1::InvalidModel(
                "a certificate type is outside the type table",
            ))?;
    Ok(SemanticU32InductionPlaceBindingV1 {
        local,
        local_identity: decl.identity(),
        ty,
        type_identity: ty_decl.identity(),
    })
}

fn block_site(
    block: usize,
    declaration: &SemanticBasicBlockV1,
) -> Result<SemanticU32InductionBlockSiteV1, SemanticU32InductionAnalysisErrorV1> {
    let block = u32::try_from(block).map_err(|_| {
        SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a certificate block index does not fit the semantic identity",
        )
    })?;
    Ok(SemanticU32InductionBlockSiteV1 {
        block: SemanticBlockIdV1::from_index(block),
        identity: declaration.identity(),
    })
}

fn statement_site(
    block: usize,
    declaration: &SemanticBasicBlockV1,
    site: DefinitionSiteV1,
) -> Result<SemanticU32InductionStatementSiteV1, SemanticU32InductionAnalysisErrorV1> {
    if site.block != block {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a certificate statement belongs to a different block",
        ));
    }
    let DefinitionPositionV1::Statement(statement) = site.position else {
        return Err(SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a certificate statement points at a terminator",
        ));
    };
    let statement = u32::try_from(statement).map_err(|_| {
        SemanticU32InductionAnalysisErrorV1::InvalidModel(
            "a certificate statement index does not fit the semantic identity",
        )
    })?;
    Ok(SemanticU32InductionStatementSiteV1 {
        block: block_site(block, declaration)?,
        statement,
    })
}

fn fallible_filled_vec<T: Clone>(
    length: usize,
    value: T,
) -> Result<Vec<T>, SemanticU32InductionAnalysisErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
    result.resize(length, value);
    Ok(result)
}

fn fallible_nested_vec<T>(
    length: usize,
) -> Result<Vec<Vec<T>>, SemanticU32InductionAnalysisErrorV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| SemanticU32InductionAnalysisErrorV1::Storage)?;
    result.resize_with(length, Vec::new);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_mir_v1::{
        InertSemanticMirRequestV1, SemanticAbiExtensionV1, SemanticAbiIdentityV1,
        SemanticAbiPassModeV1, SemanticAbiRegularAttributesV1, SemanticAbiValueAttributesV1,
        SemanticAbiValueV1, SemanticAggregateLayoutV1, SemanticAggregateTypeV1,
        SemanticBackendPrimitiveV1, SemanticBackendReprV1, SemanticBackendScalarV1,
        SemanticCanonAbiV1, SemanticConstGenericArgumentsIdentityV1, SemanticConstantV1,
        SemanticConstantValueV1, SemanticControlFlowEdgeV1, SemanticEdgeRoleV1,
        SemanticFunctionAbiV1, SemanticFunctionRoleV1, SemanticGenericTypeArgumentsIdentityV1,
        SemanticItemDefinitionIdentityV1, SemanticLayoutIdentityV1, SemanticLocalDeclV1,
        SemanticMirLimitsV1, SemanticMonomorphizationIdentityV1, SemanticPaddingV1,
        SemanticRvalueV1, SemanticScalarValidityRangeV1, SemanticSourceProvenanceV1,
        SemanticStatementV1, SemanticSwitchTargetV1, SemanticSwitchTargetsV1,
        SemanticTargetDataLayoutV1, SemanticTerminatorV1, SemanticTypeIdentityV1,
        SemanticTypeLayoutV1,
    };

    const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const CHECKED_U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const BOUND: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(1);
    const INDUCTION: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(2);
    const PREDICATE: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(3);
    const CHECKED_RESULT: SemanticLocalIdV1 = SemanticLocalIdV1::from_index(4);

    #[derive(Clone, Copy)]
    struct Shape {
        step: u128,
        expected_overflow: bool,
        bound_is_argument: bool,
        extra_induction_definition: bool,
        alias_induction: bool,
        guard_snapshot: bool,
        guard_snapshot_extra_use: bool,
        mutate_assert_operands: bool,
        identity_seed: u8,
    }

    impl Default for Shape {
        fn default() -> Self {
            Self {
                step: 1,
                expected_overflow: false,
                bound_is_argument: true,
                extra_induction_definition: false,
                alias_induction: false,
                guard_snapshot: false,
                guard_snapshot_extra_use: false,
                mutate_assert_operands: false,
                identity_seed: 0,
            }
        }
    }

    fn identity(seed: u8, tag: u8) -> [u8; 32] {
        [seed.wrapping_add(tag); 32]
    }

    fn scalar_layout(
        size: u64,
        alignment: u64,
        primitive: SemanticBackendPrimitiveV1,
        maximum: u128,
    ) -> SemanticTypeLayoutV1 {
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(size),
            alignment,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                primitive,
                SemanticScalarValidityRangeV1::new(0, maximum),
            )),
            false,
        )
        .unwrap()
    }

    fn types(seed: u8) -> Vec<SemanticTypeDeclV1> {
        vec![
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(identity(seed, 1)),
                SemanticLayoutIdentityV1::from_sha256(identity(seed, 2)),
                scalar_layout(
                    4,
                    4,
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    u128::from(u32::MAX),
                ),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                }),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(identity(seed, 3)),
                SemanticLayoutIdentityV1::from_sha256(identity(seed, 4)),
                scalar_layout(1, 1, SemanticBackendPrimitiveV1::integer(false, 8, 1), 1),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(identity(seed, 5)),
                SemanticLayoutIdentityV1::from_sha256(identity(seed, 6)),
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    4,
                    SemanticAggregateLayoutV1::new(
                        vec![0, 4],
                        vec![SemanticPaddingV1::new(5, 3).unwrap()],
                    )
                    .unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, BOOL]).unwrap()),
            ),
        ]
    }

    fn direct_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    }

    fn place(local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(local, vec![], ty).unwrap()
    }

    fn field(local: SemanticLocalIdV1, field: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(
            local,
            vec![
                crate::semantic_mir_v1::SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Field(field),
                    ty,
                )
                .unwrap(),
            ],
            ty,
        )
        .unwrap()
    }

    fn copy(local: SemanticLocalIdV1, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(place(local, ty))
    }

    fn constant(value: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    }

    fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
    }

    fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
        SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
    }

    fn assignment(
        destination: SemanticPlaceV1,
        ty: SemanticTypeIdV1,
        kind: SemanticRvalueKindV1,
    ) -> SemanticStatementV1 {
        statement(SemanticStatementKindV1::Assign(
            crate::semantic_mir_v1::SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, kind),
            ),
        ))
    }

    fn block(
        seed: u8,
        tag: u8,
        statements: Vec<SemanticStatementV1>,
        terminator: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256(identity(seed, tag)),
            SemanticSourceProvenanceV1::unavailable(),
            statements,
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap()
    }

    fn admitted(shape: Shape) -> AdmittedInertSemanticMirV1 {
        let seed = shape.identity_seed;
        let local = |tag, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(identity(seed, tag)),
                ty,
                role,
                SemanticSourceProvenanceV1::unavailable(),
            )
        };
        let mut locals = vec![
            local(20, U32, SemanticLocalRoleV1::Return),
            local(
                21,
                U32,
                if shape.bound_is_argument {
                    SemanticLocalRoleV1::Argument(0)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
            ),
            local(22, U32, SemanticLocalRoleV1::Temporary),
            local(23, BOOL, SemanticLocalRoleV1::Temporary),
            local(24, CHECKED_U32, SemanticLocalRoleV1::Temporary),
        ];
        let alias =
            if shape.alias_induction || shape.guard_snapshot || shape.guard_snapshot_extra_use {
                let alias = SemanticLocalIdV1::from_index(locals.len() as u32);
                locals.push(local(25, U32, SemanticLocalRoleV1::Temporary));
                Some(alias)
            } else {
                None
            };
        let snapshot_sink = if shape.guard_snapshot_extra_use {
            let sink = SemanticLocalIdV1::from_index(locals.len() as u32);
            locals.push(local(26, U32, SemanticLocalRoleV1::Temporary));
            Some(sink)
        } else {
            None
        };

        let mut preheader = vec![assignment(
            place(INDUCTION, U32),
            U32,
            SemanticRvalueKindV1::Use(constant(0)),
        )];
        if shape.extra_induction_definition {
            preheader.push(assignment(
                place(INDUCTION, U32),
                U32,
                SemanticRvalueKindV1::Use(constant(0)),
            ));
        }
        if shape.alias_induction {
            let alias = alias.expect("stale alias local");
            preheader.push(assignment(
                place(alias, U32),
                U32,
                SemanticRvalueKindV1::Use(copy(INDUCTION, U32)),
            ));
        }
        let mut header = Vec::new();
        if shape.guard_snapshot || shape.guard_snapshot_extra_use {
            let alias = alias.expect("header snapshot local");
            header.push(assignment(
                place(alias, U32),
                U32,
                SemanticRvalueKindV1::Use(copy(INDUCTION, U32)),
            ));
            if let Some(sink) = snapshot_sink {
                header.push(assignment(
                    place(sink, U32),
                    U32,
                    SemanticRvalueKindV1::Use(copy(alias, U32)),
                ));
            }
        }
        let guard_induction = alias.filter(|_| {
            shape.alias_induction || shape.guard_snapshot || shape.guard_snapshot_extra_use
        });
        let guard = assignment(
            place(PREDICATE, BOOL),
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: copy(guard_induction.unwrap_or(INDUCTION), U32),
                right: copy(BOUND, U32),
            },
        );
        header.push(guard);
        let checked = assignment(
            place(CHECKED_RESULT, CHECKED_U32),
            CHECKED_U32,
            SemanticRvalueKindV1::CheckedBinary(
                crate::semantic_mir_v1::SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    copy(INDUCTION, U32),
                    constant(shape.step),
                ),
            ),
        );
        let asserted_right = if shape.mutate_assert_operands {
            constant(shape.step.saturating_add(1))
        } else {
            constant(shape.step)
        };
        let blocks = vec![
            block(
                seed,
                30,
                preheader,
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(
                seed,
                31,
                header,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: copy(PREDICATE, BOOL),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 4),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(
                seed,
                32,
                vec![checked],
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Copy(field(CHECKED_RESULT, 1, BOOL)),
                    expected: shape.expected_overflow,
                    message: SemanticAssertMessageV1::Overflow {
                        operation: SemanticBinaryOpV1::Add,
                        left: copy(INDUCTION, U32),
                        right: asserted_right,
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(
                seed,
                33,
                vec![assignment(
                    place(INDUCTION, U32),
                    U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(
                        CHECKED_RESULT,
                        0,
                        U32,
                    ))),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
            ),
            block(seed, 34, vec![], SemanticTerminatorKindV1::Return),
        ];
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(identity(seed, 40)),
            SemanticLayoutIdentityV1::from_sha256(identity(seed, 41)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct_value(U32)],
            direct_value(U32),
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(identity(seed, 42)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(identity(seed, 43)),
            SemanticMonomorphizationIdentityV1::from_sha256(identity(seed, 44)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(identity(seed, 45)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(identity(seed, 46)),
            SemanticSourceProvenanceV1::unavailable(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(identity(
                seed, 47,
            ))),
            types(seed),
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap()
    }

    fn report(admitted: &AdmittedInertSemanticMirV1) -> SemanticU32InductionNoOverflowReportV1 {
        analyze_semantic_u32_induction_no_overflow_v1(admitted, SemanticFunctionIdV1::from_index(0))
            .unwrap()
    }

    #[test]
    fn exact_guarded_checked_u32_induction_produces_one_bound_certificate() {
        let admitted = admitted(Shape::default());
        let report = report(&admitted);
        assert_eq!(report.checked_additions_examined(), 1);
        let [certificate] = report.certificates() else {
            panic!("expected one exact no-overflow certificate");
        };
        assert_eq!(report.semantic_mir_sha256(), admitted.semantic_sha256());
        assert_eq!(report.function(), SemanticFunctionIdV1::from_index(0));
        assert_eq!(
            certificate.semantic_mir_sha256(),
            admitted.semantic_sha256()
        );
        assert_eq!(certificate.function(), SemanticFunctionIdV1::from_index(0));
        assert_eq!(certificate.induction().local(), INDUCTION);
        assert_eq!(certificate.guard_induction().local(), INDUCTION);
        assert_eq!(certificate.guard_induction_snapshot(), None);
        assert_eq!(certificate.bound().local(), BOUND);
        assert_eq!(certificate.predicate().local(), PREDICATE);
        assert_eq!(certificate.checked_result().local(), CHECKED_RESULT);
        assert_eq!(certificate.preheader().block().index(), 0);
        assert_eq!(certificate.header().block().index(), 1);
        assert_eq!(certificate.body_entry().block().index(), 2);
        assert_eq!(certificate.exit().block().index(), 4);
        assert_eq!(certificate.initialization().statement(), 0);
        assert_eq!(certificate.guard().statement(), 0);
        assert_eq!(certificate.checked_addition().statement(), 0);
        assert_eq!(certificate.update().statement(), 0);
        assert!(certificate.establishes_semantic_no_overflow());
        assert!(!certificate.grants_authority());
        assert!(!certificate.authorizes_compiler_transform());
        assert!(!report.grants_authority());
        assert!(!report.authorizes_compiler_transform());
    }

    #[test]
    fn exact_single_use_header_snapshot_preserves_the_induction_fact() {
        let admitted = admitted(Shape {
            guard_snapshot: true,
            ..Shape::default()
        });
        let report = report(&admitted);
        let [certificate] = report.certificates() else {
            panic!("expected one header-snapshot no-overflow certificate");
        };
        assert_eq!(certificate.induction().local(), INDUCTION);
        assert_ne!(certificate.guard_induction().local(), INDUCTION);
        assert_eq!(
            certificate
                .guard_induction_snapshot()
                .expect("exact header snapshot site")
                .statement(),
            0
        );
        assert_eq!(certificate.guard().statement(), 1);
    }

    #[test]
    fn unsupported_or_hostile_shapes_never_produce_a_certificate() {
        for shape in [
            Shape {
                step: 2,
                ..Shape::default()
            },
            Shape {
                expected_overflow: true,
                ..Shape::default()
            },
            Shape {
                extra_induction_definition: true,
                ..Shape::default()
            },
            Shape {
                alias_induction: true,
                ..Shape::default()
            },
            Shape {
                guard_snapshot_extra_use: true,
                ..Shape::default()
            },
            Shape {
                mutate_assert_operands: true,
                ..Shape::default()
            },
        ] {
            let admitted = admitted(shape);
            let report = report(&admitted);
            assert_eq!(report.checked_additions_examined(), 1);
            assert!(report.certificates().is_empty());
        }
    }

    #[test]
    fn canonical_round_trip_rederives_the_same_certificate() {
        let admitted = admitted(Shape::default());
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            admitted.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.semantic_sha256(), admitted.semantic_sha256());
        assert_eq!(report(&decoded), report(&admitted));
    }

    #[test]
    fn identity_substitution_changes_every_bound_certificate_identity() {
        let first = admitted(Shape::default());
        let second = admitted(Shape {
            identity_seed: 64,
            ..Shape::default()
        });
        let first = report(&first).certificates()[0];
        let second = report(&second).certificates()[0];
        assert_ne!(first.function_identity(), second.function_identity());
        assert_ne!(
            first.induction().local_identity(),
            second.induction().local_identity()
        );
        assert_ne!(
            first.induction().type_identity(),
            second.induction().type_identity()
        );
        assert_ne!(first.header().identity(), second.header().identity());
        assert_ne!(
            first.checked_addition().block().identity(),
            second.checked_addition().block().identity()
        );
    }

    #[test]
    fn deterministic_work_and_certificate_limits_fail_closed_at_the_boundary() {
        let admitted = admitted(Shape::default());
        let full = report(&admitted);
        let exact = analyze_semantic_u32_induction_no_overflow_with_limits_v1(
            &admitted,
            SemanticFunctionIdV1::from_index(0),
            SemanticU32InductionAnalysisLimitsV1::new(full.work_units(), 1),
        )
        .unwrap();
        assert_eq!(exact, full);
        assert!(matches!(
            analyze_semantic_u32_induction_no_overflow_with_limits_v1(
                &admitted,
                SemanticFunctionIdV1::from_index(0),
                SemanticU32InductionAnalysisLimitsV1::new(full.work_units() - 1, 1),
            ),
            Err(SemanticU32InductionAnalysisErrorV1::WorkLimit { .. })
        ));
        assert_eq!(
            analyze_semantic_u32_induction_no_overflow_with_limits_v1(
                &admitted,
                SemanticFunctionIdV1::from_index(0),
                SemanticU32InductionAnalysisLimitsV1::new(full.work_units(), 0),
            ),
            Err(SemanticU32InductionAnalysisErrorV1::CertificateLimit {
                actual: 1,
                limit: 0,
            })
        );
        assert!(matches!(
            analyze_semantic_u32_induction_no_overflow_with_limits_v1(
                &admitted,
                SemanticFunctionIdV1::from_index(0),
                SemanticU32InductionAnalysisLimitsV1::new(
                    MAX_SEMANTIC_U32_INDUCTION_WORK_V1 + 1,
                    1,
                ),
            ),
            Err(SemanticU32InductionAnalysisErrorV1::InvalidLimits { .. })
        ));
    }
}
