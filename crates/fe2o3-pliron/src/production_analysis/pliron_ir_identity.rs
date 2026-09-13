//! Bounded structural identities for the closed production ranked PLIRON subset.
//!
//! The transcript alpha-numbers blocks and SSA values by deterministic
//! region/block/operation order. It intentionally excludes pointer identities,
//! source locations, and optional block/SSA display labels. Every semantic
//! dictionary attribute is retained; `builtin.debug_info` is display metadata
//! and is excluded. Equality compares the private canonical bytes, not only the
//! SHA-256 label.

use std::{
    collections::HashMap,
    fmt::{self, Write as _},
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use pliron::{
    attribute::{AttrObj, AttributeDict},
    basic_block::BasicBlock,
    builtin::{
        attributes::TypeAttr,
        op_interfaces::OneRegionInterface,
        ops::FuncOp,
        type_interfaces::FunctionTypeInterface,
        types::{FP16Type, FP32Type, FP64Type, FunctionType, IntegerType, UnitType},
    },
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::Operation,
    printable::Printable,
    r#type::{Type, TypeHandle, Typed},
    value::Value,
};
use sha2::{Digest, Sha256};

#[cfg(test)]
use pliron::operation::verify_operation;

use dialect_kernel::{
    AllocationEffectOp, GFX950_TRANSPOSE_FP4_WORKGROUP_ALLOCATION_ORIGIN_V1,
    GFX950_TRANSPOSE_FP4_WORKGROUP_NOALIAS_CLASS_V1,
    GFX950_TRANSPOSE_FP8_WORKGROUP_ALLOCATION_ORIGIN_V1,
    GFX950_TRANSPOSE_FP8_WORKGROUP_NOALIAS_CLASS_V1, IndexLessThanBranchArgsOp, IndexType,
    MemorySpaceAttr, OwnershipContractOp, PipelineCreateOp, PipelineEventOp, PipelineType,
    RankedAccessOp, RankedViewOp, RankedViewType, SemanticScalarType,
    is_checked_access_capability_type,
};
use dialect_proof::{EvidenceRefType, ObligationRefType};

use crate::production_analysis::pliron_effect_refinement::is_effect_refinement_contract_v1;
use crate::production_analysis::pliron_pass_contract::{
    BoundedPlironIdentityCaptureV1, IdentityCaptureFailureV1, IdentityComparisonFailureV1,
    MutationEpochCaptureFailureV1, PlironStructuralIdentityLabelV1,
    PlironStructuralIdentityProviderV1,
};
use crate::production_analysis::pliron_ranked_bounds::is_production_ranked_operation_v1;
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitsV1,
    ProductionAnalysisResourcePhaseV1, ProductionAnalysisResourceUpperBoundV1,
};
use crate::production_analysis::pliron_semantic_refinement::{
    is_semantic_refinement_contract_v1, is_semantic_refinement_definition_v1,
};

pub const MAX_PLIRON_IDENTITY_BLOCKS_V1: usize = 1_024;
pub const MAX_PLIRON_IDENTITY_OPERATIONS_V1: usize = 65_536;
pub const MAX_PLIRON_IDENTITY_VALUES_V1: usize = 131_072;
pub const MAX_PLIRON_IDENTITY_OPERANDS_V1: usize = 524_288;
pub const MAX_PLIRON_IDENTITY_SUCCESSORS_V1: usize = 16_384;
pub const MAX_PLIRON_IDENTITY_ATTRIBUTES_V1: usize = 262_144;
pub const MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1: usize = 65_536;
pub const MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1: usize = 16 * 1_024 * 1_024;
pub const MAX_PLIRON_IDENTITY_TYPE_NESTING_V1: usize = 64;

const TRANSCRIPT_MAGIC_V1: &[u8] = b"fe2o3.pliron.ranked.structural-identity.v1";
const MAX_DIAGNOSTIC_DETAIL_CHARS_V1: usize = 240;
const MAX_IDENTIFIER_BYTES_V1: usize = 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironPreserveLocationV1 {
    Function,
    Block {
        block: usize,
    },
    Operation {
        block: usize,
        operation: usize,
        name: String,
    },
}

impl fmt::Display for PlironPreserveLocationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Function => formatter.write_str("function"),
            Self::Block { block } => write!(formatter, "block {block}"),
            Self::Operation {
                block,
                operation,
                name,
            } => write!(formatter, "block {block} op {operation} ({name})"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironIrIdentityErrorV1 {
    UnsupportedRoot {
        operation: String,
        detail: &'static str,
    },
    UnsupportedOperation {
        location: PlironPreserveLocationV1,
        detail: &'static str,
    },
    UnsupportedAttribute {
        location: PlironPreserveLocationV1,
        attribute: String,
    },
    UnsupportedType {
        location: PlironPreserveLocationV1,
        ty: String,
    },
    ResourceLimitExceeded {
        location: PlironPreserveLocationV1,
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    ExternalOperand {
        location: PlironPreserveLocationV1,
        operand: usize,
        value: String,
    },
    ExternalSuccessor {
        location: PlironPreserveLocationV1,
        successor: usize,
    },
    StructuralVerificationFailed {
        detail: String,
    },
    RenderingFailed {
        location: PlironPreserveLocationV1,
        entity: &'static str,
        detail: &'static str,
    },
    TraversalPanicked,
}

impl PlironIrIdentityErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::StructuralVerificationFailed { .. } => "FE2O3-PRESERVE-000",
            Self::UnsupportedRoot { .. }
            | Self::UnsupportedOperation { .. }
            | Self::UnsupportedAttribute { .. }
            | Self::UnsupportedType { .. } => "FE2O3-PRESERVE-001",
            Self::ResourceLimitExceeded { .. } => "FE2O3-PRESERVE-002",
            Self::ExternalOperand { .. } | Self::ExternalSuccessor { .. } => "FE2O3-PRESERVE-003",
            Self::RenderingFailed { .. } => "FE2O3-PRESERVE-004",
            Self::TraversalPanicked => "FE2O3-PRESERVE-005",
        }
    }
}

impl fmt::Display for PlironIrIdentityErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedRoot { operation, detail } => write!(
                formatter,
                "error[FE2O3-PRESERVE-001]: unsupported identity root {operation}: {detail}; help: preserve one verified builtin.func in the production ranked PLIRON subset"
            ),
            Self::UnsupportedOperation { location, detail } => write!(
                formatter,
                "error[FE2O3-PRESERVE-001]: unsupported structure at {location}: {detail}; help: lower the construct into the closed production ranked PLIRON subset before preservation checking"
            ),
            Self::UnsupportedAttribute {
                location,
                attribute,
            } => write!(
                formatter,
                "error[FE2O3-PRESERVE-001]: unsupported attribute {attribute} at {location}; help: lower metadata into an attribute with a closed production canonical encoding"
            ),
            Self::UnsupportedType { location, ty } => write!(
                formatter,
                "error[FE2O3-PRESERVE-001]: unsupported type {ty} at {location}; help: lower values into a type with a closed production canonical encoding"
            ),
            Self::ResourceLimitExceeded {
                location,
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "error[FE2O3-PRESERVE-002]: {resource} count {actual} at {location} exceeds identity limit {limit}; help: split or simplify the function before preservation checking"
            ),
            Self::ExternalOperand {
                location,
                operand,
                value,
            } => write!(
                formatter,
                "error[FE2O3-PRESERVE-003]: operand {operand} at {location} references external value {value}; help: make every operand a block argument or result in this function"
            ),
            Self::ExternalSuccessor {
                location,
                successor,
            } => write!(
                formatter,
                "error[FE2O3-PRESERVE-003]: successor {successor} at {location} leaves the function region; help: target a block owned by this function"
            ),
            Self::StructuralVerificationFailed { detail } => write!(
                formatter,
                "error[FE2O3-PRESERVE-000]: PLIRON structural verification failed before identity construction: {detail}; help: repair the malformed operation, type, attribute, region, or CFG"
            ),
            Self::RenderingFailed {
                location,
                entity,
                detail,
            } => write!(
                formatter,
                "error[FE2O3-PRESERVE-004]: cannot render {entity} at {location}: {detail}; help: use a registered deterministic production type or attribute"
            ),
            Self::TraversalPanicked => formatter.write_str(
                "error[FE2O3-PRESERVE-005]: PLIRON identity traversal panicked and was rejected; help: repair the malformed graph before preservation checking",
            ),
        }
    }
}

impl std::error::Error for PlironIrIdentityErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironPreserveSnapshotSideV1 {
    Before,
    After,
}

impl fmt::Display for PlironPreserveSnapshotSideV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Before => "before",
            Self::After => "after",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironIrPreservationErrorV1 {
    SnapshotFailed {
        side: PlironPreserveSnapshotSideV1,
        source: PlironIrIdentityErrorV1,
    },
    IdentityChanged(Box<PlironIrIdentityChangeV1>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironIrIdentityChangeV1 {
    location: PlironPreserveLocationV1,
    component: &'static str,
    before: String,
    after: String,
    before_sha256: [u8; 32],
    after_sha256: [u8; 32],
}

impl PlironIrIdentityChangeV1 {
    pub const fn location(&self) -> &PlironPreserveLocationV1 {
        &self.location
    }

    pub const fn component(&self) -> &'static str {
        self.component
    }

    pub fn before(&self) -> &str {
        &self.before
    }

    pub fn after(&self) -> &str {
        &self.after
    }

    pub const fn before_sha256(&self) -> &[u8; 32] {
        &self.before_sha256
    }

    pub const fn after_sha256(&self) -> &[u8; 32] {
        &self.after_sha256
    }
}

impl fmt::Display for PlironIrPreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SnapshotFailed { side, source } => {
                write!(formatter, "{source}; {side} snapshot was not constructed")
            }
            Self::IdentityChanged(change) => write!(
                formatter,
                "error[FE2O3-PRESERVE-010]: verified PLIRON structure changed at {}, component {}: before `{}`, after `{}`; before identity {}, after identity {}; help: preserve the exact ranked IR structure or re-run correctness verification for the transformed function",
                change.location,
                change.component,
                change.before,
                change.after,
                hex_digest(&change.before_sha256),
                hex_digest(&change.after_sha256),
            ),
        }
    }
}

impl std::error::Error for PlironIrPreservationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SnapshotFailed { source, .. } => Some(source),
            Self::IdentityChanged(_) => None,
        }
    }
}

/// Exact canonical structure for one verified production ranked PLIRON function.
///
/// The digest is a compact label. [`Self::exactly_matches`] compares the
/// retained canonical bytes so a digest collision cannot authorize equality.
#[derive(Clone, Debug)]
pub struct PlironIrStructuralIdentityV1 {
    sha256: [u8; 32],
    canonical: Vec<u8>,
    blocks: usize,
    operations: usize,
    values: usize,
}

impl PartialEq for PlironIrStructuralIdentityV1 {
    fn eq(&self, other: &Self) -> bool {
        self.canonical == other.canonical
    }
}

impl Eq for PlironIrStructuralIdentityV1 {}

impl PlironIrStructuralIdentityV1 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn canonical_bytes_len(&self) -> usize {
        self.canonical.len()
    }

    pub const fn block_count(&self) -> usize {
        self.blocks
    }

    pub const fn operation_count(&self) -> usize {
        self.operations
    }

    pub const fn value_count(&self) -> usize {
        self.values
    }

    pub fn exactly_matches(&self, other: &Self) -> bool {
        self.canonical == other.canonical
    }

    pub const fn grants_operational_semantics_or_refinement_authority(&self) -> bool {
        false
    }
}

struct IdentityRecordV1 {
    range: Range<usize>,
    location: PlironPreserveLocationV1,
    component: &'static str,
    summary: String,
}

pub(crate) struct BuiltIdentityV1 {
    identity: PlironIrStructuralIdentityV1,
    records: Vec<IdentityRecordV1>,
    input_census: ProductionAnalysisInputCensusV1,
}

pub(crate) struct LivePlironStructuralIdentityProviderV1<'a> {
    context: &'a Context,
    function: &'a FuncOp,
}

impl<'a> LivePlironStructuralIdentityProviderV1<'a> {
    pub(crate) const fn new(context: &'a Context, function: &'a FuncOp) -> Self {
        Self { context, function }
    }

    pub(crate) const fn scoped_endpoints_v1(&self) -> (&Context, &FuncOp) {
        (self.context, self.function)
    }
}

impl PlironStructuralIdentityProviderV1 for LivePlironStructuralIdentityProviderV1<'_> {
    type Snapshot = BuiltIdentityV1;

    fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
        self.context
            .ir_mutation_attempt_epoch()
            .map(|epoch| epoch.value())
            .map_err(|error| MutationEpochCaptureFailureV1::new(error.to_string()))
    }

    fn capture_with_resource_limits_v1(
        &mut self,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<BoundedPlironIdentityCaptureV1<Self::Snapshot>, IdentityCaptureFailureV1> {
        let (snapshot, input_census, resource_upper_bound) =
            build_identity_caught_with_resource_limits_v1(self.context, self.function, limits)?;
        Ok(BoundedPlironIdentityCaptureV1 {
            snapshot,
            input_census,
            resource_upper_bound,
        })
    }

    fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
        PlironStructuralIdentityLabelV1::new(
            snapshot.identity.sha256,
            snapshot.identity.canonical.len(),
        )
    }

    fn require_exact_identity(
        &self,
        expected: &Self::Snapshot,
        observed: &Self::Snapshot,
    ) -> Result<(), IdentityComparisonFailureV1> {
        if expected.identity.exactly_matches(&observed.identity) {
            return Ok(());
        }
        let (location, component, before, after) = first_record_difference(expected, observed);
        Err(IdentityComparisonFailureV1::new(
            "FE2O3-PRESERVE-010",
            format!(
                "verified PLIRON structure changed at {location}, component {component}: before `{before}`, after `{after}`; before identity {}, after identity {}; help: preserve the exact ranked IR structure or re-run correctness verification for the transformed function",
                hex_digest(&expected.identity.sha256),
                hex_digest(&observed.identity.sha256),
            ),
        ))
    }

    fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]> {
        Arc::from(snapshot.identity.canonical)
    }
}

struct PrescanV1 {
    blocks: Vec<Ptr<BasicBlock>>,
    operations: Vec<Vec<Ptr<Operation>>>,
    values: usize,
    operands: usize,
    successors: usize,
    block_arguments: usize,
    attributes: usize,
    type_nodes: usize,
    max_operation_arity: usize,
    max_successor_arity: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct IdentityPreflightCensusV1 {
    structural_work: usize,
    structural_storage: usize,
    rendered_entities: usize,
    type_roots: usize,
    records: usize,
    max_semantic_attributes_per_dictionary: usize,
    native_switch_verification_work: usize,
    native_switch_verification_scratch: usize,
}

/// Constructs a bounded, deterministic identity for live PLIRON.
pub(crate) fn derive_pliron_ir_structural_identity_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironIrStructuralIdentityV1, PlironIrIdentityErrorV1> {
    build_identity_caught(context, function).map(|built| built.identity)
}

/// Requires byte-exact structural preservation between two verified snapshots.
///
/// This is not an operational equivalence or refinement theorem. A changed
/// function must re-enter the ordinary correctness pipeline.
#[cfg(test)]
pub(crate) fn require_pliron_ir_structural_identity_preserved_v1(
    before_context: &Context,
    before: &FuncOp,
    after_context: &Context,
    after: &FuncOp,
) -> Result<PlironIrStructuralIdentityV1, PlironIrPreservationErrorV1> {
    let before = build_identity_caught(before_context, before).map_err(|source| {
        PlironIrPreservationErrorV1::SnapshotFailed {
            side: PlironPreserveSnapshotSideV1::Before,
            source,
        }
    })?;
    let after = build_identity_caught(after_context, after).map_err(|source| {
        PlironIrPreservationErrorV1::SnapshotFailed {
            side: PlironPreserveSnapshotSideV1::After,
            source,
        }
    })?;
    if before.identity.exactly_matches(&after.identity) {
        return Ok(after.identity);
    }
    let (location, component, before_summary, after_summary) =
        first_record_difference(&before, &after);
    Err(PlironIrPreservationErrorV1::IdentityChanged(Box::new(
        PlironIrIdentityChangeV1 {
            location,
            component,
            before: before_summary,
            after: after_summary,
            before_sha256: before.identity.sha256,
            after_sha256: after.identity.sha256,
        },
    )))
}

fn build_identity_caught(
    context: &Context,
    function: &FuncOp,
) -> Result<BuiltIdentityV1, PlironIrIdentityErrorV1> {
    match catch_unwind(AssertUnwindSafe(|| {
        build_identity(
            context,
            function,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
    })) {
        Err(_) => Err(PlironIrIdentityErrorV1::TraversalPanicked),
        Ok(Ok((built, _))) => Ok(built),
        Ok(Err(BuildIdentityFailureV1::Identity(error))) => Err(error),
        Ok(Err(BuildIdentityFailureV1::ResourceLimit(_))) => {
            Err(canonical_bytes_resource_error(usize::MAX))
        }
    }
}

fn build_identity_caught_with_resource_limits_v1(
    context: &Context,
    function: &FuncOp,
    limits: ProductionAnalysisResourceLimitsV1,
) -> Result<
    (
        BuiltIdentityV1,
        ProductionAnalysisInputCensusV1,
        ProductionAnalysisResourceUpperBoundV1,
    ),
    IdentityCaptureFailureV1,
> {
    let (built, upper_bound) = match catch_unwind(AssertUnwindSafe(|| {
        build_identity(context, function, limits)
    })) {
        Err(_) => {
            return Err(IdentityCaptureFailureV1::Unavailable {
                source_code: PlironIrIdentityErrorV1::TraversalPanicked.code(),
                detail: PlironIrIdentityErrorV1::TraversalPanicked.to_string(),
            });
        }
        Ok(Ok(result)) => result,
        Ok(Err(BuildIdentityFailureV1::Identity(error))) => {
            return Err(IdentityCaptureFailureV1::Unavailable {
                source_code: error.code(),
                detail: error.to_string(),
            });
        }
        Ok(Err(BuildIdentityFailureV1::ResourceLimit(error))) => {
            return Err(IdentityCaptureFailureV1::ResourceLimit(error));
        }
    };
    let census = built.input_census;
    Ok((built, census, upper_bound))
}

enum BuildIdentityFailureV1 {
    Identity(PlironIrIdentityErrorV1),
    ResourceLimit(
        crate::production_analysis::pliron_resource_envelope::ProductionAnalysisResourceLimitV1,
    ),
}

impl From<PlironIrIdentityErrorV1> for BuildIdentityFailureV1 {
    fn from(error: PlironIrIdentityErrorV1) -> Self {
        Self::Identity(error)
    }
}

include!("pliron_ir_identity/capture_v1.rs");

mod def_use_closure_v1;
mod scoped_verification_v1;

include!("pliron_ir_identity/resources_v1.rs");

include!("pliron_ir_identity/structure_v1.rs");

include!("pliron_ir_identity/type_rendering_v2.rs");

fn render_bounded(
    location: PlironPreserveLocationV1,
    entity: &'static str,
    render: impl FnOnce(&mut LimitedTextV1) -> fmt::Result,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_with_bounded_writer_v1(location, entity, LimitedTextV1::default(), render)
}

fn render_with_bounded_writer_v1(
    location: PlironPreserveLocationV1,
    entity: &'static str,
    mut writer: LimitedTextV1,
    render: impl FnOnce(&mut LimitedTextV1) -> fmt::Result,
) -> Result<String, PlironIrIdentityErrorV1> {
    let result = catch_unwind(AssertUnwindSafe(|| render(&mut writer)));
    match result {
        Err(_) => Err(PlironIrIdentityErrorV1::RenderingFailed {
            location,
            entity,
            detail: "the registered printer panicked",
        }),
        Ok(Err(_))
            if writer
                .type_nesting
                .as_ref()
                .is_some_and(|guard| guard.exceeded) =>
        {
            Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
                location,
                resource: "type rendering nesting",
                actual: MAX_TYPE_RENDER_DELIMITERS_V2 + 1,
                limit: MAX_TYPE_RENDER_DELIMITERS_V2,
            })
        }
        Ok(Err(_)) if writer.exceeded => Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
            location,
            resource: "rendered entity bytes",
            actual: MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 + 1,
            limit: MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1,
        }),
        Ok(Err(_)) => Err(PlironIrIdentityErrorV1::RenderingFailed {
            location,
            entity,
            detail: "the registered printer returned a formatting error",
        }),
        Ok(Ok(())) => Ok(writer.text),
    }
}

#[derive(Default)]
struct LimitedTextV1 {
    text: String,
    exceeded: bool,
    type_nesting: Option<TypeRenderNestingV2>,
}

#[derive(Default)]
struct DiagnosticSummaryV1 {
    text: String,
    chars: usize,
    truncated: bool,
}

impl DiagnosticSummaryV1 {
    fn append(&mut self, arguments: fmt::Arguments<'_>) {
        if !self.truncated {
            let _ = self.write_fmt(arguments);
        }
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn finish(mut self) -> String {
        if self.truncated {
            self.text.push_str("...");
        }
        self.text
    }
}

impl fmt::Write for DiagnosticSummaryV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for character in value.chars() {
            if self.chars == MAX_DIAGNOSTIC_DETAIL_CHARS_V1 {
                self.truncated = true;
                return Err(fmt::Error);
            }
            self.text.push(character);
            self.chars += 1;
        }
        Ok(())
    }
}

impl fmt::Write for LimitedTextV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self
            .text
            .len()
            .checked_add(value.len())
            .is_none_or(|length| length > MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1)
        {
            self.exceeded = true;
            return Err(fmt::Error);
        }
        if let Some(guard) = self.type_nesting.as_mut() {
            guard.observe(value)?;
        }
        self.text.push_str(value);
        Ok(())
    }
}

struct IdentityEncoderV1 {
    bytes: Vec<u8>,
    records: Vec<IdentityRecordV1>,
    emit: bool,
    encoded_len: usize,
    record_count: usize,
    string_payload_bytes: usize,
    record_summary_bytes: usize,
    record_location_name_bytes: usize,
}

impl IdentityEncoderV1 {
    fn counting() -> Self {
        Self {
            bytes: Vec::new(),
            records: Vec::new(),
            emit: false,
            encoded_len: 0,
            record_count: 0,
            string_payload_bytes: 0,
            record_summary_bytes: 0,
            record_location_name_bytes: 0,
        }
    }

    fn emitting(encoded_len: usize, record_count: usize) -> Result<Self, PlironIrIdentityErrorV1> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(encoded_len)
            .map_err(|_| canonical_bytes_resource_error(encoded_len))?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(record_count)
            .map_err(|_| canonical_bytes_resource_error(encoded_len))?;
        Ok(Self {
            bytes,
            records,
            emit: true,
            encoded_len: 0,
            record_count: 0,
            string_payload_bytes: 0,
            record_summary_bytes: 0,
            record_location_name_bytes: 0,
        })
    }

    fn record(
        &mut self,
        location: PlironPreserveLocationV1,
        component: &'static str,
        summary: String,
        encode: impl FnOnce(&mut Self) -> Result<(), PlironIrIdentityErrorV1>,
    ) -> Result<(), PlironIrIdentityErrorV1> {
        let start = self.encoded_len;
        self.byte(0xa5)?;
        self.string(component.as_bytes())?;
        encode(self)?;
        let end = self.encoded_len;
        let summary = truncate_detail(&summary);
        self.record_summary_bytes = self
            .record_summary_bytes
            .checked_add(summary.len())
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        if let PlironPreserveLocationV1::Operation { name, .. } = &location {
            self.record_location_name_bytes = self
                .record_location_name_bytes
                .checked_add(name.len())
                .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        }
        self.record_count = self
            .record_count
            .checked_add(1)
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        if self.emit {
            self.records.push(IdentityRecordV1 {
                range: start..end,
                location,
                component,
                summary,
            });
        }
        Ok(())
    }

    fn byte(&mut self, value: u8) -> Result<(), PlironIrIdentityErrorV1> {
        self.extend(&[value])
    }

    fn u64(&mut self, value: u64) -> Result<(), PlironIrIdentityErrorV1> {
        self.extend(&value.to_le_bytes())
    }

    fn usize(&mut self, value: usize) -> Result<(), PlironIrIdentityErrorV1> {
        self.u64(value as u64)
    }

    fn string(&mut self, value: &[u8]) -> Result<(), PlironIrIdentityErrorV1> {
        self.string_payload_bytes = self
            .string_payload_bytes
            .checked_add(value.len())
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        self.usize(value.len())?;
        self.extend(value)
    }

    fn extend(&mut self, value: &[u8]) -> Result<(), PlironIrIdentityErrorV1> {
        let Some(actual) = self.encoded_len.checked_add(value.len()) else {
            return Err(canonical_bytes_resource_error(usize::MAX));
        };
        if actual > MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1 {
            return Err(canonical_bytes_resource_error(actual));
        }
        self.encoded_len = actual;
        if self.emit {
            self.bytes.extend_from_slice(value);
        }
        Ok(())
    }
}

fn first_record_difference(
    before: &BuiltIdentityV1,
    after: &BuiltIdentityV1,
) -> (PlironPreserveLocationV1, &'static str, String, String) {
    for (before_record, after_record) in before.records.iter().zip(&after.records) {
        let before_bytes = &before.identity.canonical[before_record.range.clone()];
        let after_bytes = &after.identity.canonical[after_record.range.clone()];
        if before_bytes != after_bytes {
            return (
                after_record.location.clone(),
                after_record.component,
                before_record.summary.clone(),
                after_record.summary.clone(),
            );
        }
    }
    if let Some(record) = before.records.get(after.records.len()) {
        return (
            record.location.clone(),
            record.component,
            record.summary.clone(),
            "<missing>".to_owned(),
        );
    }
    if let Some(record) = after.records.get(before.records.len()) {
        return (
            record.location.clone(),
            record.component,
            "<missing>".to_owned(),
            record.summary.clone(),
        );
    }
    (
        PlironPreserveLocationV1::Function,
        "canonical bytes",
        "different canonical transcript".to_owned(),
        "different canonical transcript".to_owned(),
    )
}

fn check_limit(
    location: PlironPreserveLocationV1,
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), PlironIrIdentityErrorV1> {
    if actual > limit {
        Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
            location,
            resource,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

fn canonical_bytes_resource_error(actual: usize) -> PlironIrIdentityErrorV1 {
    PlironIrIdentityErrorV1::ResourceLimitExceeded {
        location: PlironPreserveLocationV1::Function,
        resource: "canonical bytes",
        actual,
        limit: MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1,
    }
}

fn truncate_detail(detail: &str) -> String {
    let mut result = detail
        .chars()
        .take(MAX_DIAGNOSTIC_DETAIL_CHARS_V1)
        .collect::<String>();
    if detail.chars().count() > MAX_DIAGNOSTIC_DETAIL_CHARS_V1 {
        result.push_str("...");
    }
    result
}

fn hex_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

include!("pliron_ir_identity/resource_tests.rs");
