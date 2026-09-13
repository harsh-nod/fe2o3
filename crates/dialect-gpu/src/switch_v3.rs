//! Self-contained typed multiway control. These operations grant no proof authority.
//!
//! Physical Index follows the production GPU64 representation contract. This
//! module does not select a target or relax ranked/identity/resource ceilings.

use std::{cell::Ref, ops::Range};

use crate::{TargetNeutralGpuOpInterface, optimization_v1::IndexType};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::OperandSegmentSizesAttr,
        op_interfaces::{
            ATTR_KEY_OPERAND_SEGMENT_SIZES, BranchOpInterface, IsTerminatorInterface,
            NRegionsInterface, NResultsInterface, OperandSegmentInterface,
        },
        types::{IntegerType, Signedness},
    },
    common_traits::Verify,
    context::{Context, Ptr},
    derive::{op_interface_impl, pliron_attr, pliron_op},
    location::Location,
    op::Op,
    operation::Operation,
    result::Result,
    r#type::{TypeHandle, Typed},
    value::Value,
    verify_err,
};

mod attributes;
mod interfaces;
mod key_validation;
pub use attributes::{SwitchCaseBitsAttrV3, SwitchKeyKindAttrV3, SwitchSuccessorOffsetsAttrV3};
use attributes::{valid_offsets, validate_keys};
pub use key_validation::{
    SMALL_LEGACY_SWITCH_KEYS_V3, SwitchKeyValidationResourcesV3,
    switch_key_validation_resources_v3, validate_switch_case_keys_v3,
};

/// Existing canonical KIR case ceiling; the bridge asserts equality explicitly.
pub const MAX_SWITCH_CASES_V3: usize = 65_536;
/// Existing canonical KIR per-edge value-argument ceiling, not a ranked cap.
pub const MAX_SWITCH_EDGE_ARGUMENTS_V3: usize = 65_536;

/// A located caller can retain this compact reason without allocating diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SwitchErrorV3 {
    /// The selector is not the exact physical integer type required by its keys.
    SelectorType,
    /// Keys are duplicate, unsorted, unrepresentable, or inconsistent with their kind.
    Keys,
    /// The key, successor, and segment rosters do not agree.
    Segments,
    /// An edge does not match its destination's exact physical signature.
    Payload,
    /// An existing wire ceiling or checked u32 operand representation was exceeded.
    Limit,
    /// An owned scratch/output allocation failed.
    Allocation,
    /// Generated interface or custom operation verification rejected construction.
    Verification,
}

/// One original successor with its exact ordered, possibly duplicate operands.
pub struct SwitchEdgeV3 {
    target: Ptr<BasicBlock>,
    arguments: Vec<Value>,
}

impl SwitchEdgeV3 {
    /// Creates descriptive edge input; the switch constructor checks it.
    pub fn new(target: Ptr<BasicBlock>, arguments: Vec<Value>) -> Self {
        Self { target, arguments }
    }
    /// Borrows all physical edge operands without deduplication.
    pub fn arguments(&self) -> &[Value] {
        &self.arguments
    }
    /// Returns the original successor block.
    pub fn target(&self) -> Ptr<BasicBlock> {
        self.target
    }
}

/// A real switch with complete case semantics, never an opaque source template.
#[pliron_op(
    name = "gpu.switch_v3",
    format,
    interfaces = [TargetNeutralGpuOpInterface, IsTerminatorInterface,
                  NResultsInterface<0>, NRegionsInterface<0>],
    operands = (selector, arguments),
    attributes = (gpu_switch_kind: SwitchKeyKindAttrV3,
                  gpu_switch_cases: SwitchCaseBitsAttrV3,
                  gpu_switch_offsets: SwitchSuccessorOffsetsAttrV3)
)]
pub struct SwitchOpV3;

impl SwitchOpV3 {
    /// Checks and constructs an unlinked switch; callers own cumulative budgets.
    /// Default is the final edge, after every key's corresponding case edge.
    pub fn try_new(
        ctx: &mut Context,
        selector: Value,
        kind: SwitchKeyKindAttrV3,
        keys: Vec<u64>,
        edges: Vec<SwitchEdgeV3>,
    ) -> std::result::Result<Self, SwitchErrorV3> {
        validate_keys(ctx, selector.get_type(ctx), kind, &keys)?;
        if edges.len() != keys.len().checked_add(1).ok_or(SwitchErrorV3::Limit)? {
            return Err(SwitchErrorV3::Segments);
        }
        let mut count = 0usize;
        for edge in &edges {
            if edge.arguments.len() > MAX_SWITCH_EDGE_ARGUMENTS_V3 {
                return Err(SwitchErrorV3::Limit);
            }
            count = count
                .checked_add(edge.arguments.len())
                .ok_or(SwitchErrorV3::Limit)?;
            let target = edge.target.deref(ctx);
            if target.get_num_arguments() != edge.arguments.len()
                || edge.arguments.iter().enumerate().any(|(ordinal, value)| {
                    value.get_type(ctx) != target.get_argument(ordinal).get_type(ctx)
                })
            {
                return Err(SwitchErrorV3::Payload);
            }
        }
        let payload_count = u32::try_from(count).map_err(|_| SwitchErrorV3::Limit)?;
        let operand_count = count.checked_add(1).ok_or(SwitchErrorV3::Limit)?;
        // The generated segment interface sums u32 lengths, including the selector.
        payload_count.checked_add(1).ok_or(SwitchErrorV3::Limit)?;
        let mut operands = Vec::new();
        let mut successors = Vec::new();
        let mut offsets = Vec::new();
        operands
            .try_reserve_exact(operand_count)
            .map_err(|_| SwitchErrorV3::Allocation)?;
        successors
            .try_reserve_exact(edges.len())
            .map_err(|_| SwitchErrorV3::Allocation)?;
        offsets
            .try_reserve_exact(edges.len() + 1)
            .map_err(|_| SwitchErrorV3::Allocation)?;
        operands.push(selector);
        offsets.push(0);
        for edge in edges {
            successors.push(edge.target);
            operands.extend(edge.arguments);
            offsets.push(u32::try_from(operands.len() - 1).map_err(|_| SwitchErrorV3::Limit)?);
        }
        let operation = Self {
            op: Operation::new(
                ctx,
                Self::get_concrete_op_info(),
                vec![],
                operands,
                successors,
                0,
            ),
        };
        operation.set_operand_segment_sizes(ctx, OperandSegmentSizesAttr(vec![1, payload_count]));
        operation.set_attr_gpu_switch_kind(ctx, kind);
        operation.set_attr_gpu_switch_cases(ctx, SwitchCaseBitsAttrV3(keys));
        operation.set_attr_gpu_switch_offsets(ctx, SwitchSuccessorOffsetsAttrV3(offsets));
        let checked = operation
            .verify(ctx)
            .and_then(|()| operation.verify_interfaces(ctx));
        if checked.is_err() {
            // Unlinked, with no results: erase all newly registered input/successor uses.
            Operation::erase(operation.get_operation(), ctx);
            return Err(SwitchErrorV3::Verification);
        }
        Ok(operation)
    }

    /// Returns the descriptive key encoding without deriving it from signed casts.
    pub fn kind(&self, ctx: &Context) -> Option<SwitchKeyKindAttrV3> {
        self.get_attr_gpu_switch_kind(ctx).map(|kind| *kind)
    }
    /// Borrows the actual key vector and its capacity for bounded consumers.
    pub fn cases<'a>(&self, ctx: &'a Context) -> Option<Ref<'a, SwitchCaseBitsAttrV3>> {
        self.get_attr_gpu_switch_cases(ctx)
    }
    /// Returns the actual selector safely even when inspecting malformed input.
    pub fn selector(&self, ctx: &Context) -> Option<Value> {
        let raw = self.get_operation().deref(ctx);
        (raw.get_num_operands() != 0).then(|| raw.get_operand(0))
    }
    /// Returns one exact operand range in O(1), including the selector offset.
    /// This is a shape observer, not a substitute for complete verification.
    pub fn successor_operand_range(&self, ctx: &Context, ordinal: usize) -> Option<Range<usize>> {
        let raw = self.get_operation().deref(ctx);
        if ordinal >= raw.get_num_successors() {
            return None;
        }
        let offsets = self.get_attr_gpu_switch_offsets(ctx)?;
        let start = (*offsets.0.get(ordinal)? as usize).checked_add(1)?;
        let end = (*offsets.0.get(ordinal.checked_add(1)?)? as usize).checked_add(1)?;
        (start <= end && end <= raw.get_num_operands()).then_some(start..end)
    }

    // Both custom and interface verification inspect the borrowed payloads.
    // In particular, a rejecting type check must not render arbitrary types.
    fn verify_successor_signatures(&self, ctx: &Context) -> Result<()> {
        let raw = self.get_operation().deref(ctx);
        let Some(offsets) = self.get_attr_gpu_switch_offsets(ctx) else {
            return verify_err!(self.loc(ctx), "native switch successor range is malformed");
        };
        if raw.get_num_successors() == 0
            || raw.get_num_successors() > MAX_SWITCH_CASES_V3 + 1
            || offsets.0.len() != raw.get_num_successors() + 1
            || offsets.0.first() != Some(&0)
            || offsets.0.last().copied().map(|end| end as usize)
                != raw.get_num_operands().checked_sub(1)
        {
            return verify_err!(self.loc(ctx), "native switch successor range is malformed");
        }
        for (ordinal, target) in raw.successors().enumerate() {
            let Some(range) = self.successor_operand_range(ctx, ordinal) else {
                return verify_err!(self.loc(ctx), "native switch successor range is malformed");
            };
            let target = target.deref(ctx);
            if range.len() > MAX_SWITCH_EDGE_ARGUMENTS_V3
                || range.len() != target.get_num_arguments()
                || range.enumerate().any(|(argument, operand)| {
                    raw.get_operand(operand).get_type(ctx)
                        != target.get_argument(argument).get_type(ctx)
                })
            {
                return verify_err!(
                    self.loc(ctx),
                    "native switch edge has an incompatible target signature"
                );
            }
        }
        Ok(())
    }
}

impl Verify for SwitchOpV3 {
    fn verify(&self, ctx: &Context) -> Result<()> {
        let raw = self.get_operation().deref(ctx);
        // Check the closed envelope before inspecting any attribute payload.
        if raw.attributes.0.len() != 4 {
            return verify_err!(
                self.loc(ctx),
                "native switch requires exactly four structural attributes"
            );
        }
        let (Some(kind), Some(keys), Some(offsets)) = (
            self.kind(ctx),
            self.cases(ctx),
            self.get_attr_gpu_switch_offsets(ctx),
        ) else {
            return verify_err!(
                self.loc(ctx),
                "native switch requires complete case attributes"
            );
        };
        let Some(segments) = raw
            .attributes
            .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
        else {
            return verify_err!(
                self.loc(ctx),
                "native switch requires generated operand segments"
            );
        };
        if raw.get_num_operands() == 0
            || raw.get_num_results() != 0
            || raw.num_regions() != 0
            || keys.0.len() > MAX_SWITCH_CASES_V3
            || raw.get_num_successors() != keys.0.len() + 1
            || offsets.0.len() != raw.get_num_successors() + 1
            || !valid_offsets(&offsets.0)
            || segments.0.len() != 2
            || segments.0[0] != 1
            || usize::try_from(segments.0[1]).ok() != Some(raw.get_num_operands() - 1)
            || offsets.0.last() != segments.0.get(1)
            || segments.0[1].checked_add(1).is_none()
        {
            return verify_err!(
                self.loc(ctx),
                "native switch key, operand and successor shapes disagree"
            );
        }
        if validate_keys(ctx, raw.get_operand(0).get_type(ctx), kind, &keys.0).is_err() {
            return verify_err!(
                self.loc(ctx),
                "native switch keys violate exact selector semantics"
            );
        }
        self.verify_successor_signatures(ctx)
    }
}

#[cfg(test)]
mod tests;
