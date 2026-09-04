//! Checked structural classification of authenticated gfx942 atomic and collective machine sites.
//!
//! The input is the move-only result of the authenticated LLVM/MC worker. This
//! layer retains that receipt while binding one exact HSACO payload, selected
//! entry, descriptor, entry bytes, instruction bytes, and closed call graph to
//! a conservative atomic/collective primitive roster. It is translation
//! validation evidence only: opcode names and MC properties are classified,
//! but instruction semantics, source/compiler refinement, memory ordering,
//! convergence, data-race freedom, and hardware behavior are not proved.

use crate::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1,
    AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1, PhysicalMachineDescriptorIdentityV1,
    PhysicalMachineInstructionTraceV1, PhysicalMachineMemoryAccessV1,
    PhysicalMachinePayloadIdentityV1, PhysicalMachineTargetV1,
    PhysicalMachineTraceEvidenceIdentityV1,
};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const RECEIPT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/GFX942-ATOMIC-COLLECTIVE-MACHINE-STRUCTURE/V1\0";

/// Storage segment structurally named by one atomic machine instruction.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942MachineAtomicStorageV1 {
    Global,
    Workgroup,
}

/// Integer atomic operation decoded from one closed gfx942 opcode family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942MachineAtomicOperationV1 {
    Swap,
    CompareExchange,
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchOr,
    FetchXor,
    FetchMinSigned,
    FetchMinUnsigned,
    FetchMaxSigned,
    FetchMaxUnsigned,
}

/// Atomic instruction properties retained from authenticated LLVM/MC output.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Gfx942MachineAtomicPrimitiveV1 {
    operation: Gfx942MachineAtomicOperationV1,
    storage: Gfx942MachineAtomicStorageV1,
    width_bits: u16,
}

impl Gfx942MachineAtomicPrimitiveV1 {
    pub const fn operation(self) -> Gfx942MachineAtomicOperationV1 {
        self.operation
    }

    pub const fn storage(self) -> Gfx942MachineAtomicStorageV1 {
        self.storage
    }

    pub const fn width_bits(self) -> u16 {
        self.width_bits
    }
}

/// Low-level building block used by the bounded gfx942 collective lowerings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942MachineCollectivePrimitiveV1 {
    LdsRead32,
    LdsWrite32,
    LdsPermute32,
    WorkgroupBarrier,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Gfx942MachinePrimitiveClassV1 {
    Atomic(Gfx942MachineAtomicPrimitiveV1),
    Collective(Gfx942MachineCollectivePrimitiveV1),
}

/// One exact instruction site classified from authenticated machine evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Gfx942MachineStructureSiteV1 {
    function_symbol: String,
    instruction_offset: u64,
    opcode: String,
    encoding_sha256: [u8; 32],
    encoding_byte_len: u16,
    primitive: Gfx942MachinePrimitiveClassV1,
}

impl Gfx942MachineStructureSiteV1 {
    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }

    pub const fn instruction_offset(&self) -> u64 {
        self.instruction_offset
    }

    pub fn opcode(&self) -> &str {
        &self.opcode
    }

    pub const fn encoding_sha256(&self) -> [u8; 32] {
        self.encoding_sha256
    }

    pub const fn encoding_byte_len(&self) -> u16 {
        self.encoding_byte_len
    }

    pub const fn primitive(&self) -> Gfx942MachinePrimitiveClassV1 {
        self.primitive
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CheckedGfx942MachineStructureIdentityV1([u8; 32]);

impl CheckedGfx942MachineStructureIdentityV1 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Move-only checked machine-structure receipt for one selected entry.
pub struct CheckedGfx942AtomicCollectiveMachineStructureV1 {
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: String,
    descriptor_symbol: String,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    entry_sha256: [u8; 32],
    entry_file_offset: u64,
    entry_byte_len: u64,
    sites: Vec<Gfx942MachineStructureSiteV1>,
    identity: CheckedGfx942MachineStructureIdentityV1,
}

impl core::fmt::Debug for CheckedGfx942AtomicCollectiveMachineStructureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("CheckedGfx942AtomicCollectiveMachineStructureV1")
            .field("kernel_symbol", &self.kernel_symbol)
            .field("descriptor_symbol", &self.descriptor_symbol)
            .field("artifact", &self.artifact_identity())
            .field("entry_file_offset", &self.entry_file_offset)
            .field("entry_byte_len", &self.entry_byte_len)
            .field("site_count", &self.sites.len())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl CheckedGfx942AtomicCollectiveMachineStructureV1 {
    pub const fn target(&self) -> PhysicalMachineTargetV1 {
        PhysicalMachineTargetV1::Gfx942XnackMinusCov6
    }

    pub const fn artifact_identity(&self) -> PhysicalMachinePayloadIdentityV1 {
        self.execution.request().payload_identity()
    }

    pub fn kernel_symbol(&self) -> &str {
        &self.kernel_symbol
    }

    pub fn descriptor_symbol(&self) -> &str {
        &self.descriptor_symbol
    }

    pub const fn descriptor_identity(&self) -> PhysicalMachineDescriptorIdentityV1 {
        self.descriptor_identity
    }

    pub const fn entry_sha256(&self) -> [u8; 32] {
        self.entry_sha256
    }

    pub const fn entry_file_offset(&self) -> u64 {
        self.entry_file_offset
    }

    pub const fn entry_byte_len(&self) -> u64 {
        self.entry_byte_len
    }

    pub fn trace_identity(&self) -> PhysicalMachineTraceEvidenceIdentityV1 {
        self.execution.analysis().trace().identity()
    }

    pub fn sites(&self) -> &[Gfx942MachineStructureSiteV1] {
        &self.sites
    }

    pub const fn identity(&self) -> CheckedGfx942MachineStructureIdentityV1 {
        self.identity
    }

    pub fn authenticated_execution_identity(
        &self,
    ) -> AuthenticatedPhysicalMachineAnalysisReceiptIdentityV1 {
        self.execution.identity()
    }

    pub const fn authenticates_analyzer_execution(&self) -> bool {
        true
    }

    pub const fn binds_exact_payload_descriptor_entry_and_instruction_bytes(&self) -> bool {
        true
    }

    pub const fn establishes_machine_instruction_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_atomic_memory_ordering(&self) -> bool {
        false
    }

    pub const fn establishes_collective_convergence(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub fn into_authenticated_execution(self) -> AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.execution
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Gfx942MachineStructureErrorV1 {
    InvalidKernelSymbol,
    KernelNotRequested,
    AmbiguousKernelEntry,
    EntryRangeInvalid,
    CallGraphInvalid,
    UnsupportedAtomicOpcode { opcode: String },
    UnsupportedCollectiveOpcode { opcode: String },
    AtomicMemoryClassification { opcode: String },
    CollectiveMemoryClassification { opcode: String },
    NoAtomicOrCollectiveMachineSites,
    DuplicateMachineSite,
}

impl core::fmt::Display for Gfx942MachineStructureErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            formatter,
            "invalid gfx942 atomic/collective machine structure: {self:?}"
        )
    }
}

impl std::error::Error for Gfx942MachineStructureErrorV1 {}

/// Failure that returns custody of the authenticated analyzer execution.
pub struct Gfx942MachineStructureFailureV1 {
    execution: Box<AuthenticatedPhysicalMachineAnalysisExecutionV1>,
    error: Gfx942MachineStructureErrorV1,
}

impl core::fmt::Debug for Gfx942MachineStructureFailureV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Gfx942MachineStructureFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl Gfx942MachineStructureFailureV1 {
    pub const fn error(&self) -> &Gfx942MachineStructureErrorV1 {
        &self.error
    }

    pub fn into_parts(
        self,
    ) -> (
        AuthenticatedPhysicalMachineAnalysisExecutionV1,
        Gfx942MachineStructureErrorV1,
    ) {
        (*self.execution, self.error)
    }
}

/// Classifies the exact selected-entry closure retained by an authenticated worker execution.
///
/// This operation consumes the execution so success cannot detach the checked
/// roster from its authenticated receipt. Any unsupported atomic, DS, barrier,
/// or DPP family fails closed instead of disappearing from the roster.
pub fn check_authenticated_gfx942_atomic_collective_machine_structure_v1(
    execution: AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
) -> Result<CheckedGfx942AtomicCollectiveMachineStructureV1, Gfx942MachineStructureFailureV1> {
    match check_structure(&execution, kernel_symbol) {
        Ok(checked) => Ok(CheckedGfx942AtomicCollectiveMachineStructureV1 {
            execution,
            kernel_symbol: checked.kernel_symbol,
            descriptor_symbol: checked.descriptor_symbol,
            descriptor_identity: checked.descriptor_identity,
            entry_sha256: checked.entry_sha256,
            entry_file_offset: checked.entry_file_offset,
            entry_byte_len: checked.entry_byte_len,
            sites: checked.sites,
            identity: checked.identity,
        }),
        Err(error) => Err(Gfx942MachineStructureFailureV1 {
            execution: Box::new(execution),
            error,
        }),
    }
}

struct CheckedParts {
    kernel_symbol: String,
    descriptor_symbol: String,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    entry_sha256: [u8; 32],
    entry_file_offset: u64,
    entry_byte_len: u64,
    sites: Vec<Gfx942MachineStructureSiteV1>,
    identity: CheckedGfx942MachineStructureIdentityV1,
}

fn check_structure(
    execution: &AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
) -> Result<CheckedParts, Gfx942MachineStructureErrorV1> {
    if !valid_symbol(kernel_symbol) {
        return Err(Gfx942MachineStructureErrorV1::InvalidKernelSymbol);
    }
    let request = execution.request();
    if request
        .entries()
        .iter()
        .filter(|entry| entry.symbol() == kernel_symbol)
        .count()
        != 1
    {
        return Err(Gfx942MachineStructureErrorV1::KernelNotRequested);
    }
    let analysis = execution.analysis();
    let matching_entries = analysis
        .effects()
        .entry_points()
        .iter()
        .filter(|entry| entry.symbol() == kernel_symbol)
        .collect::<Vec<_>>();
    let [entry] = matching_entries.as_slice() else {
        return Err(Gfx942MachineStructureErrorV1::AmbiguousKernelEntry);
    };
    let entry_end = entry
        .code_offset()
        .checked_add(entry.code_size())
        .ok_or(Gfx942MachineStructureErrorV1::EntryRangeInvalid)?;
    let entry_start = usize::try_from(entry.code_offset())
        .map_err(|_| Gfx942MachineStructureErrorV1::EntryRangeInvalid)?;
    let entry_end =
        usize::try_from(entry_end).map_err(|_| Gfx942MachineStructureErrorV1::EntryRangeInvalid)?;
    let entry_bytes = request
        .exact_payload_bytes()
        .get(entry_start..entry_end)
        .ok_or(Gfx942MachineStructureErrorV1::EntryRangeInvalid)?;
    let entry_sha256 = Sha256::digest(entry_bytes).into();

    let functions = analysis
        .effects()
        .functions()
        .iter()
        .map(|function| (function.symbol(), function))
        .collect::<BTreeMap<_, _>>();
    let mut reachable = BTreeSet::new();
    let mut pending = vec![kernel_symbol];
    while let Some(symbol) = pending.pop() {
        if !reachable.insert(symbol) {
            continue;
        }
        let function = functions
            .get(symbol)
            .ok_or(Gfx942MachineStructureErrorV1::CallGraphInvalid)?;
        pending.extend(function.direct_callees().iter().map(String::as_str));
    }

    let mut sites = Vec::new();
    for instruction in analysis.trace().instructions() {
        if !reachable.contains(instruction.function_symbol()) {
            continue;
        }
        if let Some(primitive) = classify_instruction(instruction)? {
            sites.push(Gfx942MachineStructureSiteV1 {
                function_symbol: instruction.function_symbol().to_owned(),
                instruction_offset: instruction.instruction_offset(),
                opcode: instruction.opcode().to_owned(),
                encoding_sha256: Sha256::digest(instruction.encoding()).into(),
                encoding_byte_len: u16::try_from(instruction.encoding().len())
                    .expect("authenticated trace bounds each encoding to u16"),
                primitive,
            });
        }
    }
    if sites.is_empty() {
        return Err(Gfx942MachineStructureErrorV1::NoAtomicOrCollectiveMachineSites);
    }
    sites.sort_by(|left, right| {
        (&left.function_symbol, left.instruction_offset)
            .cmp(&(&right.function_symbol, right.instruction_offset))
    });
    if sites.windows(2).any(|pair| {
        pair[0].function_symbol == pair[1].function_symbol
            && pair[0].instruction_offset == pair[1].instruction_offset
    }) {
        return Err(Gfx942MachineStructureErrorV1::DuplicateMachineSite);
    }

    let descriptor_symbol = format!("{kernel_symbol}.kd");
    let identity = derive_identity(
        execution,
        kernel_symbol,
        &descriptor_symbol,
        entry.descriptor_identity(),
        entry_sha256,
        entry.code_offset(),
        entry.code_size(),
        &sites,
    );
    Ok(CheckedParts {
        kernel_symbol: kernel_symbol.to_owned(),
        descriptor_symbol,
        descriptor_identity: entry.descriptor_identity(),
        entry_sha256,
        entry_file_offset: entry.code_offset(),
        entry_byte_len: entry.code_size(),
        sites,
        identity,
    })
}

fn classify_instruction(
    instruction: &PhysicalMachineInstructionTraceV1,
) -> Result<Option<Gfx942MachinePrimitiveClassV1>, Gfx942MachineStructureErrorV1> {
    let opcode = instruction.opcode();
    if opcode.starts_with("GLOBAL_ATOMIC_") {
        let (operation, width_bits) = classify_global_atomic_opcode(opcode).ok_or_else(|| {
            Gfx942MachineStructureErrorV1::UnsupportedAtomicOpcode {
                opcode: opcode.to_owned(),
            }
        })?;
        if instruction.memory_access()
            != (PhysicalMachineMemoryAccessV1::ReadWrite {
                byte_width: width_bits / 8,
            })
        {
            return Err(Gfx942MachineStructureErrorV1::AtomicMemoryClassification {
                opcode: opcode.to_owned(),
            });
        }
        return Ok(Some(Gfx942MachinePrimitiveClassV1::Atomic(
            Gfx942MachineAtomicPrimitiveV1 {
                operation,
                storage: Gfx942MachineAtomicStorageV1::Global,
                width_bits,
            },
        )));
    }
    if opcode.starts_with("DS_") {
        if let Some((operation, width_bits)) = classify_ds_atomic_opcode(opcode) {
            if instruction.memory_access()
                != (PhysicalMachineMemoryAccessV1::WorkgroupReadWrite {
                    byte_width: width_bits / 8,
                })
            {
                return Err(Gfx942MachineStructureErrorV1::AtomicMemoryClassification {
                    opcode: opcode.to_owned(),
                });
            }
            return Ok(Some(Gfx942MachinePrimitiveClassV1::Atomic(
                Gfx942MachineAtomicPrimitiveV1 {
                    operation,
                    storage: Gfx942MachineAtomicStorageV1::Workgroup,
                    width_bits,
                },
            )));
        }
        let (primitive, valid_memory) = if matches!(opcode, "DS_READ_B32" | "DS_READ_B32_vi") {
            (
                Gfx942MachineCollectivePrimitiveV1::LdsRead32,
                instruction.memory_access()
                    == (PhysicalMachineMemoryAccessV1::WorkgroupRead { byte_width: 4 }),
            )
        } else if matches!(opcode, "DS_WRITE_B32" | "DS_WRITE_B32_vi") {
            (
                Gfx942MachineCollectivePrimitiveV1::LdsWrite32,
                instruction.memory_access()
                    == (PhysicalMachineMemoryAccessV1::WorkgroupWrite { byte_width: 4 }),
            )
        } else if matches!(
            opcode,
            "DS_BPERMUTE_B32"
                | "DS_BPERMUTE_B32_vi"
                | "DS_PERMUTE_B32"
                | "DS_PERMUTE_B32_vi"
                | "DS_SWIZZLE_B32"
                | "DS_SWIZZLE_B32_vi"
        ) {
            (
                Gfx942MachineCollectivePrimitiveV1::LdsPermute32,
                instruction.memory_access().is_workgroup()
                    && instruction.memory_access().byte_width() == 4,
            )
        } else {
            return Err(Gfx942MachineStructureErrorV1::UnsupportedCollectiveOpcode {
                opcode: opcode.to_owned(),
            });
        };
        if !valid_memory {
            return Err(
                Gfx942MachineStructureErrorV1::CollectiveMemoryClassification {
                    opcode: opcode.to_owned(),
                },
            );
        }
        return Ok(Some(Gfx942MachinePrimitiveClassV1::Collective(primitive)));
    }
    if opcode.starts_with("S_BARRIER") {
        if opcode != "S_BARRIER" && opcode != "S_BARRIER_vi" {
            return Err(Gfx942MachineStructureErrorV1::UnsupportedCollectiveOpcode {
                opcode: opcode.to_owned(),
            });
        }
        if instruction.memory_access() != PhysicalMachineMemoryAccessV1::None
            || !instruction.flags().is_barrier()
        {
            return Err(
                Gfx942MachineStructureErrorV1::CollectiveMemoryClassification {
                    opcode: opcode.to_owned(),
                },
            );
        }
        return Ok(Some(Gfx942MachinePrimitiveClassV1::Collective(
            Gfx942MachineCollectivePrimitiveV1::WorkgroupBarrier,
        )));
    }
    if opcode.contains("_DPP") {
        return Err(Gfx942MachineStructureErrorV1::UnsupportedCollectiveOpcode {
            opcode: opcode.to_owned(),
        });
    }
    if opcode.contains("ATOMIC") {
        return Err(Gfx942MachineStructureErrorV1::UnsupportedAtomicOpcode {
            opcode: opcode.to_owned(),
        });
    }
    Ok(None)
}

fn classify_global_atomic_opcode(opcode: &str) -> Option<(Gfx942MachineAtomicOperationV1, u16)> {
    let mut spelling = opcode.strip_prefix("GLOBAL_ATOMIC_")?;
    spelling = spelling.strip_suffix("_vi").unwrap_or(spelling);
    spelling = spelling.strip_suffix("_SADDR").unwrap_or(spelling);
    spelling = spelling.strip_suffix("_RTN").unwrap_or(spelling);
    let (operation, width_bits) = if let Some(operation) = spelling.strip_suffix("_X2") {
        (operation, 64)
    } else {
        (spelling, 32)
    };
    Some((classify_global_atomic_operation(operation)?, width_bits))
}

fn classify_global_atomic_operation(opcode: &str) -> Option<Gfx942MachineAtomicOperationV1> {
    Some(match opcode {
        "CMPSWAP" => Gfx942MachineAtomicOperationV1::CompareExchange,
        "SWAP" => Gfx942MachineAtomicOperationV1::Swap,
        "ADD" => Gfx942MachineAtomicOperationV1::FetchAdd,
        "SUB" => Gfx942MachineAtomicOperationV1::FetchSub,
        "AND" => Gfx942MachineAtomicOperationV1::FetchAnd,
        "OR" => Gfx942MachineAtomicOperationV1::FetchOr,
        "XOR" => Gfx942MachineAtomicOperationV1::FetchXor,
        "SMIN" => Gfx942MachineAtomicOperationV1::FetchMinSigned,
        "UMIN" => Gfx942MachineAtomicOperationV1::FetchMinUnsigned,
        "SMAX" => Gfx942MachineAtomicOperationV1::FetchMaxSigned,
        "UMAX" => Gfx942MachineAtomicOperationV1::FetchMaxUnsigned,
        _ => return None,
    })
}

fn classify_ds_atomic_opcode(opcode: &str) -> Option<(Gfx942MachineAtomicOperationV1, u16)> {
    let spelling = opcode.strip_prefix("DS_")?;
    let spelling = spelling.strip_suffix("_vi").unwrap_or(spelling);
    Some(match spelling {
        "CMPST_B32" | "CMPST_RTN_B32" => (Gfx942MachineAtomicOperationV1::CompareExchange, 32),
        "CMPST_B64" | "CMPST_RTN_B64" => (Gfx942MachineAtomicOperationV1::CompareExchange, 64),
        "WRXCHG_B32" | "WRXCHG_RTN_B32" => (Gfx942MachineAtomicOperationV1::Swap, 32),
        "WRXCHG_B64" | "WRXCHG_RTN_B64" => (Gfx942MachineAtomicOperationV1::Swap, 64),
        "ADD_U32" | "ADD_RTN_U32" => (Gfx942MachineAtomicOperationV1::FetchAdd, 32),
        "ADD_U64" | "ADD_RTN_U64" => (Gfx942MachineAtomicOperationV1::FetchAdd, 64),
        "SUB_U32" | "SUB_RTN_U32" | "RSUB_U32" | "RSUB_RTN_U32" => {
            (Gfx942MachineAtomicOperationV1::FetchSub, 32)
        }
        "SUB_U64" | "SUB_RTN_U64" | "RSUB_U64" | "RSUB_RTN_U64" => {
            (Gfx942MachineAtomicOperationV1::FetchSub, 64)
        }
        "AND_B32" | "AND_RTN_B32" => (Gfx942MachineAtomicOperationV1::FetchAnd, 32),
        "AND_B64" | "AND_RTN_B64" => (Gfx942MachineAtomicOperationV1::FetchAnd, 64),
        "OR_B32" | "OR_RTN_B32" => (Gfx942MachineAtomicOperationV1::FetchOr, 32),
        "OR_B64" | "OR_RTN_B64" => (Gfx942MachineAtomicOperationV1::FetchOr, 64),
        "XOR_B32" | "XOR_RTN_B32" => (Gfx942MachineAtomicOperationV1::FetchXor, 32),
        "XOR_B64" | "XOR_RTN_B64" => (Gfx942MachineAtomicOperationV1::FetchXor, 64),
        "MIN_I32" | "MIN_RTN_I32" => (Gfx942MachineAtomicOperationV1::FetchMinSigned, 32),
        "MIN_I64" | "MIN_RTN_I64" => (Gfx942MachineAtomicOperationV1::FetchMinSigned, 64),
        "MIN_U32" | "MIN_RTN_U32" => (Gfx942MachineAtomicOperationV1::FetchMinUnsigned, 32),
        "MIN_U64" | "MIN_RTN_U64" => (Gfx942MachineAtomicOperationV1::FetchMinUnsigned, 64),
        "MAX_I32" | "MAX_RTN_I32" => (Gfx942MachineAtomicOperationV1::FetchMaxSigned, 32),
        "MAX_I64" | "MAX_RTN_I64" => (Gfx942MachineAtomicOperationV1::FetchMaxSigned, 64),
        "MAX_U32" | "MAX_RTN_U32" => (Gfx942MachineAtomicOperationV1::FetchMaxUnsigned, 32),
        "MAX_U64" | "MAX_RTN_U64" => (Gfx942MachineAtomicOperationV1::FetchMaxUnsigned, 64),
        _ => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn derive_identity(
    execution: &AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel_symbol: &str,
    descriptor_symbol: &str,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    entry_sha256: [u8; 32],
    entry_file_offset: u64,
    entry_byte_len: u64,
    sites: &[Gfx942MachineStructureSiteV1],
) -> CheckedGfx942MachineStructureIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(RECEIPT_IDENTITY_DOMAIN_V1);
    let artifact = execution.request().payload_identity();
    digest.update(artifact.sha256());
    digest.update(artifact.byte_len().to_le_bytes());
    digest.update(execution.identity().sha256());
    digest.update(execution.identity().byte_len().to_le_bytes());
    digest.update(execution.analysis().trace().identity().sha256());
    digest.update(
        execution
            .analysis()
            .trace()
            .identity()
            .byte_len()
            .to_le_bytes(),
    );
    update_text(&mut digest, kernel_symbol);
    update_text(&mut digest, descriptor_symbol);
    digest.update(descriptor_identity.as_bytes());
    digest.update(entry_sha256);
    digest.update(entry_file_offset.to_le_bytes());
    digest.update(entry_byte_len.to_le_bytes());
    digest.update((sites.len() as u64).to_le_bytes());
    for site in sites {
        update_text(&mut digest, &site.function_symbol);
        digest.update(site.instruction_offset.to_le_bytes());
        update_text(&mut digest, &site.opcode);
        digest.update(site.encoding_sha256);
        digest.update(site.encoding_byte_len.to_le_bytes());
        encode_primitive(&mut digest, site.primitive);
    }
    CheckedGfx942MachineStructureIdentityV1(digest.finalize().into())
}

fn encode_primitive(digest: &mut Sha256, primitive: Gfx942MachinePrimitiveClassV1) {
    match primitive {
        Gfx942MachinePrimitiveClassV1::Atomic(atomic) => {
            digest.update([
                1,
                atomic_operation_tag(atomic.operation),
                atomic_storage_tag(atomic.storage),
            ]);
            digest.update(atomic.width_bits.to_le_bytes());
        }
        Gfx942MachinePrimitiveClassV1::Collective(collective) => {
            digest.update([2, collective_primitive_tag(collective)]);
        }
    }
}

const fn atomic_operation_tag(operation: Gfx942MachineAtomicOperationV1) -> u8 {
    match operation {
        Gfx942MachineAtomicOperationV1::Swap => 1,
        Gfx942MachineAtomicOperationV1::CompareExchange => 2,
        Gfx942MachineAtomicOperationV1::FetchAdd => 3,
        Gfx942MachineAtomicOperationV1::FetchSub => 4,
        Gfx942MachineAtomicOperationV1::FetchAnd => 5,
        Gfx942MachineAtomicOperationV1::FetchOr => 6,
        Gfx942MachineAtomicOperationV1::FetchXor => 7,
        Gfx942MachineAtomicOperationV1::FetchMinSigned => 8,
        Gfx942MachineAtomicOperationV1::FetchMinUnsigned => 9,
        Gfx942MachineAtomicOperationV1::FetchMaxSigned => 10,
        Gfx942MachineAtomicOperationV1::FetchMaxUnsigned => 11,
    }
}

const fn atomic_storage_tag(storage: Gfx942MachineAtomicStorageV1) -> u8 {
    match storage {
        Gfx942MachineAtomicStorageV1::Global => 1,
        Gfx942MachineAtomicStorageV1::Workgroup => 2,
    }
}

const fn collective_primitive_tag(primitive: Gfx942MachineCollectivePrimitiveV1) -> u8 {
    match primitive {
        Gfx942MachineCollectivePrimitiveV1::LdsRead32 => 1,
        Gfx942MachineCollectivePrimitiveV1::LdsWrite32 => 2,
        Gfx942MachineCollectivePrimitiveV1::LdsPermute32 => 3,
        Gfx942MachineCollectivePrimitiveV1::WorkgroupBarrier => 4,
    }
}

fn update_text(digest: &mut Sha256, text: &str) {
    digest.update((text.len() as u64).to_le_bytes());
    digest.update(text.as_bytes());
}

fn valid_symbol(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 256
        && (bytes[0].is_ascii_alphabetic() || matches!(bytes[0], b'_' | b'.' | b'$'))
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_atomic_suffix_grammar_is_closed() {
        for (opcode, width) in [
            ("GLOBAL_ATOMIC_ADD", 32),
            ("GLOBAL_ATOMIC_ADD_SADDR_vi", 32),
            ("GLOBAL_ATOMIC_ADD_RTN_vi", 32),
            ("GLOBAL_ATOMIC_ADD_X2_RTN_SADDR_vi", 64),
        ] {
            assert_eq!(
                classify_global_atomic_opcode(opcode).map(|(_, actual)| actual),
                Some(width)
            );
        }
        for opcode in [
            "GLOBAL_ATOMIC_ADD_FUTURE_vi",
            "GLOBAL_ATOMIC_ADD_SADDR_FUTURE_vi",
            "GLOBAL_ATOMIC_ADD_SADDR_RTN_vi",
            "GLOBAL_ATOMIC_ADD_vi_SADDR",
            "GLOBAL_ATOMIC_ADD_X2_X2_vi",
        ] {
            assert_eq!(classify_global_atomic_opcode(opcode), None);
        }
    }

    #[test]
    fn ds_atomic_suffix_grammar_is_closed() {
        assert_eq!(
            classify_ds_atomic_opcode("DS_ADD_U32_vi").map(|(_, width)| width),
            Some(32)
        );
        assert_eq!(
            classify_ds_atomic_opcode("DS_ADD_RTN_U64_vi").map(|(_, width)| width),
            Some(64)
        );
        for opcode in [
            "DS_ADD_FUTURE_vi",
            "DS_ADD_U32_FUTURE_vi",
            "DS_ADD_U32_vi_FUTURE",
        ] {
            assert_eq!(classify_ds_atomic_opcode(opcode), None);
        }
    }
}
