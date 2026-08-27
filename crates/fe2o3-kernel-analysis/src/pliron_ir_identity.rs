//! Bounded structural identities for the closed production ranked PLIRON subset.
//!
//! The transcript alpha-numbers blocks and SSA values by deterministic
//! region/block/operation order. It intentionally excludes pointer identities,
//! source locations, and optional block/SSA display labels. Every dictionary
//! attribute is retained. Equality compares the private canonical bytes, not
//! only the SHA-256 label.

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
    common_traits::Named,
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    printable::Printable,
    r#type::{Type, TypeHandle, Typed},
    value::Value,
};
use sha2::{Digest, Sha256};

use dialect_kernel::{IndexType, RankedViewType, SemanticScalarType};
use dialect_proof::{EvidenceRefType, ObligationRefType};

use crate::pliron_pass_contract::{
    IdentityCaptureFailureV1, IdentityComparisonFailureV1, PlironStructuralIdentityLabelV1,
    PlironStructuralIdentityProviderV1,
};
use crate::pliron_ranked_bounds::is_production_ranked_operation_v1;

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
}

pub(crate) struct LivePlironStructuralIdentityProviderV1<'a> {
    context: &'a Context,
    function: &'a FuncOp,
}

impl<'a> LivePlironStructuralIdentityProviderV1<'a> {
    pub(crate) const fn new(context: &'a Context, function: &'a FuncOp) -> Self {
        Self { context, function }
    }
}

impl PlironStructuralIdentityProviderV1 for LivePlironStructuralIdentityProviderV1<'_> {
    type Snapshot = BuiltIdentityV1;

    fn capture(&mut self) -> Result<Self::Snapshot, IdentityCaptureFailureV1> {
        build_identity_caught(self.context, self.function).map_err(|error| {
            IdentityCaptureFailureV1::Unavailable {
                source_code: error.code(),
                detail: error.to_string(),
            }
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
    operations: Vec<Vec<(Ptr<Operation>, String)>>,
    values: usize,
}

/// Constructs a bounded, deterministic identity for live PLIRON.
pub fn derive_pliron_ir_structural_identity_v1(
    context: &Context,
    function: &FuncOp,
) -> Result<PlironIrStructuralIdentityV1, PlironIrIdentityErrorV1> {
    build_identity_caught(context, function).map(|built| built.identity)
}

/// Requires byte-exact structural preservation between two verified snapshots.
///
/// This is not an operational equivalence or refinement theorem. A changed
/// function must re-enter the ordinary correctness pipeline.
pub fn require_pliron_ir_structural_identity_preserved_v1(
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
    catch_unwind(AssertUnwindSafe(|| build_identity(context, function)))
        .unwrap_or(Err(PlironIrIdentityErrorV1::TraversalPanicked))
}

fn build_identity(
    context: &Context,
    function: &FuncOp,
) -> Result<BuiltIdentityV1, PlironIrIdentityErrorV1> {
    let prescan = prescan(context, function)?;
    let verification = catch_unwind(AssertUnwindSafe(|| {
        verify_operation(function.get_operation(), context)
    }));
    match verification {
        Err(_) => {
            return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
                detail: "the PLIRON verifier panicked".to_owned(),
            });
        }
        Ok(Err(error)) => {
            let detail = render_bounded(
                PlironPreserveLocationV1::Function,
                "structural verifier diagnostic",
                |writer| write!(writer, "{error}"),
            )?;
            return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
                detail: truncate_detail(&detail),
            });
        }
        Ok(Ok(())) => {}
    }

    let block_ids = prescan
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (*block, index as u64))
        .collect::<HashMap<_, _>>();
    let mut value_ids = HashMap::<Value, u64>::with_capacity(prescan.values);
    let mut next_value = 0_u64;
    for (block_index, block) in prescan.blocks.iter().copied().enumerate() {
        for argument in block.deref(context).arguments() {
            value_ids.insert(argument, next_value);
            next_value += 1;
        }
        for (operation, _) in &prescan.operations[block_index] {
            for result in operation.deref(context).results() {
                value_ids.insert(result, next_value);
                next_value += 1;
            }
        }
    }

    let mut encoder = IdentityEncoderV1::new();
    encoder.record(
        PlironPreserveLocationV1::Function,
        "format",
        "ranked structural identity v1".to_owned(),
        |encoder| encoder.string(TRANSCRIPT_MAGIC_V1),
    )?;
    let root = function.get_operation();
    let root_raw = root.deref(context);
    let root_name = Operation::get_op_dyn(root, context).get_opid().to_string();
    encoder.record(
        PlironPreserveLocationV1::Function,
        "operation",
        root_name.clone(),
        |encoder| {
            encoder.string(root_name.as_bytes())?;
            encoder.usize(1)
        },
    )?;
    encode_attributes(
        context,
        &root_raw.attributes,
        PlironPreserveLocationV1::Function,
        &mut encoder,
    )?;

    for (block_index, block) in prescan.blocks.iter().copied().enumerate() {
        let block_ref = block.deref(context);
        let block_location = PlironPreserveLocationV1::Block { block: block_index };
        encoder.record(
            block_location.clone(),
            "block",
            format!(
                "{} arguments, {} operations",
                block_ref.get_num_arguments(),
                prescan.operations[block_index].len()
            ),
            |encoder| {
                encoder.usize(block_index)?;
                encoder.usize(block_ref.get_num_arguments())?;
                encoder.usize(prescan.operations[block_index].len())
            },
        )?;
        encode_attributes(
            context,
            &block_ref.attributes,
            block_location.clone(),
            &mut encoder,
        )?;
        for (argument_index, argument) in block_ref.arguments().enumerate() {
            let (type_id, ty) = render_type(context, argument, block_location.clone())?;
            let value_id = value_ids[&argument];
            encoder.record(
                block_location.clone(),
                "block argument type",
                format!("argument {argument_index}: {ty}"),
                |encoder| {
                    encoder.usize(argument_index)?;
                    encoder.u64(value_id)?;
                    encoder.string(type_id.as_bytes())?;
                    encoder.string(ty.as_bytes())
                },
            )?;
        }

        for (operation_index, (operation, name)) in
            prescan.operations[block_index].iter().enumerate()
        {
            let raw = operation.deref(context);
            let location = PlironPreserveLocationV1::Operation {
                block: block_index,
                operation: operation_index,
                name: name.clone(),
            };
            encoder.record(location.clone(), "operation", name.clone(), |encoder| {
                encoder.string(name.as_bytes())?;
                encoder.usize(raw.get_num_results())?;
                encoder.usize(raw.get_num_operands())?;
                encoder.usize(raw.get_num_successors())
            })?;
            let mut results = Vec::with_capacity(raw.get_num_results());
            let mut rendered_result_bytes = 0_usize;
            for result in raw.results() {
                let (type_id, ty) = render_type(context, result, location.clone())?;
                rendered_result_bytes = rendered_result_bytes
                    .checked_add(type_id.len())
                    .and_then(|total| total.checked_add(ty.len()))
                    .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
                if rendered_result_bytes > MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1 {
                    return Err(canonical_bytes_resource_error(rendered_result_bytes));
                }
                results.push((value_ids[&result], type_id, ty));
            }
            let result_summary = if results.is_empty() {
                "no results".to_owned()
            } else {
                results
                    .iter()
                    .map(|(value, type_id, ty)| format!("v{value}: {type_id} {ty}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            encoder.record(
                location.clone(),
                "result types",
                result_summary,
                |encoder| {
                    encoder.usize(results.len())?;
                    for (value, type_id, ty) in &results {
                        encoder.u64(*value)?;
                        encoder.string(type_id.as_bytes())?;
                        encoder.string(ty.as_bytes())?;
                    }
                    Ok(())
                },
            )?;
            let mut operand_ids = Vec::with_capacity(raw.get_num_operands());
            for (operand_index, operand) in raw.operands().enumerate() {
                let Some(value_id) = value_ids.get(&operand).copied() else {
                    let value =
                        render_bounded(location.clone(), "external value name", |writer| {
                            write!(writer, "{}", operand.unique_name(context))
                        })?;
                    return Err(PlironIrIdentityErrorV1::ExternalOperand {
                        location: location.clone(),
                        operand: operand_index,
                        value,
                    });
                };
                operand_ids.push(value_id);
            }
            let operand_summary = if operand_ids.is_empty() {
                "no operands".to_owned()
            } else {
                operand_ids
                    .iter()
                    .map(|value| format!("v{value}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            encoder.record(location.clone(), "operands", operand_summary, |encoder| {
                encoder.usize(operand_ids.len())?;
                for value_id in &operand_ids {
                    encoder.u64(*value_id)?;
                }
                Ok(())
            })?;
            encode_attributes(context, &raw.attributes, location.clone(), &mut encoder)?;
            let mut successor_ids = Vec::with_capacity(raw.get_num_successors());
            for (successor_index, successor) in raw.successors().enumerate() {
                let Some(block_id) = block_ids.get(&successor).copied() else {
                    return Err(PlironIrIdentityErrorV1::ExternalSuccessor {
                        location: location.clone(),
                        successor: successor_index,
                    });
                };
                successor_ids.push(block_id);
            }
            let successor_summary = if successor_ids.is_empty() {
                "no successors".to_owned()
            } else {
                successor_ids
                    .iter()
                    .map(|block| format!("block {block}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            encoder.record(
                location.clone(),
                "successors",
                successor_summary,
                |encoder| {
                    encoder.usize(successor_ids.len())?;
                    for block_id in &successor_ids {
                        encoder.u64(*block_id)?;
                    }
                    Ok(())
                },
            )?;
        }
    }
    let sha256 = Sha256::digest(&encoder.bytes).into();
    Ok(BuiltIdentityV1 {
        identity: PlironIrStructuralIdentityV1 {
            sha256,
            canonical: encoder.bytes,
            blocks: prescan.blocks.len(),
            operations: prescan.operations.iter().map(Vec::len).sum(),
            values: prescan.values,
        },
        records: encoder.records,
    })
}

fn prescan(context: &Context, function: &FuncOp) -> Result<PrescanV1, PlironIrIdentityErrorV1> {
    let root = function.get_operation();
    let root_ref = root.deref(context);
    let root_name = Operation::get_op_dyn(root, context).get_opid().to_string();
    if root_name != "builtin.func" {
        return Err(PlironIrIdentityErrorV1::UnsupportedRoot {
            operation: root_name,
            detail: "identity construction requires builtin.func",
        });
    }
    if root_ref.num_regions() != 1
        || root_ref.get_num_results() != 0
        || root_ref.get_num_operands() != 0
        || root_ref.get_num_successors() != 0
    {
        return Err(PlironIrIdentityErrorV1::UnsupportedRoot {
            operation: root_name,
            detail: "builtin.func must have exactly one region and no SSA results, operands, or successors",
        });
    }
    check_limit(
        PlironPreserveLocationV1::Function,
        "attributes",
        root_ref.attributes.0.len(),
        MAX_PLIRON_IDENTITY_ATTRIBUTES_V1,
    )?;
    validate_attribute_dict(
        context,
        &root_ref.attributes,
        PlironPreserveLocationV1::Function,
    )?;
    let function_type = function.get_type(context);
    validate_type_handle(context, function_type, PlironPreserveLocationV1::Function)?;
    let function_type_ref = function_type.deref(context);
    let function_type = function_type_ref
        .downcast_ref::<FunctionType>()
        .ok_or_else(|| PlironIrIdentityErrorV1::UnsupportedType {
            location: PlironPreserveLocationV1::Function,
            ty: function_type_ref.get_type_id().to_string(),
        })?;
    let function_arguments = function_type.arg_types();
    let function_results = function_type.res_types();
    check_limit(
        PlironPreserveLocationV1::Function,
        "function signature types",
        function_arguments
            .len()
            .saturating_add(function_results.len()),
        MAX_PLIRON_IDENTITY_VALUES_V1,
    )?;
    for ty in function_arguments.into_iter().chain(function_results) {
        validate_type_handle(context, ty, PlironPreserveLocationV1::Function)?;
    }

    let mut blocks = Vec::new();
    let mut operations = Vec::new();
    let mut operation_count = 0_usize;
    let mut values = 0_usize;
    let mut operands = 0_usize;
    let mut successors = 0_usize;
    let mut attributes = root_ref.attributes.0.len();
    for block in function.get_region(context).deref(context).iter(context) {
        check_limit(
            PlironPreserveLocationV1::Function,
            "basic blocks",
            blocks.len().saturating_add(1),
            MAX_PLIRON_IDENTITY_BLOCKS_V1,
        )?;
        let block_index = blocks.len();
        let block_ref = block.deref(context);
        let block_location = PlironPreserveLocationV1::Block { block: block_index };
        validate_attribute_dict(context, &block_ref.attributes, block_location.clone())?;
        for argument in block_ref.arguments() {
            validate_type_handle(context, argument.get_type(context), block_location.clone())?;
        }
        values = values.saturating_add(block_ref.get_num_arguments());
        attributes = attributes.saturating_add(block_ref.attributes.0.len());
        check_limit(
            PlironPreserveLocationV1::Block { block: block_index },
            "SSA values",
            values,
            MAX_PLIRON_IDENTITY_VALUES_V1,
        )?;
        check_limit(
            PlironPreserveLocationV1::Block { block: block_index },
            "attributes",
            attributes,
            MAX_PLIRON_IDENTITY_ATTRIBUTES_V1,
        )?;
        let mut block_operations = Vec::new();
        for (operation_index, operation) in block_ref.iter(context).enumerate() {
            operation_count = operation_count.saturating_add(1);
            let dynamic = Operation::get_op_dyn(operation, context);
            let name = dynamic.get_opid().to_string();
            let location = PlironPreserveLocationV1::Operation {
                block: block_index,
                operation: operation_index,
                name: name.clone(),
            };
            check_limit(
                location.clone(),
                "operations",
                operation_count,
                MAX_PLIRON_IDENTITY_OPERATIONS_V1,
            )?;
            if !is_production_ranked_operation_v1(dynamic.as_ref()) {
                return Err(PlironIrIdentityErrorV1::UnsupportedOperation {
                    location,
                    detail: "operation is outside the closed ranked operation allowlist",
                });
            }
            let raw = operation.deref(context);
            validate_attribute_dict(context, &raw.attributes, location.clone())?;
            for result in raw.results() {
                validate_type_handle(context, result.get_type(context), location.clone())?;
            }
            if raw.num_regions() != 0 {
                return Err(PlironIrIdentityErrorV1::UnsupportedOperation {
                    location,
                    detail: "ranked body operations must not contain nested regions",
                });
            }
            values = values.saturating_add(raw.get_num_results());
            operands = operands.saturating_add(raw.get_num_operands());
            successors = successors.saturating_add(raw.get_num_successors());
            attributes = attributes.saturating_add(raw.attributes.0.len());
            check_limit(
                location.clone(),
                "SSA values",
                values,
                MAX_PLIRON_IDENTITY_VALUES_V1,
            )?;
            check_limit(
                location.clone(),
                "operands",
                operands,
                MAX_PLIRON_IDENTITY_OPERANDS_V1,
            )?;
            check_limit(
                location.clone(),
                "CFG successors",
                successors,
                MAX_PLIRON_IDENTITY_SUCCESSORS_V1,
            )?;
            check_limit(
                location,
                "attributes",
                attributes,
                MAX_PLIRON_IDENTITY_ATTRIBUTES_V1,
            )?;
            block_operations.push((operation, name));
        }
        blocks.push(block);
        operations.push(block_operations);
    }
    if blocks.is_empty() {
        return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
            detail: "builtin.func has no basic blocks".to_owned(),
        });
    }
    Ok(PrescanV1 {
        blocks,
        operations,
        values,
    })
}

fn validate_attribute_dict(
    context: &Context,
    attributes: &AttributeDict,
    location: PlironPreserveLocationV1,
) -> Result<(), PlironIrIdentityErrorV1> {
    for attribute in attributes.0.values() {
        let attribute_id = attribute.get_attr_id().to_string();
        if !is_production_attribute_id(&attribute_id) {
            return Err(PlironIrIdentityErrorV1::UnsupportedAttribute {
                location: location.clone(),
                attribute: attribute_id,
            });
        }
        if let Some(type_attribute) = attribute.downcast_ref::<TypeAttr>() {
            validate_type_handle(context, type_attribute.get_type(context), location.clone())?;
        }
    }
    Ok(())
}

fn encode_attributes(
    context: &Context,
    attributes: &AttributeDict,
    location: PlironPreserveLocationV1,
    encoder: &mut IdentityEncoderV1,
) -> Result<(), PlironIrIdentityErrorV1> {
    let mut sorted = attributes.0.iter().collect::<Vec<_>>();
    sorted.sort_by(|lhs, rhs| lhs.0.cmp(rhs.0));
    for (key, _) in &sorted {
        check_limit(
            location.clone(),
            "attribute key bytes",
            key.as_ref().len(),
            MAX_IDENTIFIER_BYTES_V1,
        )?;
    }
    let mut rendered = Vec::with_capacity(sorted.len());
    let mut rendered_bytes = 0_usize;
    for (key, attribute) in sorted {
        let key = key.to_string();
        let (attribute_id, value) = render_attribute(context, attribute, location.clone())?;
        rendered_bytes = rendered_bytes
            .checked_add(key.len())
            .and_then(|total| total.checked_add(attribute_id.len()))
            .and_then(|total| total.checked_add(value.len()))
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        if rendered_bytes > MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1 {
            return Err(canonical_bytes_resource_error(rendered_bytes));
        }
        rendered.push((key, attribute_id, value));
    }
    let summary = if rendered.is_empty() {
        "no attributes".to_owned()
    } else {
        truncate_detail(
            &rendered
                .iter()
                .map(|(key, attribute_id, value)| format!("{key}={attribute_id} {value}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
    };
    encoder.record(location.clone(), "attributes", summary, |encoder| {
        encoder.usize(rendered.len())?;
        for (key, attribute_id, value) in rendered {
            encoder.string(key.as_bytes())?;
            encoder.string(attribute_id.as_bytes())?;
            encoder.string(value.as_bytes())?;
        }
        Ok(())
    })
}

fn render_attribute(
    context: &Context,
    attribute: &AttrObj,
    location: PlironPreserveLocationV1,
) -> Result<(String, String), PlironIrIdentityErrorV1> {
    let attribute_id = attribute.get_attr_id().to_string();
    if !is_production_attribute_id(&attribute_id) {
        return Err(PlironIrIdentityErrorV1::UnsupportedAttribute {
            location,
            attribute: attribute_id,
        });
    }
    let value = render_bounded(location, "attribute", |writer| {
        write!(writer, "{}", attribute.disp(context))
    })?;
    Ok((attribute_id, value))
}

fn render_type(
    context: &Context,
    value: Value,
    location: PlironPreserveLocationV1,
) -> Result<(String, String), PlironIrIdentityErrorV1> {
    render_type_handle(context, value.get_type(context), location)
}

fn render_type_handle(
    context: &Context,
    ty: TypeHandle,
    location: PlironPreserveLocationV1,
) -> Result<(String, String), PlironIrIdentityErrorV1> {
    let type_id = validate_type_handle(context, ty, location.clone())?;
    let value = render_bounded(location, "type", |writer| {
        write!(writer, "{}", ty.disp(context))
    })?;
    Ok((type_id, value))
}

fn validate_type_handle(
    context: &Context,
    ty: TypeHandle,
    location: PlironPreserveLocationV1,
) -> Result<String, PlironIrIdentityErrorV1> {
    validate_type_handle_at_depth(context, ty, location, 0)
}

fn validate_type_handle_at_depth(
    context: &Context,
    ty: TypeHandle,
    location: PlironPreserveLocationV1,
    depth: usize,
) -> Result<String, PlironIrIdentityErrorV1> {
    check_limit(
        location.clone(),
        "type nesting depth",
        depth,
        MAX_PLIRON_IDENTITY_TYPE_NESTING_V1,
    )?;
    let borrowed = ty.deref(context);
    let type_id = borrowed.get_type_id().to_string();
    if !is_production_type(&*borrowed) {
        return Err(PlironIrIdentityErrorV1::UnsupportedType {
            location,
            ty: type_id,
        });
    }
    let nested = borrowed
        .downcast_ref::<FunctionType>()
        .map(|function| {
            function
                .arg_types()
                .into_iter()
                .chain(function.res_types())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    drop(borrowed);
    for nested_type in nested {
        validate_type_handle_at_depth(context, nested_type, location.clone(), depth + 1)?;
    }
    Ok(type_id)
}

fn is_production_type(ty: &dyn Type) -> bool {
    ty.downcast_ref::<FunctionType>().is_some()
        || ty.downcast_ref::<IntegerType>().is_some()
        || ty.downcast_ref::<UnitType>().is_some()
        || ty.downcast_ref::<FP16Type>().is_some()
        || ty.downcast_ref::<FP32Type>().is_some()
        || ty.downcast_ref::<FP64Type>().is_some()
        || ty.downcast_ref::<IndexType>().is_some()
        || ty.downcast_ref::<RankedViewType>().is_some()
        || ty.downcast_ref::<SemanticScalarType>().is_some()
        || ty.downcast_ref::<ObligationRefType>().is_some()
        || ty.downcast_ref::<EvidenceRefType>().is_some()
}

fn is_production_attribute_id(attribute: &str) -> bool {
    matches!(
        attribute,
        "builtin.identifier"
            | "builtin.debug_info"
            | "builtin.string"
            | "builtin.bool"
            | "builtin.integer"
            | "builtin.unit"
            | "builtin.type"
            | "builtin.operand_segment_sizes"
            | "gpu.address_space"
            | "gpu.execution_domain"
            | "gpu.execution_extent"
            | "gpu.grid_identity"
            | "gpu.hierarchy"
            | "gpu.memory_order"
            | "gpu.memory_scope"
            | "gpu.subgroup_size"
            | "kernel.access_kind"
            | "kernel.allocation_origin"
            | "kernel.analysis_split_control_count"
            | "kernel.atomic_ordering"
            | "kernel.atomic_scope"
            | "kernel.dimension"
            | "kernel.index_binary_kind"
            | "kernel.index_value"
            | "kernel.invocation_dimension"
            | "kernel.launch_extent"
            | "kernel.memory_space"
            | "kernel.noalias_class"
            | "kernel.ownership_coverage"
            | "kernel.ownership_partition"
            | "kernel.semantic_binary_kind"
            | "kernel.semantic_cast_kind"
            | "kernel.semantic_compare_kind"
            | "kernel.semantic_constant"
            | "kernel.semantic_coverage_binding"
            | "kernel.semantic_domain_bound"
            | "kernel.semantic_evaluation_order"
            | "kernel.semantic_exceptional_value"
            | "kernel.semantic_expression_commitment"
            | "kernel.semantic_ieee_rounding"
            | "kernel.semantic_numerical_policy"
            | "kernel.semantic_overflow"
            | "kernel.semantic_scalar_kind"
            | "kernel.semantic_step_bound"
            | "kernel.semantic_symbol"
            | "kernel.semantic_typed_binary_kind"
            | "kernel.semantic_unary_kind"
            | "kernel.tensor_convergence"
            | "kernel.tensor_fragment"
            | "kernel.tensor_instruction"
            | "kernel.tensor_value_root"
            | "proof.absolute_error_f64_bits"
            | "proof.covered_boundary"
            | "proof.evidence_status"
            | "proof.id"
            | "proof.property"
            | "proof.relative_error_f64_bits"
    )
}

fn render_bounded(
    location: PlironPreserveLocationV1,
    entity: &'static str,
    render: impl FnOnce(&mut LimitedTextV1) -> fmt::Result,
) -> Result<String, PlironIrIdentityErrorV1> {
    let mut writer = LimitedTextV1::default();
    let result = catch_unwind(AssertUnwindSafe(|| render(&mut writer)));
    match result {
        Err(_) => Err(PlironIrIdentityErrorV1::RenderingFailed {
            location,
            entity,
            detail: "the registered printer panicked",
        }),
        Ok(Err(_)) if writer.exceeded => Err(PlironIrIdentityErrorV1::ResourceLimitExceeded {
            location,
            resource: "rendered entity bytes",
            actual: MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1.saturating_add(1),
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
        self.text.push_str(value);
        Ok(())
    }
}

struct IdentityEncoderV1 {
    bytes: Vec<u8>,
    records: Vec<IdentityRecordV1>,
}

impl IdentityEncoderV1 {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            records: Vec::new(),
        }
    }

    fn record(
        &mut self,
        location: PlironPreserveLocationV1,
        component: &'static str,
        summary: String,
        encode: impl FnOnce(&mut Self) -> Result<(), PlironIrIdentityErrorV1>,
    ) -> Result<(), PlironIrIdentityErrorV1> {
        let start = self.bytes.len();
        self.byte(0xa5)?;
        self.string(component.as_bytes())?;
        encode(self)?;
        let end = self.bytes.len();
        self.records.push(IdentityRecordV1 {
            range: start..end,
            location,
            component,
            summary: truncate_detail(&summary),
        });
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
        self.usize(value.len())?;
        self.extend(value)
    }

    fn extend(&mut self, value: &[u8]) -> Result<(), PlironIrIdentityErrorV1> {
        let Some(actual) = self.bytes.len().checked_add(value.len()) else {
            return Err(canonical_bytes_resource_error(usize::MAX));
        };
        if actual > MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1 {
            return Err(canonical_bytes_resource_error(actual));
        }
        self.bytes.extend_from_slice(value);
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
