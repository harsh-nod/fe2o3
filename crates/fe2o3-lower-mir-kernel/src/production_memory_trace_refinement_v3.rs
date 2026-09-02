//! Bounded byte-memory semantics for one guarded vector-style kernel lane.
//!
//! This module deliberately does not model general Rust memory, atomics,
//! concurrency, pointer exposure, LLVM, or hardware. Its executable model has
//! three disjoint byte allocations, two aligned little-endian `u32` reads, a
//! pure `u32` XOR/diamond helper call, and one aligned `u32` write. The false
//! guard has no memory effect. A separate live-owner checker binds that model to one
//! exact production semantic-MIR/KIR shape and to compiler-derived formal
//! allocation and byte-offset obligations.

use std::collections::{BTreeMap, BTreeSet};
use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, BlockId, ByteExpression, ComparePredicate, Constant,
    FormalAllocationParameter, FormalMemoryAccess, FormalMemoryAccessKind, FormalParameterKind,
    Function, FunctionId, FunctionOperationLocation, FunctionRole, Module, Operation,
    OperationKind, ScalarType, Terminator, Type, ValueId,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticBlockIdV1, SemanticCallableDeclV1, SemanticConstantValueV1,
    SemanticEdgeRoleV1, SemanticFunctionDeclV1, SemanticFunctionIdV1, SemanticFunctionRoleV1,
    SemanticLocalIdV1, SemanticLocalRoleV1, SemanticOperandV1, SemanticRvalueKindV1,
    SemanticScalarTypeV1, SemanticStatementKindV1, SemanticSwitchTargetsV1,
    SemanticTerminatorKindV1, SemanticTypeDeclV1, SemanticTypeShapeV1, SemanticUnwindActionV1,
};
use sha2::{Digest, Sha256};

use crate::{
    ProductionCanonicalKernelIrIdentityV1, ProductionFormalMemoryOwnerV1,
    ProductionSemanticKirOwnerV1, SemanticKirCorrespondenceV1, SemanticKirStatementOperationSpanV1,
};

/// Version of the bounded executable byte-memory model.
pub const MEMORY_TRACE_REFINEMENT_MODEL_VERSION_V3: u16 = 3;
/// Stable name of the positive Verus theorem.
pub const MEMORY_TRACE_REFINEMENT_THEOREM_V3: &str = "fe2o3_guarded_two_load_xor_store_refines_v3";
/// SHA-256 of the exact positive Verus source.
///
/// Updated by the proof runner's checked source manifest.
pub const MEMORY_TRACE_REFINEMENT_PROOF_SHA256_V3: [u8; 32] = [
    0xa9, 0x7c, 0xe1, 0x40, 0xfb, 0xb8, 0xf6, 0xb4, 0x73, 0x9c, 0x2a, 0xba, 0xb5, 0xce, 0xee, 0x88,
    0x02, 0xfa, 0x0d, 0xe1, 0x0d, 0xf5, 0xc8, 0x36, 0x9f, 0xd3, 0xa7, 0x78, 0x65, 0xe3, 0x42, 0x81,
];
/// SHA-256 of the pinned Verus executable.
pub const MEMORY_TRACE_REFINEMENT_VERUS_SHA256_V3: [u8; 32] = [
    0xad, 0x26, 0x69, 0xf5, 0x79, 0xd8, 0x98, 0xed, 0xe5, 0x3f, 0x2b, 0xf8, 0x4e, 0x80, 0xa1, 0xda,
    0xf4, 0xe3, 0x57, 0x87, 0x39, 0xb0, 0xf5, 0x80, 0x7e, 0xf2, 0x09, 0xa0, 0xc9, 0xf3, 0x82, 0xdd,
];
/// SHA-256 of the complete pinned Verus/vstd/Z3 closure manifest.
pub const MEMORY_TRACE_REFINEMENT_CLOSURE_SHA256_V3: [u8; 32] = [
    0xd2, 0x8d, 0xf3, 0xfb, 0x5e, 0x0d, 0x74, 0x76, 0x37, 0x54, 0x39, 0x33, 0xdf, 0xc3, 0x8c, 0xff,
    0x45, 0x57, 0x6d, 0xa9, 0xb9, 0x20, 0xd7, 0x55, 0xb4, 0xb7, 0xe9, 0x19, 0xe4, 0x7a, 0x60, 0x19,
];

const MODEL_DOMAIN_V3: &[u8] = b"FE2O3/SOURCE-MIR-KIR/GUARDED-U32-MEMORY/MODEL/V3\0";
const EVIDENCE_DOMAIN_V3: &[u8] = b"FE2O3/SOURCE-MIR-KIR/GUARDED-U32-MEMORY/EVIDENCE/V3\0";
const U32_BYTES_V3: u64 = 4;

/// A runtime allocation in the bounded executable semantics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryAllocationV3 {
    parameter: u32,
    provenance: [u8; 32],
    base_address: u64,
    required_alignment: u64,
    mutable: bool,
    bytes: Box<[u8]>,
}

impl MemoryAllocationV3 {
    /// Creates one allocation. Validation happens when a memory image is built.
    pub fn new(
        parameter: u32,
        provenance: [u8; 32],
        base_address: u64,
        required_alignment: u64,
        mutable: bool,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            parameter,
            provenance,
            base_address,
            required_alignment,
            mutable,
            bytes: bytes.into_boxed_slice(),
        }
    }

    /// Returns the formal parameter ordinal naming this allocation.
    pub const fn parameter(&self) -> u32 {
        self.parameter
    }

    /// Returns the runtime allocation provenance identity.
    pub const fn provenance(&self) -> &[u8; 32] {
        &self.provenance
    }

    /// Returns the modeled byte length.
    pub fn byte_len(&self) -> usize {
        self.bytes.len()
    }

    /// Returns the modeled bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// A validated set of pairwise-disjoint runtime byte allocations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteMemoryV3 {
    allocations: BTreeMap<u32, MemoryAllocationV3>,
}

impl ByteMemoryV3 {
    /// Validates nonzero provenance/alignment, unique parameters and provenance,
    /// representable address ranges, base alignment, and disjoint ranges.
    pub fn try_new(
        allocations: Vec<MemoryAllocationV3>,
    ) -> Result<Self, MemoryTraceRefinementErrorV3> {
        if allocations.len() != 3 {
            return Err(MemoryTraceRefinementErrorV3::AllocationRoster);
        }
        let mut by_parameter = BTreeMap::new();
        let mut provenances = BTreeSet::new();
        let mut ranges = Vec::with_capacity(allocations.len());
        for allocation in allocations {
            let len = u64::try_from(allocation.bytes.len())
                .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
            let end = allocation
                .base_address
                .checked_add(len)
                .ok_or(MemoryTraceRefinementErrorV3::AddressOverflow)?;
            if allocation.provenance == [0; 32]
                || allocation.required_alignment == 0
                || !allocation.required_alignment.is_power_of_two()
                || !allocation
                    .base_address
                    .is_multiple_of(allocation.required_alignment)
                || !provenances.insert(allocation.provenance)
                || by_parameter.contains_key(&allocation.parameter)
            {
                return Err(MemoryTraceRefinementErrorV3::AllocationRoster);
            }
            if ranges
                .iter()
                .any(|&(start, prior_end)| allocation.base_address < prior_end && start < end)
            {
                return Err(MemoryTraceRefinementErrorV3::OverlappingAllocations);
            }
            ranges.push((allocation.base_address, end));
            by_parameter.insert(allocation.parameter, allocation);
        }
        Ok(Self {
            allocations: by_parameter,
        })
    }

    fn allocation(
        &self,
        address: MemoryAddressV3,
    ) -> Result<&MemoryAllocationV3, MemoryTraceRefinementErrorV3> {
        let allocation = self
            .allocations
            .get(&address.parameter)
            .ok_or(MemoryTraceRefinementErrorV3::UnknownAllocation)?;
        if allocation.provenance != address.provenance {
            return Err(MemoryTraceRefinementErrorV3::ProvenanceMismatch);
        }
        Ok(allocation)
    }

    fn checked_u32_range(
        &self,
        address: MemoryAddressV3,
    ) -> Result<(usize, usize, u64), MemoryTraceRefinementErrorV3> {
        let allocation = self.allocation(address)?;
        let end_offset = address
            .byte_offset
            .checked_add(U32_BYTES_V3)
            .ok_or(MemoryTraceRefinementErrorV3::AddressOverflow)?;
        let absolute = allocation
            .base_address
            .checked_add(address.byte_offset)
            .ok_or(MemoryTraceRefinementErrorV3::AddressOverflow)?;
        let _end_absolute = absolute
            .checked_add(U32_BYTES_V3)
            .ok_or(MemoryTraceRefinementErrorV3::AddressOverflow)?;
        if !absolute.is_multiple_of(U32_BYTES_V3) || allocation.required_alignment < U32_BYTES_V3 {
            return Err(MemoryTraceRefinementErrorV3::MisalignedAccess);
        }
        if end_offset
            > u64::try_from(allocation.bytes.len())
                .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?
        {
            return Err(MemoryTraceRefinementErrorV3::OutOfBounds);
        }
        let start = usize::try_from(address.byte_offset)
            .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
        let end = usize::try_from(end_offset)
            .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
        Ok((start, end, absolute))
    }

    fn load_u32(
        &self,
        address: MemoryAddressV3,
    ) -> Result<(u32, MemoryTraceEventV3), MemoryTraceRefinementErrorV3> {
        let allocation = self.allocation(address)?;
        let (start, end, absolute) = self.checked_u32_range(address)?;
        let value = u32::from_le_bytes(
            allocation.bytes[start..end]
                .try_into()
                .expect("a checked u32 range is four bytes"),
        );
        Ok((
            value,
            MemoryTraceEventV3::ReadU32 {
                parameter: address.parameter,
                provenance: address.provenance,
                byte_range: (address.byte_offset, address.byte_offset + U32_BYTES_V3),
                absolute_address: absolute,
                value,
            },
        ))
    }

    fn store_u32(
        &mut self,
        address: MemoryAddressV3,
        value: u32,
    ) -> Result<MemoryTraceEventV3, MemoryTraceRefinementErrorV3> {
        let (start, end, absolute) = self.checked_u32_range(address)?;
        let allocation = self.allocation(address)?;
        if !allocation.mutable {
            return Err(MemoryTraceRefinementErrorV3::ImmutableStore);
        }
        let previous = u32::from_le_bytes(
            allocation.bytes[start..end]
                .try_into()
                .expect("a checked u32 range is four bytes"),
        );
        self.allocations
            .get_mut(&address.parameter)
            .expect("validated allocation remains present")
            .bytes[start..end]
            .copy_from_slice(&value.to_le_bytes());
        Ok(MemoryTraceEventV3::WriteU32 {
            parameter: address.parameter,
            provenance: address.provenance,
            byte_range: (address.byte_offset, address.byte_offset + U32_BYTES_V3),
            absolute_address: absolute,
            previous,
            value,
        })
    }
}

/// A provenance-bearing byte address relative to one formal parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryAddressV3 {
    /// Formal parameter ordinal.
    pub parameter: u32,
    /// Runtime allocation provenance identity.
    pub provenance: [u8; 32],
    /// Byte offset relative to the allocation base.
    pub byte_offset: u64,
}

/// One observable typed memory event.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryTraceEventV3 {
    /// One aligned little-endian `u32` read.
    ReadU32 {
        /// Formal parameter ordinal.
        parameter: u32,
        /// Runtime allocation provenance.
        provenance: [u8; 32],
        /// Half-open allocation-relative byte range.
        byte_range: (u64, u64),
        /// Checked absolute byte address.
        absolute_address: u64,
        /// Loaded value.
        value: u32,
    },
    /// One aligned little-endian `u32` write.
    WriteU32 {
        /// Formal parameter ordinal.
        parameter: u32,
        /// Runtime allocation provenance.
        provenance: [u8; 32],
        /// Half-open allocation-relative byte range.
        byte_range: (u64, u64),
        /// Checked absolute byte address.
        absolute_address: u64,
        /// Replaced value.
        previous: u32,
        /// Stored value.
        value: u32,
    },
}

/// Inputs to one vector-style lane of the bounded model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuardedMemoryLaneV3 {
    /// Whether the source bounds guard admits the lane.
    pub enabled: bool,
    /// Logical one-dimensional invocation index.
    pub invocation_index: u64,
    /// First input allocation provenance.
    pub first_input_provenance: [u8; 32],
    /// Second input allocation provenance.
    pub second_input_provenance: [u8; 32],
    /// Output allocation provenance.
    pub output_provenance: [u8; 32],
    /// First input parameter ordinal.
    pub first_input_parameter: u32,
    /// Second input parameter ordinal.
    pub second_input_parameter: u32,
    /// Output parameter ordinal.
    pub output_parameter: u32,
}

impl GuardedMemoryLaneV3 {
    fn validate_against(self, memory: &ByteMemoryV3) -> Result<(), MemoryTraceRefinementErrorV3> {
        let first = memory.allocation(MemoryAddressV3 {
            parameter: self.first_input_parameter,
            provenance: self.first_input_provenance,
            byte_offset: 0,
        })?;
        let second = memory.allocation(MemoryAddressV3 {
            parameter: self.second_input_parameter,
            provenance: self.second_input_provenance,
            byte_offset: 0,
        })?;
        let output = memory.allocation(MemoryAddressV3 {
            parameter: self.output_parameter,
            provenance: self.output_provenance,
            byte_offset: 0,
        })?;
        if first.mutable
            || second.mutable
            || !output.mutable
            || self.first_input_parameter == self.second_input_parameter
            || self.first_input_parameter == self.output_parameter
            || self.second_input_parameter == self.output_parameter
        {
            return Err(MemoryTraceRefinementErrorV3::AllocationRoster);
        }
        let end = self
            .invocation_index
            .checked_mul(U32_BYTES_V3)
            .and_then(|offset| offset.checked_add(U32_BYTES_V3))
            .ok_or(MemoryTraceRefinementErrorV3::AddressOverflow)?;
        let enabled_by_all_extents =
            [first, second, output]
                .into_iter()
                .try_fold(true, |enabled, allocation| {
                    let byte_len = u64::try_from(allocation.bytes.len())
                        .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
                    Ok::<_, MemoryTraceRefinementErrorV3>(enabled && end <= byte_len)
                })?;
        if self.enabled != enabled_by_all_extents {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        Ok(())
    }

    fn addresses(self) -> Result<[MemoryAddressV3; 3], MemoryTraceRefinementErrorV3> {
        let byte_offset = self
            .invocation_index
            .checked_mul(U32_BYTES_V3)
            .ok_or(MemoryTraceRefinementErrorV3::AddressOverflow)?;
        Ok([
            MemoryAddressV3 {
                parameter: self.first_input_parameter,
                provenance: self.first_input_provenance,
                byte_offset,
            },
            MemoryAddressV3 {
                parameter: self.second_input_parameter,
                provenance: self.second_input_provenance,
                byte_offset,
            },
            MemoryAddressV3 {
                parameter: self.output_parameter,
                provenance: self.output_provenance,
                byte_offset,
            },
        ])
    }
}

/// Final bytes, computed value, and ordered observable trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryTraceObservationV3 {
    /// Memory after the guarded lane.
    pub memory: ByteMemoryV3,
    /// Helper result when enabled; absent on the no-effect path.
    pub result: Option<u32>,
    /// Ordered read/read/write trace, or empty on the no-effect path.
    pub trace: Vec<MemoryTraceEventV3>,
}

/// Results at the source helper return, MIR call destination, and KIR call-result SSA boundary.
///
/// The retained identities intentionally match Track B's structured-CFG
/// boundary. This slice independently checks the exact helper result; the V3
/// aggregate composer must still equate the two tracks' live evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RelatedHelperResultsV3 {
    first_argument: u32,
    second_argument: u32,
    fallback: u32,
    source_result: u32,
    mir_call_destination: u32,
    kir_call_result: u32,
}

impl RelatedHelperResultsV3 {
    /// Constructs an exact helper-result relation, rejecting any substituted boundary value.
    pub fn try_new(
        first_argument: u32,
        second_argument: u32,
        fallback: u32,
        source_result: u32,
        mir_call_destination: u32,
        kir_call_result: u32,
    ) -> Result<Self, MemoryTraceRefinementErrorV3> {
        let expected = xor_diamond_helper_v3(first_argument, second_argument, fallback);
        if source_result != expected
            || mir_call_destination != expected
            || kir_call_result != expected
        {
            return Err(MemoryTraceRefinementErrorV3::HelperResultMismatch);
        }
        Ok(Self {
            first_argument,
            second_argument,
            fallback,
            source_result,
            mir_call_destination,
            kir_call_result,
        })
    }

    /// Returns the common result after checking all three boundaries.
    pub const fn result(self) -> u32 {
        self.source_result
    }

    fn validate_arguments(
        self,
        first: u32,
        second: u32,
    ) -> Result<(), MemoryTraceRefinementErrorV3> {
        if self.first_argument != first || self.second_argument != second {
            Err(MemoryTraceRefinementErrorV3::HelperResultMismatch)
        } else {
            Ok(())
        }
    }
}

fn xor_diamond_helper_v3(first: u32, second: u32, fallback: u32) -> u32 {
    let combined = first ^ second;
    if combined == 0 { combined } else { fallback }
}

/// Executes the source-language model for the bounded guarded lane.
pub fn execute_source_memory_lane_v3(
    mut memory: ByteMemoryV3,
    lane: GuardedMemoryLaneV3,
    helper: RelatedHelperResultsV3,
) -> Result<MemoryTraceObservationV3, MemoryTraceRefinementErrorV3> {
    lane.validate_against(&memory)?;
    if !lane.enabled {
        return Ok(MemoryTraceObservationV3 {
            memory,
            result: None,
            trace: vec![],
        });
    }
    let [first, second, output] = lane.addresses()?;
    let (first_value, first_event) = memory.load_u32(first)?;
    let (second_value, second_event) = memory.load_u32(second)?;
    helper.validate_arguments(first_value, second_value)?;
    let result = helper.source_result;
    let store_event = memory.store_u32(output, result)?;
    Ok(MemoryTraceObservationV3 {
        memory,
        result: Some(result),
        trace: vec![first_event, second_event, store_event],
    })
}

/// Executes the distinct semantic-MIR local/update model.
pub fn execute_mir_memory_lane_v3(
    mut memory: ByteMemoryV3,
    lane: GuardedMemoryLaneV3,
    helper: RelatedHelperResultsV3,
) -> Result<MemoryTraceObservationV3, MemoryTraceRefinementErrorV3> {
    lane.validate_against(&memory)?;
    let mut trace = Vec::with_capacity(3);
    let mut call_destination = None;
    if lane.enabled {
        let addresses = lane.addresses()?;
        let (value, event) = memory.load_u32(addresses[0])?;
        let first_local = value;
        trace.push(event);
        let (value, event) = memory.load_u32(addresses[1])?;
        let second_local = value;
        trace.push(event);
        helper.validate_arguments(first_local, second_local)?;
        call_destination = Some(helper.mir_call_destination);
        trace.push(memory.store_u32(
            addresses[2],
            call_destination.expect("defined MIR call destination"),
        )?);
    }
    Ok(MemoryTraceObservationV3 {
        memory,
        result: call_destination,
        trace,
    })
}

/// Executes the distinct KIR SSA-valuation model.
pub fn execute_kir_memory_lane_v3(
    mut memory: ByteMemoryV3,
    lane: GuardedMemoryLaneV3,
    helper: RelatedHelperResultsV3,
) -> Result<MemoryTraceObservationV3, MemoryTraceRefinementErrorV3> {
    lane.validate_against(&memory)?;
    let mut ssa = BTreeMap::<u32, u32>::new();
    let mut trace = Vec::with_capacity(3);
    if lane.enabled {
        let [first, second, output] = lane.addresses()?;
        let (value, event) = memory.load_u32(first)?;
        ssa.insert(1, value);
        trace.push(event);
        let (value, event) = memory.load_u32(second)?;
        ssa.insert(2, value);
        trace.push(event);
        let _helper_arguments = (
            *ssa.get(&1).expect("defined KIR first-load SSA"),
            *ssa.get(&2).expect("defined KIR second-load SSA"),
        );
        helper.validate_arguments(_helper_arguments.0, _helper_arguments.1)?;
        ssa.insert(3, helper.kir_call_result);
        trace.push(memory.store_u32(output, *ssa.get(&3).expect("defined KIR call-result SSA"))?);
    }
    Ok(MemoryTraceObservationV3 {
        memory,
        result: ssa.get(&3).copied(),
        trace,
    })
}

/// Runs all three executable semantics and requires identical observations.
pub fn check_guarded_memory_refinement_v3(
    memory: ByteMemoryV3,
    lane: GuardedMemoryLaneV3,
    helper: RelatedHelperResultsV3,
) -> Result<MemoryTraceObservationV3, MemoryTraceRefinementErrorV3> {
    let source = execute_source_memory_lane_v3(memory.clone(), lane, helper)?;
    let mir = execute_mir_memory_lane_v3(memory.clone(), lane, helper)?;
    let kir = execute_kir_memory_lane_v3(memory, lane, helper)?;
    if source != mir || mir != kir {
        return Err(MemoryTraceRefinementErrorV3::ObservationMismatch);
    }
    Ok(source)
}

/// Exact semantic sites selecting one supported production fragment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMemoryTraceSelectorV3 {
    /// Kernel-root semantic function.
    pub root_function: u32,
    /// Ordered semantic guards for input 0, input 1, and output length.
    pub guard_blocks: [u32; 3],
    /// Final true successor containing the memory fragment.
    pub enabled_block: u32,
    /// First semantic load statement site `(block, statement)`.
    pub first_load: (u32, u32),
    /// Second semantic load statement site `(block, statement)`.
    pub second_load: (u32, u32),
    /// Semantic block whose terminator calls the helper.
    pub helper_call_block: u32,
    /// Semantic store statement site `(block, statement)`.
    pub store: (u32, u32),
    /// Exact internal-helper semantic function.
    pub helper_function: u32,
}

/// Authority-free optional coverage for the exact bounded memory fragment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProductionMemoryTraceStatusV3 {
    /// The live owner has no semantic candidate in this narrow language.
    NotEligible,
    /// The unique candidate passed every semantic, KIR, formal-memory, and pin check.
    Verified(ProductionMemoryTraceEvidenceV3),
}

impl ProductionMemoryTraceStatusV3 {
    /// Discovers and checks the unique eligible fragment in a live production owner.
    pub fn from_live_owner(
        owner: &ProductionFormalMemoryOwnerV1,
    ) -> Result<Self, MemoryTraceRefinementErrorV3> {
        owner
            .verify_equivalence()
            .map_err(|error| MemoryTraceRefinementErrorV3::LiveOwner(error.to_string()))?;
        let candidates = discover_memory_trace_selectors_v3(owner)?;
        if candidates.is_empty() {
            return Ok(Self::NotEligible);
        }
        let [selector] = candidates.as_slice() else {
            return Err(MemoryTraceRefinementErrorV3::AmbiguousSelector);
        };
        match ProductionMemoryTraceEvidenceV3::from_revalidated_owner(owner, *selector) {
            Ok(evidence) => Ok(Self::Verified(evidence)),
            Err(error) => Err(error),
        }
    }

    /// Replays classification and rejects any changed status or evidence.
    pub fn revalidate_against(
        &self,
        owner: &ProductionFormalMemoryOwnerV1,
    ) -> Result<(), MemoryTraceRefinementErrorV3> {
        (Self::from_live_owner(owner)? == *self)
            .then_some(())
            .ok_or(MemoryTraceRefinementErrorV3::EvidenceMismatch)
    }

    /// Returns exact evidence only for a fully verified owner.
    pub const fn evidence(&self) -> Option<&ProductionMemoryTraceEvidenceV3> {
        match self {
            Self::NotEligible => None,
            Self::Verified(evidence) => Some(evidence),
        }
    }

    /// Optional classification never grants artifact or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Exact live production observation bound to the bounded memory model.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionMemoryTraceEvidenceV3 {
    identity: [u8; 32],
    model_identity: [u8; 32],
    semantic_mir_sha256: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    selector: ProductionMemoryTraceSelectorV3,
    parameters: [u32; 3],
    gid: ValueId,
    length_values: [ValueId; 3],
    guard_locations: [FunctionOperationLocation; 3],
    memory_locations: [FunctionOperationLocation; 3],
    root_call_location: FunctionOperationLocation,
    fallback: u32,
    semantic_root_values: [u32; 3],
    semantic_helper_values: [u32; 4],
    kir_root_values: [ValueId; 3],
    kir_helper_values: [ValueId; 5],
    source_site_sha256: [[u8; 32]; 7],
}

impl ProductionMemoryTraceEvidenceV3 {
    /// Revalidates a live semantic-MIR/KIR/formal-memory owner and admits only
    /// the exact guarded two-load/XOR-helper/store fragment.
    pub fn from_live_owner(
        owner: &ProductionFormalMemoryOwnerV1,
        selector: ProductionMemoryTraceSelectorV3,
    ) -> Result<Self, MemoryTraceRefinementErrorV3> {
        owner
            .verify_equivalence()
            .map_err(|error| MemoryTraceRefinementErrorV3::LiveOwner(error.to_string()))?;
        Self::from_revalidated_owner(owner, selector)
    }

    fn from_revalidated_owner(
        owner: &ProductionFormalMemoryOwnerV1,
        selector: ProductionMemoryTraceSelectorV3,
    ) -> Result<Self, MemoryTraceRefinementErrorV3> {
        let semantic_kir = owner.semantic_kir();
        let observation = validate_live_shape_v3(owner, semantic_kir, selector)?;
        let model_identity = memory_trace_refinement_model_identity_v3();
        let semantic_mir_sha256 = *semantic_kir
            .semantic()
            .semantic()
            .semantic_sha256()
            .as_bytes();
        let canonical_kernel_ir = semantic_kir.canonical_kernel_ir_identity();
        let mut evidence = Self {
            identity: [0; 32],
            model_identity,
            semantic_mir_sha256,
            canonical_kernel_ir,
            selector,
            parameters: observation.parameters,
            gid: observation.gid,
            length_values: observation.length_values,
            guard_locations: observation.guard_locations,
            memory_locations: observation.memory_locations,
            root_call_location: observation.root_call_location,
            fallback: observation.fallback,
            semantic_root_values: observation.semantic_root_values,
            semantic_helper_values: observation.semantic_helper_values,
            kir_root_values: observation.kir_root_values,
            kir_helper_values: observation.kir_helper_values,
            source_site_sha256: observation.source_site_sha256,
        };
        evidence.identity = evidence_identity_v3(&evidence);
        evidence.revalidate()?;
        Ok(evidence)
    }

    /// Rechecks proof/model pins and the complete retained identity payload.
    pub fn revalidate(&self) -> Result<(), MemoryTraceRefinementErrorV3> {
        if self.model_identity != memory_trace_refinement_model_identity_v3()
            || self.semantic_mir_sha256 == [0; 32]
            || self.canonical_kernel_ir.digest() == &[0; 32]
            || self.canonical_kernel_ir.canonical_length() == 0
            || self.parameters[0] == self.parameters[1]
            || self.parameters[0] == self.parameters[2]
            || self.parameters[1] == self.parameters[2]
            || self.length_values[0] == self.length_values[1]
            || self.length_values[0] == self.length_values[2]
            || self.length_values[1] == self.length_values[2]
            || self.length_values.contains(&self.gid)
            || self.guard_locations[0] == self.guard_locations[1]
            || self.guard_locations[0] == self.guard_locations[2]
            || self.guard_locations[1] == self.guard_locations[2]
            || self.kir_root_values[0] == self.kir_root_values[1]
            || self.kir_root_values[..2].contains(&self.kir_root_values[2])
            || self.semantic_root_values[0] == self.semantic_root_values[1]
            || self.semantic_root_values[..2].contains(&self.semantic_root_values[2])
            || self.source_site_sha256.contains(&[0; 32])
            || self.identity != evidence_identity_v3(self)
        {
            return Err(MemoryTraceRefinementErrorV3::EvidenceMismatch);
        }
        Ok(())
    }

    /// Returns the evidence content identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Returns the exact executable/formal model identity.
    pub const fn model_identity(&self) -> &[u8; 32] {
        &self.model_identity
    }

    /// Returns the exact semantic MIR identity.
    pub const fn semantic_mir_sha256(&self) -> &[u8; 32] {
        &self.semantic_mir_sha256
    }

    /// Returns the exact versioned KIR identity.
    pub const fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.canonical_kernel_ir
    }

    /// Returns the exact semantic-site selector retained by this evidence.
    pub const fn selector(&self) -> ProductionMemoryTraceSelectorV3 {
        self.selector
    }

    /// Returns the exact ordered input/input/output formal parameter ordinals.
    pub const fn parameters(&self) -> [u32; 3] {
        self.parameters
    }

    /// Returns the exact common KIR global-invocation-index SSA.
    pub const fn gid(&self) -> ValueId {
        self.gid
    }

    /// Returns the ordered input-0/input-1/output KIR `SliceLength` SSAs.
    pub const fn length_values(&self) -> [ValueId; 3] {
        self.length_values
    }

    /// Returns the ordered exact KIR compare-operation locations for the guards.
    pub const fn guard_locations(&self) -> [FunctionOperationLocation; 3] {
        self.guard_locations
    }

    /// Returns the exact ordered load/load/store operation locations.
    pub const fn memory_locations(&self) -> [FunctionOperationLocation; 3] {
        self.memory_locations
    }

    /// Returns the exact KIR helper-call operation location in the root.
    pub const fn root_call_location(&self) -> FunctionOperationLocation {
        self.root_call_location
    }

    /// Returns the exact nonzero-arm `u32` fallback constant.
    pub const fn fallback(&self) -> u32 {
        self.fallback
    }

    /// Returns root load destinations and the exact semantic call destination.
    pub const fn semantic_root_values(&self) -> [u32; 3] {
        self.semantic_root_values
    }

    /// Returns semantic helper left/right/XOR/return local identities.
    pub const fn semantic_helper_values(&self) -> [u32; 4] {
        self.semantic_helper_values
    }

    /// Returns KIR load results and the exact helper call-result SSA.
    pub const fn kir_root_values(&self) -> [ValueId; 3] {
        self.kir_root_values
    }

    /// Returns KIR helper left/right/XOR/fallback/join SSA identities.
    pub const fn kir_helper_values(&self) -> [ValueId; 5] {
        self.kir_helper_values
    }

    /// This evidence is conditional compiler-correctness evidence only.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// This bounded evidence does not claim general memory correctness.
    pub const fn claims_general_memory_correctness(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug)]
struct LiveShapeObservationV3 {
    parameters: [u32; 3],
    gid: ValueId,
    length_values: [ValueId; 3],
    guard_locations: [FunctionOperationLocation; 3],
    memory_locations: [FunctionOperationLocation; 3],
    root_call_location: FunctionOperationLocation,
    fallback: u32,
    semantic_root_values: [u32; 3],
    semantic_helper_values: [u32; 4],
    kir_root_values: [ValueId; 3],
    kir_helper_values: [ValueId; 5],
    source_site_sha256: [[u8; 32]; 7],
}

fn discover_memory_trace_selectors_v3(
    formal: &ProductionFormalMemoryOwnerV1,
) -> Result<Vec<ProductionMemoryTraceSelectorV3>, MemoryTraceRefinementErrorV3> {
    let owner = formal.semantic_kir();
    let semantic = owner.semantic().semantic();
    let roots = semantic
        .functions()
        .iter()
        .enumerate()
        .filter(|(_, function)| function.role() == SemanticFunctionRoleV1::KernelRoot)
        .collect::<Vec<_>>();
    let helpers = semantic
        .functions()
        .iter()
        .enumerate()
        .filter(|(_, function)| function.role() == SemanticFunctionRoleV1::InternalHelper)
        .collect::<Vec<_>>();
    let ([(root_index, root)], [(helper_index, _)]) = (roots.as_slice(), helpers.as_slice()) else {
        return Ok(Vec::new());
    };
    let root_function =
        u32::try_from(*root_index).map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
    let helper_function =
        u32::try_from(*helper_index).map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
    if validate_semantic_xor_helper_v3(owner, SemanticFunctionIdV1::from_index(helper_function))
        .is_err()
    {
        return Ok(Vec::new());
    }

    let mut loads = Vec::<(u32, u32, SemanticLocalIdV1)>::new();
    let mut stores = Vec::<(u32, u32, SemanticLocalIdV1)>::new();
    let mut calls = Vec::<(u32, [SemanticLocalIdV1; 2], SemanticLocalIdV1)>::new();
    let mut boolean_edges = BTreeMap::<u32, (u32, u32)>::new();
    for (block_index, block) in root.blocks().iter().enumerate() {
        let block_index = u32::try_from(block_index)
            .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let statement_index = u32::try_from(statement_index)
                .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment)
                    if assignment.destination().projections().is_empty()
                        && matches!(assignment.value().kind(), SemanticRvalueKindV1::Load(_)) =>
                {
                    loads.push((
                        block_index,
                        statement_index,
                        assignment.destination().local(),
                    ));
                }
                SemanticStatementKindV1::Store(store) => {
                    if let Some(value) = operand_direct_local_v3(store.value()) {
                        stores.push((block_index, statement_index, value));
                    }
                }
                _ => {}
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                let [left, right] = call.arguments() else {
                    continue;
                };
                let (Some(left), Some(right), Some(destination)) = (
                    operand_direct_local_v3(left),
                    operand_direct_local_v3(right),
                    call.destination()
                        .filter(|destination| destination.place().projections().is_empty())
                        .map(|destination| destination.place().local()),
                ) else {
                    continue;
                };
                let callable = semantic.callables().get(call.callee().index() as usize);
                if matches!(callable, Some(SemanticCallableDeclV1::Defined { function })
                    if function.index() == helper_function)
                {
                    calls.push((block_index, [left, right], destination));
                }
            }
            SemanticTerminatorKindV1::SwitchInt { targets, .. } => {
                if let Some(edges) = semantic_boolean_edges_v3(targets) {
                    boolean_edges.insert(block_index, edges);
                }
            }
            _ => {}
        }
    }

    let mut candidates = Vec::new();
    for (call_block, arguments, destination) in calls {
        let first_loads = loads
            .iter()
            .filter(|(_, _, local)| *local == arguments[0])
            .collect::<Vec<_>>();
        let second_loads = loads
            .iter()
            .filter(|(_, _, local)| *local == arguments[1])
            .collect::<Vec<_>>();
        let matching_stores = stores
            .iter()
            .filter(|(_, _, value)| *value == destination)
            .collect::<Vec<_>>();
        let ([first_load], [second_load], [store]) = (
            first_loads.as_slice(),
            second_loads.as_slice(),
            matching_stores.as_slice(),
        ) else {
            continue;
        };
        let first_guard = root.entry().index();
        if let Some(&(second_guard, _)) = boolean_edges.get(&first_guard) {
            let Some(&(third_guard, _)) = boolean_edges.get(&second_guard) else {
                continue;
            };
            let Some(&(enabled_block, _)) = boolean_edges.get(&third_guard) else {
                continue;
            };
            if [first_guard, second_guard, third_guard]
                .into_iter()
                .collect::<BTreeSet<_>>()
                .len()
                != 3
                || [first_guard, second_guard, third_guard].contains(&enabled_block)
            {
                continue;
            }
            let candidate = ProductionMemoryTraceSelectorV3 {
                root_function,
                guard_blocks: [first_guard, second_guard, third_guard],
                enabled_block,
                first_load: (first_load.0, first_load.1),
                second_load: (second_load.0, second_load.1),
                helper_call_block: call_block,
                store: (store.0, store.1),
                helper_function,
            };
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    }
    Ok(candidates)
}

fn semantic_boolean_edges_v3(targets: &SemanticSwitchTargetsV1) -> Option<(u32, u32)> {
    let [value] = targets.values() else {
        return None;
    };
    if value.edge().role() != SemanticEdgeRoleV1::SwitchValue
        || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
    {
        return None;
    }
    match value.value() {
        0 => Some((
            targets.otherwise().target().index(),
            value.edge().target().index(),
        )),
        1 => Some((
            value.edge().target().index(),
            targets.otherwise().target().index(),
        )),
        _ => None,
    }
}

fn validate_live_shape_v3(
    formal: &ProductionFormalMemoryOwnerV1,
    owner: &ProductionSemanticKirOwnerV1,
    selector: ProductionMemoryTraceSelectorV3,
) -> Result<LiveShapeObservationV3, MemoryTraceRefinementErrorV3> {
    let obligations = formal
        .obligations()
        .ok_or(MemoryTraceRefinementErrorV3::ProductionShape)?;
    if obligations.accesses().len() != 3 || !obligations.inter_invocation_conflicts().is_empty() {
        return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
    }
    let module = owner.module();
    let [kernel] = module.kernels.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::ProductionShape);
    };
    if &kernel.entry != obligations.entry() {
        return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
    }
    let root = find_function_v3(module, &kernel.entry)?;
    if root.role != FunctionRole::KernelEntry {
        return Err(MemoryTraceRefinementErrorV3::ProductionShape);
    }

    let correspondence = owner.correspondence();
    let semantic_function = SemanticFunctionIdV1::from_index(selector.root_function);
    let semantic = owner
        .semantic()
        .resolve_function(semantic_function)
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    if semantic.role() != SemanticFunctionRoleV1::KernelRoot
        || !source_available_v3(semantic.source())
    {
        return Err(MemoryTraceRefinementErrorV3::SourceIdentity);
    }
    let root_mapping = correspondence
        .lowered_functions()
        .iter()
        .find(|mapping| mapping.semantic_function() == semantic_function)
        .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)?;
    if root_mapping.kernel_ir_function() != &root.id
        || root_mapping.correspondence_owner() != semantic_function
    {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    }

    let load_sites = [selector.first_load, selector.second_load];
    let mut load_locals = [SemanticLocalIdV1::from_index(0); 2];
    let mut load_locations = [FunctionOperationLocation::new(BlockId(0), 0); 2];
    let mut load_results = [ValueId(0); 2];
    let mut parameters = [0_u32; 3];
    let mut source_site_sha256 = [[0_u8; 32]; 7];
    for (index, (block, statement)) in load_sites.into_iter().enumerate() {
        let site = owner
            .semantic()
            .resolve_statement(
                semantic_function,
                SemanticBlockIdV1::from_index(block),
                statement,
            )
            .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
        source_site_sha256[index + 3] = source_identity_v3(site.source())?;
        let SemanticStatementKindV1::Assign(assignment) = site.kind() else {
            return Err(MemoryTraceRefinementErrorV3::SemanticShape);
        };
        let SemanticRvalueKindV1::Load(load) = assignment.value().kind() else {
            return Err(MemoryTraceRefinementErrorV3::SemanticShape);
        };
        if load.atomic().is_some()
            || load.volatility()
                != fe2o3_mir_model::semantic_mir_v1::SemanticVolatilityV1::NonVolatile
            || !assignment.destination().projections().is_empty()
            || !is_u32_type_v3(owner, assignment.value().result_type())
            || !is_u32_type_v3(owner, load.source().ty())
        {
            return Err(MemoryTraceRefinementErrorV3::SemanticShape);
        }
        load_locals[index] = assignment.destination().local();
        let span = exact_statement_span_v3(correspondence, semantic_function, block, statement)?;
        let (location, operation) = exact_effect_in_span_v3(root, span, false)?;
        let OperationKind::Load { pointer, access } = operation.kind else {
            return Err(MemoryTraceRefinementErrorV3::KirShape);
        };
        if access.address_space != AddressSpace::Global
            || access.alignment != 4
            || access.volatile
            || operation
                .results
                .as_slice()
                .first()
                .map(|result| &result.ty)
                != Some(&Type::Scalar(ScalarType::U32))
            || operation.results.len() != 1
        {
            return Err(MemoryTraceRefinementErrorV3::KirShape);
        }
        let (kir_parameter, _gid) = indexed_parameter_pointer_v3(root, pointer)?;
        parameters[index] = kir_parameter;
        load_locations[index] = location;
        load_results[index] = operation.results[0].id;
    }

    let call_terminator = owner
        .semantic()
        .resolve_terminator(
            semantic_function,
            SemanticBlockIdV1::from_index(selector.helper_call_block),
        )
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    source_site_sha256[5] = source_identity_v3(call_terminator.source())?;
    let SemanticTerminatorKindV1::Call(call) = call_terminator.kind() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    let [first_argument, second_argument] = call.arguments() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    if operand_direct_local_v3(first_argument) != Some(load_locals[0])
        || operand_direct_local_v3(second_argument) != Some(load_locals[1])
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let call_destination_record = call
        .destination()
        .filter(|destination| destination.place().projections().is_empty())
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    let call_destination = call_destination_record.place().local();
    let call_destination_decl = semantic
        .locals()
        .get(call_destination.index() as usize)
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    if call_destination_decl.role() != SemanticLocalRoleV1::Temporary
        || !is_u32_type_v3(owner, call_destination_decl.ty())
        || call_destination_record.edge().role() != SemanticEdgeRoleV1::CallReturn
        || call_destination_record.edge().target().index() != selector.store.0
        || call.unwind() != SemanticUnwindActionV1::Unreachable
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let helper_function = SemanticFunctionIdV1::from_index(selector.helper_function);
    let callable = owner
        .semantic()
        .semantic()
        .callables()
        .get(call.callee().index() as usize)
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    if !matches!(callable, SemanticCallableDeclV1::Defined { function } if *function == helper_function)
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let semantic_helper = validate_semantic_xor_helper_v3(owner, helper_function)?;

    let call_span = correspondence
        .terminator_operation_spans()
        .iter()
        .find(|span| {
            span.semantic_function() == semantic_function
                && span.semantic_block().index() == selector.helper_call_block
        })
        .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)?;
    let [call_operation] = operations_in_terminator_span_v3(root, *call_span)? else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    let OperationKind::Call {
        callee: kir_helper,
        arguments,
    } = &call_operation.kind
    else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    if arguments.as_slice() != load_results
        || call_operation.results.len() != 1
        || call_operation.results[0].ty != Type::Scalar(ScalarType::U32)
        || !call_operation.memory_effects().is_empty()
    {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    }
    let helper_result = call_operation.results[0].id;
    let helper_mapping = correspondence
        .lowered_functions()
        .iter()
        .find(|mapping| mapping.semantic_function() == helper_function)
        .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)?;
    if helper_mapping.kernel_ir_function() != kir_helper
        || helper_mapping.correspondence_owner() != semantic_function
    {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    }
    let kir_helper_values = validate_kir_xor_helper_v3(
        module,
        kir_helper,
        correspondence,
        semantic_function,
        helper_function,
        semantic_helper,
    )?;
    let root_call_location = FunctionOperationLocation::new(
        call_span.kernel_ir_block(),
        call_span.first_operation_ordinal() as usize,
    );
    if selector.first_load.0 != selector.second_load.0
        || selector.first_load.0 != selector.helper_call_block
        || selector.first_load.1 >= selector.second_load.1
        || load_locations[0].block != root_call_location.block
        || load_locations[1].block != root_call_location.block
        || load_locations[0].operation_index >= load_locations[1].operation_index
        || load_locations[1].operation_index >= root_call_location.operation_index
    {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    }

    let store_statement = owner
        .semantic()
        .resolve_statement(
            semantic_function,
            SemanticBlockIdV1::from_index(selector.store.0),
            selector.store.1,
        )
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    source_site_sha256[6] = source_identity_v3(store_statement.source())?;
    let SemanticStatementKindV1::Store(store) = store_statement.kind() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    if store.atomic().is_some()
        || store.volatility() != fe2o3_mir_model::semantic_mir_v1::SemanticVolatilityV1::NonVolatile
        || operand_direct_local_v3(store.value()) != Some(call_destination)
        || !is_u32_type_v3(owner, store.destination().ty())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let store_span = exact_statement_span_v3(
        correspondence,
        semantic_function,
        selector.store.0,
        selector.store.1,
    )?;
    let (store_location, store_operation) = exact_effect_in_span_v3(root, store_span, true)?;
    let (pointer, stored_value, access) = match store_operation.kind {
        OperationKind::Store {
            pointer,
            value,
            access,
        } => (pointer, value, access),
        _ => return Err(MemoryTraceRefinementErrorV3::KirShape),
    };
    if stored_value != helper_result
        || access.address_space != AddressSpace::Global
        || access.alignment != 4
        || access.volatile
    {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    }
    let (kir_output_parameter, gid) = indexed_parameter_pointer_v3(root, pointer)?;
    parameters[2] = kir_output_parameter;
    if parameters[0] == parameters[1]
        || parameters[0] == parameters[2]
        || parameters[1] == parameters[2]
    {
        return Err(MemoryTraceRefinementErrorV3::AllocationRoster);
    }
    let root_body = root
        .body
        .as_ref()
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    let call_block = exact_block_v3(root_body, root_call_location.block)?;
    let store_block = exact_block_v3(root_body, store_location.block)?;
    if !matches!(call_block.terminator.as_ref(), Some(Terminator::Branch { target, arguments })
        if *target == store_location.block && arguments.is_empty())
        || !matches!(store_block.terminator.as_ref(), Some(Terminator::Return { values })
            if values.is_empty())
        || !matches!(
            semantic.blocks()[selector.store.0 as usize]
                .terminator()
                .kind(),
            SemanticTerminatorKindV1::Return
        )
    {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    }
    for location in load_locations {
        let operation = operation_at_v3(root, location)?;
        let OperationKind::Load { pointer, .. } = operation.kind else {
            return Err(MemoryTraceRefinementErrorV3::KirShape);
        };
        if indexed_parameter_pointer_v3(root, pointer)?.1 != gid {
            return Err(MemoryTraceRefinementErrorV3::KirShape);
        }
    }

    let guard_observation =
        validate_guard_v3(owner, root, semantic_function, selector, gid, parameters)?;
    let memory_locations = [load_locations[0], load_locations[1], store_location];
    validate_formal_obligations_v3(
        obligations.allocations(),
        obligations.accesses(),
        memory_locations,
        parameters,
    )?;
    for (index, block) in selector.guard_blocks.into_iter().enumerate() {
        source_site_sha256[index] = source_identity_v3(
            owner
                .semantic()
                .resolve_terminator(semantic_function, SemanticBlockIdV1::from_index(block))
                .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?
                .source(),
        )?;
    }
    Ok(LiveShapeObservationV3 {
        parameters,
        gid,
        length_values: guard_observation.length_values,
        guard_locations: guard_observation.guard_locations,
        memory_locations,
        root_call_location,
        fallback: semantic_helper.fallback,
        semantic_root_values: [
            load_locals[0].index(),
            load_locals[1].index(),
            call_destination.index(),
        ],
        semantic_helper_values: semantic_helper.values,
        kir_root_values: [load_results[0], load_results[1], helper_result],
        kir_helper_values,
        source_site_sha256,
    })
}

#[derive(Clone, Copy, Debug)]
struct GuardObservationV3 {
    length_values: [ValueId; 3],
    guard_locations: [FunctionOperationLocation; 3],
}

fn validate_guard_v3(
    owner: &ProductionSemanticKirOwnerV1,
    root: &Function,
    function: SemanticFunctionIdV1,
    selector: ProductionMemoryTraceSelectorV3,
    gid: ValueId,
    parameters: [u32; 3],
) -> Result<GuardObservationV3, MemoryTraceRefinementErrorV3> {
    let correspondence = owner.correspondence();
    let semantic = owner
        .semantic()
        .resolve_function(function)
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    if semantic.entry().index() != selector.guard_blocks[0] {
        return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
    }
    let enabled_kir =
        exact_block_binding_v3(correspondence, function, function, selector.enabled_block)?;
    let mut false_targets = Vec::with_capacity(3);
    let mut length_values = [ValueId(0); 3];
    let mut guard_locations = [FunctionOperationLocation::new(BlockId(0), 0); 3];
    for (index, guard_block) in selector.guard_blocks.into_iter().enumerate() {
        let guard = owner
            .semantic()
            .resolve_terminator(function, SemanticBlockIdV1::from_index(guard_block))
            .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
        let SemanticTerminatorKindV1::SwitchInt { targets, .. } = guard.kind() else {
            return Err(MemoryTraceRefinementErrorV3::SemanticShape);
        };
        let (true_target, false_target) = semantic_boolean_edges_v3(targets)
            .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
        let expected_true = selector
            .guard_blocks
            .get(index + 1)
            .copied()
            .unwrap_or(selector.enabled_block);
        if true_target != expected_true || false_target == expected_true {
            return Err(MemoryTraceRefinementErrorV3::SemanticShape);
        }
        let guard_kir = exact_block_binding_v3(correspondence, function, function, guard_block)?;
        if index == 0 {
            validate_declared_entry_guard_v3(root, guard_kir)?;
        }
        let expected_true_kir =
            exact_block_binding_v3(correspondence, function, function, expected_true)?;
        let false_kir = exact_block_binding_v3(correspondence, function, function, false_target)?;
        let block = root
            .body
            .as_ref()
            .and_then(|body| body.blocks.iter().find(|block| block.id == guard_kir))
            .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
        if block.operations.iter().any(|operation| {
            !operation.memory_effects().is_empty()
                || !operation.has_complete_effect_summary()
                || matches!(operation.kind, OperationKind::Call { .. })
        }) {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        let condition = match block.terminator.as_ref() {
            Some(Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            }) if *then_target == expected_true_kir
                && *else_target == false_kir
                && then_arguments.is_empty()
                && else_arguments.is_empty() =>
            {
                *condition
            }
            _ => return Err(MemoryTraceRefinementErrorV3::KirShape),
        };
        let (parameter, length_value, compare_location) =
            guarded_length_parameter_v3(root, condition, gid)?;
        if parameter != parameters[index] || compare_location.block != guard_kir {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        length_values[index] = length_value;
        guard_locations[index] = compare_location;
        false_targets.push((false_target, false_kir));
    }
    for (semantic_false_target, kir_false_target) in false_targets {
        let semantic_blocks = validate_no_semantic_effect_path_v3(
            owner,
            function,
            semantic_false_target,
            selector.enabled_block,
        )?;
        let kir_blocks = validate_no_memory_effect_path_v3(root, kir_false_target, enabled_kir)?;
        let mapped_semantic_blocks = semantic_blocks
            .into_iter()
            .map(|block| exact_block_binding_v3(correspondence, function, function, block))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if mapped_semantic_blocks != kir_blocks {
            return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
        }
    }
    let dominated = dominated_blocks_v3(root, enabled_kir)?;
    for semantic_block in [
        selector.first_load.0,
        selector.second_load.0,
        selector.helper_call_block,
        selector.store.0,
    ] {
        let kir_block = correspondence
            .blocks()
            .iter()
            .find(|mapping| {
                mapping.semantic_function() == function
                    && mapping.semantic_block().index() == semantic_block
            })
            .map(|mapping| mapping.kernel_ir_block())
            .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)?;
        if !dominated.contains(&kir_block) {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
    }
    Ok(GuardObservationV3 {
        length_values,
        guard_locations,
    })
}

fn validate_declared_entry_guard_v3(
    function: &Function,
    guard: BlockId,
) -> Result<(), MemoryTraceRefinementErrorV3> {
    (function
        .body
        .as_ref()
        .and_then(|body| body.blocks.first())
        .map(|block| block.id)
        == Some(guard))
    .then_some(())
    .ok_or(MemoryTraceRefinementErrorV3::GuardMismatch)
}

fn guarded_length_parameter_v3(
    function: &Function,
    condition: ValueId,
    gid: ValueId,
) -> Result<(u32, ValueId, FunctionOperationLocation), MemoryTraceRefinementErrorV3> {
    let (compare_location, compare) = definition_at_v3(function, condition)?;
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = compare.kind
    else {
        return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
    };
    if lhs != gid
        || compare.results.len() != 1
        || compare.results[0].id != condition
        || compare.results[0].ty != Type::BOOL
        || !compare.memory_effects().is_empty()
    {
        return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
    }
    let length = definition_v3(function, rhs)?;
    let OperationKind::SliceLength { slice } = length.kind else {
        return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
    };
    if length.results.len() != 1
        || length.results[0].id != rhs
        || !length.memory_effects().is_empty()
    {
        return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
    }
    Ok((
        parameter_ordinal_v3(function, slice)?,
        rhs,
        compare_location,
    ))
}

fn validate_no_memory_effect_path_v3(
    function: &Function,
    start: BlockId,
    forbidden: BlockId,
) -> Result<BTreeSet<BlockId>, MemoryTraceRefinementErrorV3> {
    let body = function
        .body
        .as_ref()
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    fn visit(
        body: &fe2o3_kernel_ir::FunctionBody,
        block_id: BlockId,
        forbidden: BlockId,
        visiting: &mut BTreeSet<BlockId>,
        complete: &mut BTreeSet<BlockId>,
    ) -> Result<(), MemoryTraceRefinementErrorV3> {
        if block_id == forbidden {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        if complete.contains(&block_id) {
            return Ok(());
        }
        if !visiting.insert(block_id) {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        let block = exact_block_v3(body, block_id)?;
        if block.operations.iter().any(|operation| {
            !operation.memory_effects().is_empty()
                || !operation.has_complete_effect_summary()
                || matches!(operation.kind, OperationKind::Call { .. })
        }) {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
        match terminator {
            Terminator::Return { values } if values.is_empty() => {}
            Terminator::Return { .. } | Terminator::Unreachable => {
                return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
            }
            _ => {
                for successor in terminator.successors() {
                    visit(body, successor, forbidden, visiting, complete)?;
                }
            }
        }
        visiting.remove(&block_id);
        complete.insert(block_id);
        Ok(())
    }
    let mut complete = BTreeSet::new();
    visit(body, start, forbidden, &mut BTreeSet::new(), &mut complete)?;
    Ok(complete)
}

fn validate_no_semantic_effect_path_v3(
    owner: &ProductionSemanticKirOwnerV1,
    function: SemanticFunctionIdV1,
    start: u32,
    forbidden: u32,
) -> Result<BTreeSet<u32>, MemoryTraceRefinementErrorV3> {
    let semantic = owner
        .semantic()
        .resolve_function(function)
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    fn visit(
        semantic: &SemanticFunctionDeclV1,
        block_index: u32,
        forbidden: u32,
        visiting: &mut BTreeSet<u32>,
        complete: &mut BTreeSet<u32>,
    ) -> Result<(), MemoryTraceRefinementErrorV3> {
        if block_index == forbidden {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        if complete.contains(&block_index) {
            return Ok(());
        }
        if !visiting.insert(block_index) {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        let block = semantic
            .blocks()
            .get(block_index as usize)
            .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
        if !block.statements().is_empty() || !source_available_v3(block.terminator().source()) {
            return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Return => {}
            SemanticTerminatorKindV1::Goto(edge) if edge.role() == SemanticEdgeRoleV1::Goto => {
                visit(
                    semantic,
                    edge.target().index(),
                    forbidden,
                    visiting,
                    complete,
                )?;
            }
            _ => return Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        }
        visiting.remove(&block_index);
        complete.insert(block_index);
        Ok(())
    }
    let mut complete = BTreeSet::new();
    visit(
        semantic,
        start,
        forbidden,
        &mut BTreeSet::new(),
        &mut complete,
    )?;
    Ok(complete)
}

fn dominated_blocks_v3(
    function: &Function,
    start: BlockId,
) -> Result<BTreeSet<BlockId>, MemoryTraceRefinementErrorV3> {
    let body = function
        .body
        .as_ref()
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    let predecessors = predecessor_map_v3(function)?;
    let all = body
        .blocks
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let entry = body
        .blocks
        .first()
        .map(|block| block.id)
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    let mut dominators = BTreeMap::new();
    for block in &body.blocks {
        dominators.insert(
            block.id,
            if block.id == entry {
                BTreeSet::from([entry])
            } else {
                all.clone()
            },
        );
    }
    loop {
        let mut changed = false;
        for block in &body.blocks {
            if block.id == entry {
                continue;
            }
            let incoming = predecessors.get(&block.id).cloned().unwrap_or_default();
            if incoming.is_empty() {
                return Err(MemoryTraceRefinementErrorV3::GuardMismatch);
            }
            let mut next = all.clone();
            for predecessor in incoming {
                next = next
                    .intersection(&dominators[&predecessor])
                    .copied()
                    .collect();
            }
            next.insert(block.id);
            if dominators[&block.id] != next {
                dominators.insert(block.id, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    Ok(dominators
        .into_iter()
        .filter_map(|(block, values)| values.contains(&start).then_some(block))
        .collect())
}

fn predecessor_map_v3(
    function: &Function,
) -> Result<BTreeMap<BlockId, BTreeSet<BlockId>>, MemoryTraceRefinementErrorV3> {
    let body = function
        .body
        .as_ref()
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    let mut predecessors = body
        .blocks
        .iter()
        .map(|block| (block.id, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        let terminator = block
            .terminator
            .as_ref()
            .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
        let mut successors = Vec::new();
        match terminator {
            Terminator::Branch { target, .. } => successors.push(*target),
            Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            } => successors.extend([*then_target, *else_target]),
            Terminator::Switch {
                cases,
                default_target,
                ..
            } => {
                successors.extend(cases.iter().map(|case| case.target));
                successors.push(*default_target);
            }
            Terminator::IntegerSwitch {
                cases,
                default_target,
                ..
            } => {
                successors.extend(cases.iter().map(|case| case.target));
                successors.push(*default_target);
            }
            Terminator::Return { .. } | Terminator::Unreachable => {}
        }
        for successor in successors {
            predecessors
                .get_mut(&successor)
                .ok_or(MemoryTraceRefinementErrorV3::KirShape)?
                .insert(block.id);
        }
    }
    Ok(predecessors)
}

fn validate_formal_obligations_v3(
    allocations: &[FormalAllocationParameter],
    accesses: &[FormalMemoryAccess],
    locations: [FunctionOperationLocation; 3],
    parameters: [u32; 3],
) -> Result<(), MemoryTraceRefinementErrorV3> {
    if allocations.len() != 3 {
        return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
    }
    for (index, parameter) in parameters.into_iter().enumerate() {
        let matches = allocations
            .iter()
            .filter(|allocation| allocation.identity().parameter_index() == parameter)
            .collect::<Vec<_>>();
        let [allocation] = matches.as_slice() else {
            return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
        };
        let expected_access = if index < 2 {
            AccessMode::ReadOnly
        } else if matches!(
            allocation.access(),
            AccessMode::WriteOnly | AccessMode::ReadWrite
        ) {
            allocation.access()
        } else {
            return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
        };
        if allocation.kind() != FormalParameterKind::Slice
            || allocation.address_space() != AddressSpace::Global
            || allocation.access() != expected_access
        {
            return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
        }
    }
    for (index, (location, parameter)) in locations.into_iter().zip(parameters).enumerate() {
        let matches = accesses
            .iter()
            .filter(|access| access.location() == location)
            .collect::<Vec<_>>();
        let [access] = matches.as_slice() else {
            return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
        };
        let expected_kind = if index < 2 {
            FormalMemoryAccessKind::Read
        } else {
            FormalMemoryAccessKind::Write
        };
        if access.allocation().parameter_index() != parameter
            || access.kind() != expected_kind
            || access.address_space() != AddressSpace::Global
            || access.byte_width() != U32_BYTES_V3
            || access.alignment() != U32_BYTES_V3
            || access.byte_offset() != ByteExpression::invocation_affine(0, U32_BYTES_V3)
        {
            return Err(MemoryTraceRefinementErrorV3::FormalObligationMismatch);
        }
    }
    Ok(())
}

fn exact_statement_span_v3(
    correspondence: &SemanticKirCorrespondenceV1,
    function: SemanticFunctionIdV1,
    block: u32,
    statement: u32,
) -> Result<SemanticKirStatementOperationSpanV1, MemoryTraceRefinementErrorV3> {
    let matches = correspondence
        .statement_operation_spans()
        .iter()
        .filter(|span| {
            span.semantic_function() == function
                && span.semantic_block().index() == block
                && span.statement_ordinal() == statement
        })
        .copied()
        .collect::<Vec<_>>();
    let [span] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    };
    if span.correspondence_owner() != function {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    }
    Ok(*span)
}

fn exact_effect_in_span_v3<'a>(
    function: &'a Function,
    span: SemanticKirStatementOperationSpanV1,
    write: bool,
) -> Result<(FunctionOperationLocation, &'a Operation), MemoryTraceRefinementErrorV3> {
    let block = function
        .body
        .as_ref()
        .and_then(|body| {
            body.blocks
                .iter()
                .find(|block| block.id == span.kernel_ir_block())
        })
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    let start = usize::try_from(span.first_operation_ordinal())
        .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
    let count = usize::try_from(span.operation_count())
        .map_err(|_| MemoryTraceRefinementErrorV3::AddressOverflow)?;
    let end = start
        .checked_add(count)
        .filter(|end| *end <= block.operations.len())
        .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)?;
    let matches = block.operations[start..end]
        .iter()
        .enumerate()
        .filter(|(_, operation)| {
            if write {
                matches!(operation.kind, OperationKind::Store { .. })
            } else {
                matches!(operation.kind, OperationKind::Load { .. })
            }
        })
        .collect::<Vec<_>>();
    let [(relative, operation)] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    Ok((
        FunctionOperationLocation::new(block.id, start + relative),
        *operation,
    ))
}

fn operations_in_terminator_span_v3<'a>(
    function: &'a Function,
    span: crate::SemanticKirTerminatorOperationSpanV1,
) -> Result<&'a [Operation], MemoryTraceRefinementErrorV3> {
    let block = function
        .body
        .as_ref()
        .and_then(|body| {
            body.blocks
                .iter()
                .find(|block| block.id == span.kernel_ir_block())
        })
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    let start = span.first_operation_ordinal() as usize;
    let end = start
        .checked_add(span.operation_count() as usize)
        .filter(|end| *end <= block.operations.len())
        .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)?;
    Ok(&block.operations[start..end])
}

#[derive(Clone, Copy, Debug)]
struct SemanticHelperObservationV3 {
    fallback: u32,
    values: [u32; 4],
    blocks: [u32; 4],
}

fn validate_semantic_xor_helper_v3(
    owner: &ProductionSemanticKirOwnerV1,
    function: SemanticFunctionIdV1,
) -> Result<SemanticHelperObservationV3, MemoryTraceRefinementErrorV3> {
    let helper = owner
        .semantic()
        .resolve_function(function)
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    if helper.role() != SemanticFunctionRoleV1::InternalHelper
        || helper.blocks().len() != 4
        || helper.locals().len() != 4
        || helper
            .locals()
            .iter()
            .any(|local| !is_u32_type_v3(owner, local.ty()))
        || !source_available_v3(helper.source())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let return_local = unique_local_with_role_v3(helper, SemanticLocalRoleV1::Return)?;
    let left_local = unique_local_with_role_v3(helper, SemanticLocalRoleV1::Argument(0))?;
    let right_local = unique_local_with_role_v3(helper, SemanticLocalRoleV1::Argument(1))?;
    let xor_local = unique_local_with_role_v3(helper, SemanticLocalRoleV1::Temporary)?;
    let entry = helper.entry().index();
    let entry_block = helper
        .blocks()
        .get(entry as usize)
        .ok_or(MemoryTraceRefinementErrorV3::SemanticShape)?;
    let [statement] = entry_block.statements() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::BitXor,
        left,
        right,
    } = assignment.value().kind()
    else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    if assignment.destination().local() != xor_local
        || !assignment.destination().projections().is_empty()
        || operand_direct_local_v3(left) != Some(left_local)
        || operand_direct_local_v3(right) != Some(right_local)
        || !source_available_v3(statement.source())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = entry_block.terminator().kind()
    else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    if operand_direct_local_v3(discriminant) != Some(xor_local)
        || targets.values().len() != 1
        || targets.values()[0].value() != 0
        || targets.values()[0].edge().role() != SemanticEdgeRoleV1::SwitchValue
        || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
        || !source_available_v3(entry_block.terminator().source())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let zero = targets.values()[0].edge().target().index();
    let nonzero = targets.otherwise().target().index();
    if zero == nonzero
        || [zero, nonzero]
            .into_iter()
            .any(|block| block == entry || block as usize >= helper.blocks().len())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    let join = semantic_goto_target_v3(helper, zero)?;
    if semantic_goto_target_v3(helper, nonzero)? != join
        || [entry, zero, nonzero].contains(&join)
        || join as usize >= helper.blocks().len()
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    validate_semantic_copy_arm_v3(helper, zero, return_local, xor_local, join)?;
    let fallback = validate_semantic_constant_arm_v3(
        owner.semantic().semantic().types(),
        helper,
        nonzero,
        return_local,
        join,
    )?;
    let join_block = &helper.blocks()[join as usize];
    if !join_block.statements().is_empty()
        || !matches!(
            join_block.terminator().kind(),
            SemanticTerminatorKindV1::Return
        )
        || !source_available_v3(join_block.terminator().source())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    Ok(SemanticHelperObservationV3 {
        fallback,
        values: [
            left_local.index(),
            right_local.index(),
            xor_local.index(),
            return_local.index(),
        ],
        blocks: [entry, zero, nonzero, join],
    })
}

fn validate_kir_xor_helper_v3(
    module: &Module,
    id: &FunctionId,
    correspondence: &SemanticKirCorrespondenceV1,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    semantic: SemanticHelperObservationV3,
) -> Result<[ValueId; 5], MemoryTraceRefinementErrorV3> {
    let helper = find_function_v3(module, id)?;
    let body = helper
        .body
        .as_ref()
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    if helper.role != FunctionRole::InternalHelper
        || helper.signature.parameters.as_slice()
            != [Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U32)]
        || helper.signature.results.as_slice() != [Type::Scalar(ScalarType::U32)]
        || body.parameters.len() != 2
        || body.blocks.len() != 4
    {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    }
    let left = exact_parameter_binding_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.values[0],
    )?;
    let right = exact_parameter_binding_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.values[1],
    )?;
    if body.parameters.as_slice() != [left, right] {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    }
    let blocks = semantic
        .blocks
        .map(|block| {
            exact_block_binding_v3(
                correspondence,
                correspondence_owner,
                semantic_function,
                block,
            )
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let [entry_id, zero_id, nonzero_id, join_id] = blocks.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    };
    let entry = exact_block_v3(body, *entry_id)?;
    let zero = exact_block_v3(body, *zero_id)?;
    let nonzero = exact_block_v3(body, *nonzero_id)?;
    let join = exact_block_v3(body, *join_id)?;
    require_helper_statement_span_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.blocks[0],
        0,
        *entry_id,
        0,
        1,
    )?;
    require_helper_terminator_span_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.blocks[0],
        *entry_id,
        1,
        0,
    )?;
    require_helper_statement_span_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.blocks[1],
        0,
        *zero_id,
        0,
        0,
    )?;
    require_helper_terminator_span_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.blocks[1],
        *zero_id,
        0,
        0,
    )?;
    require_helper_statement_span_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.blocks[2],
        0,
        *nonzero_id,
        0,
        1,
    )?;
    require_helper_terminator_span_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.blocks[2],
        *nonzero_id,
        1,
        0,
    )?;
    require_helper_terminator_span_v3(
        correspondence,
        correspondence_owner,
        semantic_function,
        semantic.blocks[3],
        *join_id,
        0,
        0,
    )?;
    let [xor] = entry.operations.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    let [xor_result] = xor.results.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    if xor_result.ty != Type::Scalar(ScalarType::U32)
        || !xor.memory_effects().is_empty()
        || !matches!(xor.kind, OperationKind::Binary { op: BinaryOp::BitXor, lhs, rhs }
            if lhs == left && rhs == right)
        || !entry.parameters.is_empty()
        || !matches!(entry.terminator.as_ref(), Some(Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        }) if *selector == xor_result.id
            && cases.len() == 1
            && cases[0].value == 0
            && cases[0].target == *zero_id
            && cases[0].arguments.is_empty()
            && *default_target == *nonzero_id
            && default_arguments.is_empty())
    {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    }
    let [fallback_operation] = nonzero.operations.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    let [fallback_value] = fallback_operation.results.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    if fallback_operation.kind != OperationKind::Constant(Constant::U32(semantic.fallback))
        || fallback_value.ty != Type::Scalar(ScalarType::U32)
        || !fallback_operation.memory_effects().is_empty()
        || !zero.parameters.is_empty()
        || !zero.operations.is_empty()
        || !nonzero.parameters.is_empty()
        || !matches!(zero.terminator.as_ref(), Some(Terminator::Branch { target, arguments })
            if *target == *join_id && arguments.as_slice() == [xor_result.id])
        || !matches!(nonzero.terminator.as_ref(), Some(Terminator::Branch { target, arguments })
            if *target == *join_id && arguments.as_slice() == [fallback_value.id])
    {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    }
    let [join_parameter] = join.parameters.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    if join_parameter.ty != Type::Scalar(ScalarType::U32)
        || !join.operations.is_empty()
        || !matches!(join.terminator.as_ref(), Some(Terminator::Return { values })
            if values.as_slice() == [join_parameter.id])
    {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    }
    Ok([
        left,
        right,
        xor_result.id,
        fallback_value.id,
        join_parameter.id,
    ])
}

fn unique_local_with_role_v3(
    function: &SemanticFunctionDeclV1,
    role: SemanticLocalRoleV1,
) -> Result<SemanticLocalIdV1, MemoryTraceRefinementErrorV3> {
    let matches = function
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| local.role() == role)
        .map(|(index, _)| SemanticLocalIdV1::from_index(index as u32))
        .collect::<Vec<_>>();
    let [local] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    Ok(*local)
}

fn semantic_goto_target_v3(
    function: &SemanticFunctionDeclV1,
    block: u32,
) -> Result<u32, MemoryTraceRefinementErrorV3> {
    match function
        .blocks()
        .get(block as usize)
        .map(|block| block.terminator().kind())
    {
        Some(SemanticTerminatorKindV1::Goto(edge))
            if edge.role() == SemanticEdgeRoleV1::Goto
                && source_available_v3(function.blocks()[block as usize].terminator().source()) =>
        {
            Ok(edge.target().index())
        }
        _ => Err(MemoryTraceRefinementErrorV3::SemanticShape),
    }
}

fn validate_semantic_copy_arm_v3(
    function: &SemanticFunctionDeclV1,
    block: u32,
    destination: SemanticLocalIdV1,
    source: SemanticLocalIdV1,
    join: u32,
) -> Result<(), MemoryTraceRefinementErrorV3> {
    let [statement] = function.blocks()[block as usize].statements() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    if assignment.destination().local() != destination
        || !assignment.destination().projections().is_empty()
        || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(operand)
            if operand_direct_local_v3(operand) == Some(source))
        || semantic_goto_target_v3(function, block)? != join
        || !source_available_v3(statement.source())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    Ok(())
}

fn validate_semantic_constant_arm_v3(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    block: u32,
    destination: SemanticLocalIdV1,
    join: u32,
) -> Result<u32, MemoryTraceRefinementErrorV3> {
    let [statement] = function.blocks()[block as usize].statements() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) =
        assignment.value().kind()
    else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    let SemanticConstantValueV1::Scalar(value) = constant.value() else {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    };
    if assignment.destination().local() != destination
        || !assignment.destination().projections().is_empty()
        || !matches!(
            types
                .get(constant.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }))
        )
        || value.size_bytes() != 4
        || semantic_goto_target_v3(function, block)? != join
        || !source_available_v3(statement.source())
    {
        return Err(MemoryTraceRefinementErrorV3::SemanticShape);
    }
    u32::try_from(value.bits()).map_err(|_| MemoryTraceRefinementErrorV3::SemanticShape)
}

fn exact_parameter_binding_v3(
    correspondence: &SemanticKirCorrespondenceV1,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    semantic_local: u32,
) -> Result<ValueId, MemoryTraceRefinementErrorV3> {
    let matches = correspondence
        .parameter_bindings()
        .iter()
        .filter(|binding| {
            binding.correspondence_owner() == correspondence_owner
                && binding.semantic_function() == semantic_function
                && binding.semantic_local().index() == semantic_local
        })
        .map(|binding| binding.kernel_ir_value())
        .collect::<Vec<_>>();
    let [value] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    };
    Ok(*value)
}

#[allow(clippy::too_many_arguments)]
fn require_helper_statement_span_v3(
    correspondence: &SemanticKirCorrespondenceV1,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: u32,
    statement: u32,
    kir_block: BlockId,
    first_operation: u32,
    operation_count: u32,
) -> Result<(), MemoryTraceRefinementErrorV3> {
    (correspondence
        .statement_operation_spans()
        .iter()
        .filter(|span| {
            span.correspondence_owner() == owner
                && span.semantic_function() == function
                && span.semantic_block().index() == block
                && span.statement_ordinal() == statement
                && span.kernel_ir_block() == kir_block
                && span.first_operation_ordinal() == first_operation
                && span.operation_count() == operation_count
        })
        .count()
        == 1)
        .then_some(())
        .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)
}

#[allow(clippy::too_many_arguments)]
fn require_helper_terminator_span_v3(
    correspondence: &SemanticKirCorrespondenceV1,
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: u32,
    kir_block: BlockId,
    first_operation: u32,
    operation_count: u32,
) -> Result<(), MemoryTraceRefinementErrorV3> {
    (correspondence
        .terminator_operation_spans()
        .iter()
        .filter(|span| {
            span.correspondence_owner() == owner
                && span.semantic_function() == function
                && span.semantic_block().index() == block
                && span.kernel_ir_block() == kir_block
                && span.first_operation_ordinal() == first_operation
                && span.operation_count() == operation_count
        })
        .count()
        == 1)
        .then_some(())
        .ok_or(MemoryTraceRefinementErrorV3::CorrespondenceMismatch)
}

fn exact_block_binding_v3(
    correspondence: &SemanticKirCorrespondenceV1,
    correspondence_owner: SemanticFunctionIdV1,
    semantic_function: SemanticFunctionIdV1,
    semantic_block: u32,
) -> Result<BlockId, MemoryTraceRefinementErrorV3> {
    let matches = correspondence
        .blocks()
        .iter()
        .filter(|binding| {
            binding.correspondence_owner() == correspondence_owner
                && binding.semantic_function() == semantic_function
                && binding.semantic_block().index() == semantic_block
        })
        .map(|binding| binding.kernel_ir_block())
        .collect::<Vec<_>>();
    let [block] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::CorrespondenceMismatch);
    };
    Ok(*block)
}

fn exact_block_v3(
    body: &fe2o3_kernel_ir::FunctionBody,
    id: BlockId,
) -> Result<&fe2o3_kernel_ir::BasicBlock, MemoryTraceRefinementErrorV3> {
    let matches = body
        .blocks
        .iter()
        .filter(|block| block.id == id)
        .collect::<Vec<_>>();
    let [block] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    Ok(*block)
}

fn is_u32_type_v3(
    owner: &ProductionSemanticKirOwnerV1,
    ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
) -> bool {
    matches!(
        owner
            .semantic()
            .semantic()
            .types()
            .get(ty.index() as usize)
            .map(|decl| decl.shape()),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }))
    )
}

fn operand_direct_local_v3(operand: &SemanticOperandV1) -> Option<SemanticLocalIdV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place.local())
        }
        SemanticOperandV1::Copy(_)
        | SemanticOperandV1::Move(_)
        | SemanticOperandV1::Constant(_) => None,
    }
}

fn indexed_parameter_pointer_v3(
    function: &Function,
    pointer: ValueId,
) -> Result<(u32, ValueId), MemoryTraceRefinementErrorV3> {
    let OperationKind::GetElementPointer { base, offset } = definition_v3(function, pointer)?.kind
    else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    let OperationKind::SliceData { slice } = definition_v3(function, base)?.kind else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    Ok((parameter_ordinal_v3(function, slice)?, offset))
}

fn parameter_ordinal_v3(
    function: &Function,
    value: ValueId,
) -> Result<u32, MemoryTraceRefinementErrorV3> {
    function
        .body
        .as_ref()
        .and_then(|body| {
            body.parameters
                .iter()
                .position(|parameter| *parameter == value)
        })
        .and_then(|index| u32::try_from(index).ok())
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)
}

fn definition_v3(
    function: &Function,
    value: ValueId,
) -> Result<&Operation, MemoryTraceRefinementErrorV3> {
    definition_at_v3(function, value).map(|(_, operation)| operation)
}

fn definition_at_v3(
    function: &Function,
    value: ValueId,
) -> Result<(FunctionOperationLocation, &Operation), MemoryTraceRefinementErrorV3> {
    let body = function
        .body
        .as_ref()
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)?;
    let matches = body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .map(move |(index, operation)| {
                    (FunctionOperationLocation::new(block.id, index), operation)
                })
        })
        .filter(|(_, operation)| operation.results.iter().any(|result| result.id == value))
        .collect::<Vec<_>>();
    let [definition] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::KirShape);
    };
    Ok(*definition)
}

fn operation_at_v3(
    function: &Function,
    location: FunctionOperationLocation,
) -> Result<&Operation, MemoryTraceRefinementErrorV3> {
    function
        .body
        .as_ref()
        .and_then(|body| body.blocks.iter().find(|block| block.id == location.block))
        .and_then(|block| block.operations.get(location.operation_index))
        .ok_or(MemoryTraceRefinementErrorV3::KirShape)
}

fn find_function_v3<'a>(
    module: &'a Module,
    id: &FunctionId,
) -> Result<&'a Function, MemoryTraceRefinementErrorV3> {
    let matches = module
        .functions
        .iter()
        .filter(|function| &function.id == id)
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(MemoryTraceRefinementErrorV3::ProductionShape);
    };
    Ok(*function)
}

fn source_available_v3(
    source: fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
) -> bool {
    source.expansion().is_some() && source.call_site().is_some()
}

fn source_identity_v3(
    source: fe2o3_mir_model::semantic_mir_v1::SemanticSourceProvenanceV1,
) -> Result<[u8; 32], MemoryTraceRefinementErrorV3> {
    let expansion = source
        .expansion()
        .ok_or(MemoryTraceRefinementErrorV3::SourceIdentity)?;
    let call_site = source
        .call_site()
        .ok_or(MemoryTraceRefinementErrorV3::SourceIdentity)?;
    let mut digest = Sha256::new();
    digest.update(b"FE2O3/MEMORY-TRACE/SOURCE-SITE/V3\0");
    for origin in [expansion, call_site] {
        digest.update(origin.file().as_bytes());
        let (start, end) = origin.byte_range();
        digest.update(start.to_le_bytes());
        digest.update(end.to_le_bytes());
        let (line, column) = origin.start_coordinate();
        digest.update(line.to_le_bytes());
        digest.update(column.to_le_bytes());
        let (line, column) = origin.end_coordinate();
        digest.update(line.to_le_bytes());
        digest.update(column.to_le_bytes());
    }
    let identity: [u8; 32] = digest.finalize().into();
    if identity == [0; 32] {
        Err(MemoryTraceRefinementErrorV3::SourceIdentity)
    } else {
        Ok(identity)
    }
}

/// Returns the exact proof/model identity checked by production evidence.
pub fn memory_trace_refinement_model_identity_v3() -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(MODEL_DOMAIN_V3);
    digest.update(MEMORY_TRACE_REFINEMENT_MODEL_VERSION_V3.to_le_bytes());
    digest.update(MEMORY_TRACE_REFINEMENT_THEOREM_V3.as_bytes());
    digest.update(MEMORY_TRACE_REFINEMENT_PROOF_SHA256_V3);
    digest.update(MEMORY_TRACE_REFINEMENT_VERUS_SHA256_V3);
    digest.update(MEMORY_TRACE_REFINEMENT_CLOSURE_SHA256_V3);
    digest.update(U32_BYTES_V3.to_le_bytes());
    digest.update([2, 1, 1]); // two reads, one XOR call, one write
    digest.update([3, 3, 1]); // three extents, predicates, and ordered short-circuit chain
    digest.finalize().into()
}

fn evidence_identity_v3(evidence: &ProductionMemoryTraceEvidenceV3) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(EVIDENCE_DOMAIN_V3);
    digest.update(evidence.model_identity);
    digest.update(evidence.semantic_mir_sha256);
    digest.update(evidence.canonical_kernel_ir.digest());
    digest.update(
        evidence
            .canonical_kernel_ir
            .canonical_length()
            .to_le_bytes(),
    );
    for value in [
        evidence.selector.root_function,
        evidence.selector.enabled_block,
        evidence.selector.first_load.0,
        evidence.selector.first_load.1,
        evidence.selector.second_load.0,
        evidence.selector.second_load.1,
        evidence.selector.helper_call_block,
        evidence.selector.store.0,
        evidence.selector.store.1,
        evidence.selector.helper_function,
    ] {
        digest.update(value.to_le_bytes());
    }
    for block in evidence.selector.guard_blocks {
        digest.update(block.to_le_bytes());
    }
    for parameter in evidence.parameters {
        digest.update(parameter.to_le_bytes());
    }
    digest.update(evidence.gid.0.to_le_bytes());
    for value in evidence.length_values {
        digest.update(value.0.to_le_bytes());
    }
    for location in evidence.guard_locations {
        digest.update(location.block.0.to_le_bytes());
        digest.update((location.operation_index as u64).to_le_bytes());
    }
    for location in evidence.memory_locations {
        digest.update(location.block.0.to_le_bytes());
        digest.update((location.operation_index as u64).to_le_bytes());
    }
    digest.update(evidence.root_call_location.block.0.to_le_bytes());
    digest.update((evidence.root_call_location.operation_index as u64).to_le_bytes());
    digest.update(evidence.fallback.to_le_bytes());
    for value in evidence.semantic_root_values {
        digest.update(value.to_le_bytes());
    }
    for value in evidence.semantic_helper_values {
        digest.update(value.to_le_bytes());
    }
    for value in evidence.kir_root_values {
        digest.update(value.0.to_le_bytes());
    }
    for value in evidence.kir_helper_values {
        digest.update(value.0.to_le_bytes());
    }
    for source in evidence.source_site_sha256 {
        digest.update(source);
    }
    digest.finalize().into()
}

/// Fail-closed errors from the bounded model and production checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MemoryTraceRefinementErrorV3 {
    /// The runtime allocation roster is not exactly three valid identities.
    AllocationRoster,
    /// Two runtime allocation address ranges overlap.
    OverlappingAllocations,
    /// The requested formal parameter is absent.
    UnknownAllocation,
    /// The address carries the wrong runtime allocation provenance.
    ProvenanceMismatch,
    /// Address arithmetic overflowed.
    AddressOverflow,
    /// A typed access is not four-byte aligned.
    MisalignedAccess,
    /// A typed access lies outside its allocation.
    OutOfBounds,
    /// The output allocation is immutable.
    ImmutableStore,
    /// Executable source, MIR, and KIR observations differ.
    ObservationMismatch,
    /// The production owner failed revalidation.
    LiveOwner(String),
    /// The helper arguments/result do not close the XOR/diamond boundary.
    HelperResultMismatch,
    /// The module/kernel roster is outside the bounded fragment.
    ProductionShape,
    /// The semantic MIR is outside the bounded fragment.
    SemanticShape,
    /// The KIR is outside the bounded fragment.
    KirShape,
    /// Source spans are unavailable or invalid.
    SourceIdentity,
    /// Semantic/KIR operation correspondence is not exact.
    CorrespondenceMismatch,
    /// The gid bounds guard does not dominate or bind every access.
    GuardMismatch,
    /// Formal allocation/range/alignment obligations differ.
    FormalObligationMismatch,
    /// Retained evidence or one of its model pins changed.
    EvidenceMismatch,
    /// More than one exact bounded fragment survived live validation.
    AmbiguousSelector,
}

impl fmt::Display for MemoryTraceRefinementErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationRoster => formatter.write_str("invalid three-allocation roster"),
            Self::OverlappingAllocations => formatter.write_str("allocation byte ranges overlap"),
            Self::UnknownAllocation => formatter.write_str("unknown formal allocation"),
            Self::ProvenanceMismatch => formatter.write_str("allocation provenance mismatch"),
            Self::AddressOverflow => formatter.write_str("byte-address arithmetic overflow"),
            Self::MisalignedAccess => formatter.write_str("misaligned typed u32 access"),
            Self::OutOfBounds => formatter.write_str("typed u32 access is out of bounds"),
            Self::ImmutableStore => formatter.write_str("store targets an immutable allocation"),
            Self::ObservationMismatch => {
                formatter.write_str("source, MIR, and KIR memory observations differ")
            }
            Self::LiveOwner(error) => write!(formatter, "live production owner changed: {error}"),
            Self::HelperResultMismatch => {
                formatter.write_str("structured helper call-result relation mismatch")
            }
            Self::ProductionShape => formatter.write_str("unsupported production kernel shape"),
            Self::SemanticShape => formatter.write_str("unsupported semantic MIR memory shape"),
            Self::KirShape => formatter.write_str("unsupported KIR memory shape"),
            Self::SourceIdentity => formatter.write_str("source identity is unavailable"),
            Self::CorrespondenceMismatch => {
                formatter.write_str("semantic-MIR/KIR correspondence mismatch")
            }
            Self::GuardMismatch => formatter.write_str("gid bounds guard mismatch"),
            Self::FormalObligationMismatch => {
                formatter.write_str("formal memory obligation mismatch")
            }
            Self::EvidenceMismatch => formatter.write_str("memory refinement evidence changed"),
            Self::AmbiguousSelector => {
                formatter.write_str("multiple guarded memory fragments are eligible")
            }
        }
    }
}

impl Error for MemoryTraceRefinementErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn allocation(
        parameter: u32,
        provenance: u8,
        base: u64,
        mutable: bool,
        words: [u32; 2],
    ) -> MemoryAllocationV3 {
        MemoryAllocationV3::new(
            parameter,
            [provenance; 32],
            base,
            4,
            mutable,
            words
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
    }

    fn memory() -> ByteMemoryV3 {
        ByteMemoryV3::try_new(vec![
            allocation(0, 1, 0x1000, false, [0x0102_0304, 0xa5a5_5a5a]),
            allocation(1, 2, 0x2000, false, [0x1111_2222, 0x0f0f_f0f0]),
            allocation(2, 3, 0x3000, true, [0, 0]),
        ])
        .unwrap()
    }

    fn lane(enabled: bool) -> GuardedMemoryLaneV3 {
        GuardedMemoryLaneV3 {
            enabled,
            invocation_index: 1,
            first_input_provenance: [1; 32],
            second_input_provenance: [2; 32],
            output_provenance: [3; 32],
            first_input_parameter: 0,
            second_input_parameter: 1,
            output_parameter: 2,
        }
    }

    fn helper() -> RelatedHelperResultsV3 {
        RelatedHelperResultsV3::try_new(0xa5a5_5a5a, 0x0f0f_f0f0, 17, 17, 17, 17).unwrap()
    }

    fn guard_function(slice_parameter: ValueId, predicate: ComparePredicate) -> Function {
        let mut block = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::effect_free(
                fe2o3_kernel_ir::ValueDef::new(ValueId(10), Type::INDEX),
                OperationKind::SliceLength {
                    slice: slice_parameter,
                },
            ),
            Operation::effect_free(
                fe2o3_kernel_ir::ValueDef::new(ValueId(11), Type::BOOL),
                OperationKind::Compare {
                    predicate,
                    lhs: ValueId(3),
                    rhs: ValueId(10),
                },
            ),
        ];
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        Function::kernel_entry(
            "guard",
            fe2o3_kernel_ir::Signature::new(vec![Type::Unit; 4], Vec::new()),
            vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
            vec![block],
        )
    }

    #[test]
    fn executable_source_mir_kir_memory_observations_match() {
        let observation =
            check_guarded_memory_refinement_v3(memory(), lane(true), helper()).unwrap();
        assert_eq!(observation.result, Some(17));
        assert_eq!(observation.trace.len(), 3);
        assert_eq!(
            &observation.memory.allocations[&2].bytes[4..8],
            &17_u32.to_le_bytes(),
        );
        assert!(
            !memory_trace_refinement_model_identity_v3()
                .iter()
                .all(|byte| *byte == 0)
        );
    }

    #[test]
    fn false_guard_has_no_trace_and_preserves_every_byte() {
        let before = memory();
        let mut disabled = lane(false);
        disabled.invocation_index = 2;
        let observation =
            check_guarded_memory_refinement_v3(before.clone(), disabled, helper()).unwrap();
        assert_eq!(observation.memory, before);
        assert_eq!(observation.result, None);
        assert!(observation.trace.is_empty());
    }

    #[test]
    fn hostile_provenance_alignment_range_mutability_and_overflow_fail_closed() {
        let mut wrong_provenance = lane(true);
        wrong_provenance.first_input_provenance = [9; 32];
        assert_eq!(
            check_guarded_memory_refinement_v3(memory(), wrong_provenance, helper()),
            Err(MemoryTraceRefinementErrorV3::ProvenanceMismatch),
        );

        let misaligned = ByteMemoryV3::try_new(vec![
            allocation(0, 1, 0x1002, false, [1, 2]),
            allocation(1, 2, 0x2000, false, [3, 4]),
            allocation(2, 3, 0x3000, true, [0, 0]),
        ]);
        assert_eq!(
            misaligned,
            Err(MemoryTraceRefinementErrorV3::AllocationRoster)
        );

        let mut out_of_range = lane(true);
        out_of_range.invocation_index = 2;
        assert_eq!(
            check_guarded_memory_refinement_v3(memory(), out_of_range, helper()),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );

        let immutable = ByteMemoryV3::try_new(vec![
            allocation(0, 1, 0x1000, false, [1, 2]),
            allocation(1, 2, 0x2000, false, [3, 4]),
            allocation(2, 3, 0x3000, false, [0, 0]),
        ])
        .unwrap();
        assert_eq!(
            check_guarded_memory_refinement_v3(immutable, lane(true), helper()),
            Err(MemoryTraceRefinementErrorV3::AllocationRoster),
        );

        let mutable_input = ByteMemoryV3::try_new(vec![
            allocation(0, 1, 0x1000, true, [1, 2]),
            allocation(1, 2, 0x2000, false, [3, 4]),
            allocation(2, 3, 0x3000, true, [0, 0]),
        ])
        .unwrap();
        assert_eq!(
            check_guarded_memory_refinement_v3(mutable_input, lane(true), helper()),
            Err(MemoryTraceRefinementErrorV3::AllocationRoster),
        );

        let mut overflow = lane(true);
        overflow.invocation_index = u64::MAX;
        assert_eq!(
            check_guarded_memory_refinement_v3(memory(), overflow, helper()),
            Err(MemoryTraceRefinementErrorV3::AddressOverflow),
        );
    }

    #[test]
    fn overlapping_and_reused_allocation_identities_are_rejected() {
        assert_eq!(
            ByteMemoryV3::try_new(vec![
                allocation(0, 1, 0x1000, false, [1, 2]),
                allocation(1, 2, 0x1004, false, [3, 4]),
                allocation(2, 3, 0x3000, true, [0, 0]),
            ]),
            Err(MemoryTraceRefinementErrorV3::OverlappingAllocations),
        );
        assert_eq!(
            ByteMemoryV3::try_new(vec![
                allocation(0, 1, 0x1000, false, [1, 2]),
                allocation(1, 1, 0x2000, false, [3, 4]),
                allocation(2, 3, 0x3000, true, [0, 0]),
            ]),
            Err(MemoryTraceRefinementErrorV3::AllocationRoster),
        );
    }

    #[test]
    fn substituted_helper_call_destination_or_kir_result_is_rejected() {
        assert_eq!(
            RelatedHelperResultsV3::try_new(1, 2, 17, 17, 19, 17),
            Err(MemoryTraceRefinementErrorV3::HelperResultMismatch),
        );
        assert_eq!(
            RelatedHelperResultsV3::try_new(1, 2, 17, 17, 17, 23),
            Err(MemoryTraceRefinementErrorV3::HelperResultMismatch),
        );
    }

    #[test]
    fn guard_lookup_binds_exact_ordered_slice_parameter_and_opcode() {
        let first = guard_function(ValueId(0), ComparePredicate::LessThan);
        assert_eq!(
            guarded_length_parameter_v3(&first, ValueId(11), ValueId(3)),
            Ok((
                0,
                ValueId(10),
                FunctionOperationLocation::new(BlockId(0), 1),
            )),
        );

        let swapped = guard_function(ValueId(1), ComparePredicate::LessThan);
        assert_eq!(
            guarded_length_parameter_v3(&swapped, ValueId(11), ValueId(3)),
            Ok((
                1,
                ValueId(10),
                FunctionOperationLocation::new(BlockId(0), 1),
            )),
        );
        assert_eq!(
            guarded_length_parameter_v3(&first, ValueId(11), ValueId(2)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );

        let wrong_opcode = guard_function(ValueId(0), ComparePredicate::Equal);
        assert_eq!(
            guarded_length_parameter_v3(&wrong_opcode, ValueId(11), ValueId(3)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );
    }

    #[test]
    fn false_guard_path_rejects_effects_and_reentry_to_enabled_region() {
        let mut false_exit = fe2o3_kernel_ir::BasicBlock::new(BlockId(1));
        false_exit.terminator = Some(Terminator::Return { values: Vec::new() });
        let mut enabled = fe2o3_kernel_ir::BasicBlock::new(BlockId(2));
        enabled.terminator = Some(Terminator::Return { values: Vec::new() });
        let clean = Function::kernel_entry(
            "false_path",
            fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![false_exit.clone(), enabled.clone()],
        );
        assert_eq!(
            validate_no_memory_effect_path_v3(&clean, BlockId(1), BlockId(2)),
            Ok(BTreeSet::from([BlockId(1)])),
        );

        false_exit.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(20),
                value: ValueId(21),
                access: fe2o3_kernel_ir::MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        let effectful = Function::kernel_entry(
            "effectful_false_path",
            fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![false_exit, enabled.clone()],
        );
        assert_eq!(
            validate_no_memory_effect_path_v3(&effectful, BlockId(1), BlockId(2)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );

        let mut reenter = fe2o3_kernel_ir::BasicBlock::new(BlockId(1));
        reenter.terminator = Some(Terminator::Branch {
            target: BlockId(2),
            arguments: Vec::new(),
        });
        let reentering = Function::kernel_entry(
            "reentering_false_path",
            fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![reenter, enabled.clone()],
        );
        assert_eq!(
            validate_no_memory_effect_path_v3(&reentering, BlockId(1), BlockId(2)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );

        let mut cycle = fe2o3_kernel_ir::BasicBlock::new(BlockId(1));
        cycle.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: Vec::new(),
        });
        let cycling = Function::kernel_entry(
            "cycling_false_path",
            fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![cycle, fe2o3_kernel_ir::BasicBlock::new(BlockId(2))],
        );
        assert_eq!(
            validate_no_memory_effect_path_v3(&cycling, BlockId(1), BlockId(2)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );

        let mut diverging_call = fe2o3_kernel_ir::BasicBlock::new(BlockId(1));
        diverging_call.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Call {
                callee: FunctionId::new("unknown_empty_effect_call"),
                arguments: Vec::new(),
            },
        ));
        diverging_call.terminator = Some(Terminator::Return { values: Vec::new() });
        let calling = Function::kernel_entry(
            "calling_false_path",
            fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![diverging_call, enabled.clone()],
        );
        assert_eq!(
            validate_no_memory_effect_path_v3(&calling, BlockId(1), BlockId(2)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );

        let mut unreachable = fe2o3_kernel_ir::BasicBlock::new(BlockId(1));
        unreachable.terminator = Some(Terminator::Unreachable);
        let unreachable_path = Function::kernel_entry(
            "unreachable_false_path",
            fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![unreachable, enabled],
        );
        assert_eq!(
            validate_no_memory_effect_path_v3(&unreachable_path, BlockId(1), BlockId(2)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );
    }

    #[test]
    fn guard_chain_cannot_be_bypassed_from_declared_entry() {
        let mut bypass = fe2o3_kernel_ir::BasicBlock::new(BlockId(9));
        bypass.terminator = Some(Terminator::Branch {
            target: BlockId(2),
            arguments: Vec::new(),
        });
        let mut guard = fe2o3_kernel_ir::BasicBlock::new(BlockId(0));
        guard.terminator = Some(Terminator::Return { values: Vec::new() });
        let function = Function::kernel_entry(
            "bypass",
            fe2o3_kernel_ir::Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![bypass, guard],
        );
        assert_eq!(
            validate_declared_entry_guard_v3(&function, BlockId(0)),
            Err(MemoryTraceRefinementErrorV3::GuardMismatch),
        );
        assert_eq!(
            validate_declared_entry_guard_v3(&function, BlockId(9)),
            Ok(()),
        );
    }
}
