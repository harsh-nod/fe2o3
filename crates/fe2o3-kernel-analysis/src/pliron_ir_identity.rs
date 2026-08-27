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
};

use pliron::{
    attribute::{AttrObj, AttributeDict},
    basic_block::BasicBlock,
    builtin::{op_interfaces::OneRegionInterface, ops::FuncOp},
    common_traits::Named,
    context::{Context, Ptr},
    linked_list::ContainsLinkedList,
    op::Op,
    operation::{Operation, verify_operation},
    printable::Printable,
    r#type::Typed,
    value::Value,
};
use sha2::{Digest, Sha256};

pub const MAX_PLIRON_IDENTITY_BLOCKS_V1: usize = 1_024;
pub const MAX_PLIRON_IDENTITY_OPERATIONS_V1: usize = 65_536;
pub const MAX_PLIRON_IDENTITY_VALUES_V1: usize = 131_072;
pub const MAX_PLIRON_IDENTITY_OPERANDS_V1: usize = 524_288;
pub const MAX_PLIRON_IDENTITY_SUCCESSORS_V1: usize = 16_384;
pub const MAX_PLIRON_IDENTITY_ATTRIBUTES_V1: usize = 262_144;
pub const MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1: usize = 65_536;
pub const MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1: usize = 16 * 1_024 * 1_024;

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

struct BuiltIdentityV1 {
    identity: PlironIrStructuralIdentityV1,
    records: Vec<IdentityRecordV1>,
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
            return Err(PlironIrIdentityErrorV1::StructuralVerificationFailed {
                detail: truncate_detail(&error.to_string()),
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
            let ty = render_type(context, argument, block_location.clone())?;
            let value_id = value_ids[&argument];
            encoder.record(
                block_location.clone(),
                "block argument type",
                format!("argument {argument_index}: {ty}"),
                |encoder| {
                    encoder.usize(argument_index)?;
                    encoder.u64(value_id)?;
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
                let ty = render_type(context, result, location.clone())?;
                rendered_result_bytes = rendered_result_bytes
                    .checked_add(ty.len())
                    .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
                if rendered_result_bytes > MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1 {
                    return Err(canonical_bytes_resource_error(rendered_result_bytes));
                }
                results.push((value_ids[&result], ty));
            }
            let result_summary = if results.is_empty() {
                "no results".to_owned()
            } else {
                results
                    .iter()
                    .map(|(value, ty)| format!("v{value}: {ty}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            encoder.record(
                location.clone(),
                "result types",
                result_summary,
                |encoder| {
                    encoder.usize(results.len())?;
                    for (value, ty) in &results {
                        encoder.u64(*value)?;
                        encoder.string(ty.as_bytes())?;
                    }
                    Ok(())
                },
            )?;
            let mut operand_ids = Vec::with_capacity(raw.get_num_operands());
            for (operand_index, operand) in raw.operands().enumerate() {
                let Some(value_id) = value_ids.get(&operand).copied() else {
                    return Err(PlironIrIdentityErrorV1::ExternalOperand {
                        location: location.clone(),
                        operand: operand_index,
                        value: operand.unique_name(context).to_string(),
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
            if !is_production_ranked_operation(&name) {
                return Err(PlironIrIdentityErrorV1::UnsupportedOperation {
                    location,
                    detail: "operation is outside the closed ranked operation allowlist",
                });
            }
            let raw = operation.deref(context);
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

fn is_production_ranked_operation(name: &str) -> bool {
    matches!(
        name,
        "kernel.ranked_view"
            | "kernel.index_constant"
            | "kernel.index_unknown"
            | "kernel.invocation_index"
            | "kernel.index_binary"
            | "kernel.deterministic_join"
            | "kernel.checked_tiled_index_2d"
            | "kernel.checked_row_striped_index_2d"
            | "kernel.dim"
            | "kernel.access"
            | "kernel.ownership_contract"
            | "kernel.allocation_effect"
            | "kernel.index_lt_br"
            | "kernel.index_lt_br_args"
            | "kernel.index_eq_br"
            | "kernel.index_eq_br_args"
            | "kernel.analysis_split"
            | "kernel.br"
            | "kernel.br_args"
            | "kernel.return"
            | "kernel.trap"
            | "gpu.barrier"
            | "gpu.execution_layout"
            | "gpu.fence"
            | "kernel.semantic_symbol"
            | "kernel.semantic_constant"
            | "kernel.semantic_binary"
            | "kernel.semantic_expression_commitment"
            | "kernel.semantic_typed_symbol"
            | "kernel.tensor_result_component"
            | "kernel.semantic_typed_constant"
            | "kernel.semantic_typed_unary"
            | "kernel.semantic_typed_binary"
            | "kernel.semantic_typed_compare"
            | "kernel.semantic_typed_select"
            | "kernel.semantic_typed_cast"
            | "kernel.semantic_typed_root"
            | "kernel.require_equivalent"
            | "kernel.require_finite_fold"
            | "kernel.require_finite_recurrence"
            | "kernel.require_permutation_gather"
            | "proof.obligation"
            | "proof.evidence_ref"
            | "proof.require_refinement"
            | "proof.require_tensor_refinement"
            | "proof.require_effect_refinement"
            | "proof.require_numerical_refinement"
            | "kernel.tensor_layout"
    )
}

fn encode_attributes(
    context: &Context,
    attributes: &AttributeDict,
    location: PlironPreserveLocationV1,
    encoder: &mut IdentityEncoderV1,
) -> Result<(), PlironIrIdentityErrorV1> {
    let mut sorted = attributes
        .0
        .iter()
        .map(|(key, attribute)| (key.to_string(), attribute))
        .collect::<Vec<_>>();
    sorted.sort_by(|lhs, rhs| lhs.0.cmp(&rhs.0));
    for (key, _) in &sorted {
        check_limit(
            location.clone(),
            "attribute key bytes",
            key.len(),
            MAX_IDENTIFIER_BYTES_V1,
        )?;
    }
    let mut rendered = Vec::with_capacity(sorted.len());
    let mut rendered_bytes = 0_usize;
    for (key, attribute) in sorted {
        let value = render_attribute(context, attribute, location.clone())?;
        rendered_bytes = rendered_bytes
            .checked_add(key.len())
            .and_then(|total| total.checked_add(value.len()))
            .ok_or_else(|| canonical_bytes_resource_error(usize::MAX))?;
        if rendered_bytes > MAX_PLIRON_IDENTITY_CANONICAL_BYTES_V1 {
            return Err(canonical_bytes_resource_error(rendered_bytes));
        }
        rendered.push((key, value));
    }
    let summary = if rendered.is_empty() {
        "no attributes".to_owned()
    } else {
        truncate_detail(
            &rendered
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", "),
        )
    };
    encoder.record(location.clone(), "attributes", summary, |encoder| {
        encoder.usize(rendered.len())?;
        for (key, value) in rendered {
            encoder.string(key.as_bytes())?;
            encoder.string(value.as_bytes())?;
        }
        Ok(())
    })
}

fn render_attribute(
    context: &Context,
    attribute: &AttrObj,
    location: PlironPreserveLocationV1,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_bounded(location, "attribute", |writer| {
        write!(writer, "{}", attribute.disp(context))
    })
}

fn render_type(
    context: &Context,
    value: Value,
    location: PlironPreserveLocationV1,
) -> Result<String, PlironIrIdentityErrorV1> {
    render_bounded(location, "type", |writer| {
        write!(writer, "{}", value.get_type(context).disp(context))
    })
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
