//! Bounded gfx942 machine-effect mechanics and inert evidence.
//!
//! This first slice analyzes a caller-supplied, canonical mechanics record. It
//! does not extract operations from LLVM IR or an HSACO, disassemble ISA, or
//! prove that the record refines the bound payload. Those joins require a
//! separately authenticated extractor and compiler-refinement argument.
//! Function operation records are straight-line, single-basic-block summaries;
//! V1 rejects branches and does not infer control-flow dominance.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

/// Canonical input schema consumed by the first gfx942 machine-effect pass.
pub const MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-MACHINE-EFFECT-ANALYSIS-INPUT/V1\0";
/// Canonical inert evidence schema emitted by the first gfx942 pass.
pub const MACHINE_EFFECT_EVIDENCE_DOMAIN_V1: &[u8] = b"FE2O3/GFX942-MACHINE-EFFECT-EVIDENCE/V1\0";
/// Numeric schema version for [`MachineEffectEvidenceV1`].
pub const MACHINE_EFFECT_EVIDENCE_VERSION_V1: u16 = 1;
const MACHINE_EFFECT_INPUT_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-MACHINE-EFFECT-ANALYSIS-INPUT-IDENTITY/V1\0";
const MACHINE_EFFECT_EVIDENCE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-MACHINE-EFFECT-EVIDENCE-IDENTITY/V1\0";

pub const MAX_MACHINE_EFFECT_ENTRY_POINTS_V1: usize = 32;
pub const MAX_MACHINE_EFFECT_FUNCTIONS_V1: usize = 256;
pub const MAX_MACHINE_EFFECT_CALL_EDGES_V1: usize = 1_024;
pub const MAX_MACHINE_EFFECT_OPERATIONS_V1: usize = 16_384;
pub const MAX_MACHINE_EFFECT_EFFECTS_V1: usize = 16_384;
pub const MAX_MACHINE_EFFECT_SYMBOL_BYTES_V1: usize = 256;
pub const MAX_MACHINE_EFFECT_RECURSION_DEPTH_V1: u16 = 1_024;
pub const MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1: usize = 2 * 1024 * 1024;
pub const MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1: usize = 2 * 1024 * 1024;

const MIN_ENTRY_POINT_RECORD_BYTES_V1: usize = 4 + 4 + 2 + 1;
const MIN_FUNCTION_RECORD_BYTES_V1: usize = 4 + 4 + 4;
const CALL_RECORD_BYTES_V1: usize = 2 + 4;
const MIN_OPERATION_RECORD_BYTES_V1: usize = 2 + 2;
const RECURSION_BOUND_RECORD_BYTES_V1: usize = 4 + 2;
const MIN_EFFECT_RECORD_BYTES_V1: usize = 4 + 4 + 4 + 2 + 4 + 2;

macro_rules! digest_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_sha256_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

digest_identity!(MachineTargetIdentityV1);
digest_identity!(MachineToolchainIdentityV1);
digest_identity!(MachineAnalyzerIdentityV1);
digest_identity!(MachineKernelIdentityV1);
digest_identity!(MachineDescriptorIdentityV1);

/// Exact payload identity bound by an analysis input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachinePayloadIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl MachinePayloadIdentityV1 {
    pub const fn from_parts(sha256: [u8; 32], byte_len: u64) -> Self {
        Self { sha256, byte_len }
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Target selected for the bounded mechanics model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MachineTargetV1 {
    Gfx942,
    Unsupported(u16),
}

/// Identities copied into both the input and resulting evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineEffectBindingsV1 {
    target: MachineTargetV1,
    target_identity: MachineTargetIdentityV1,
    toolchain_identity: MachineToolchainIdentityV1,
    analyzer_identity: MachineAnalyzerIdentityV1,
    kernel_identity: MachineKernelIdentityV1,
    payload_identity: MachinePayloadIdentityV1,
    descriptor_identity: MachineDescriptorIdentityV1,
}

impl MachineEffectBindingsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        target: MachineTargetV1,
        target_identity: MachineTargetIdentityV1,
        toolchain_identity: MachineToolchainIdentityV1,
        analyzer_identity: MachineAnalyzerIdentityV1,
        kernel_identity: MachineKernelIdentityV1,
        payload_identity: MachinePayloadIdentityV1,
        descriptor_identity: MachineDescriptorIdentityV1,
    ) -> Result<Self, MachineEffectInputErrorV1> {
        let digests = [
            ("target identity", target_identity.0),
            ("toolchain identity", toolchain_identity.0),
            ("analyzer identity", analyzer_identity.0),
            ("kernel identity", kernel_identity.0),
            ("payload identity", payload_identity.sha256),
            ("descriptor identity", descriptor_identity.0),
        ];
        for (field, digest) in digests {
            if digest == [0; 32] {
                return Err(MachineEffectInputErrorV1::ZeroDigestIdentity { field });
            }
        }
        if payload_identity.byte_len == 0 {
            return Err(MachineEffectInputErrorV1::ZeroPayloadLength);
        }
        Ok(Self {
            target,
            target_identity,
            toolchain_identity,
            analyzer_identity,
            kernel_identity,
            payload_identity,
            descriptor_identity,
        })
    }

    pub const fn target(self) -> MachineTargetV1 {
        self.target
    }

    pub const fn target_identity(self) -> MachineTargetIdentityV1 {
        self.target_identity
    }

    pub const fn toolchain_identity(self) -> MachineToolchainIdentityV1 {
        self.toolchain_identity
    }

    pub const fn analyzer_identity(self) -> MachineAnalyzerIdentityV1 {
        self.analyzer_identity
    }

    pub const fn kernel_identity(self) -> MachineKernelIdentityV1 {
        self.kernel_identity
    }

    pub const fn payload_identity(self) -> MachinePayloadIdentityV1 {
        self.payload_identity
    }

    pub const fn descriptor_identity(self) -> MachineDescriptorIdentityV1 {
        self.descriptor_identity
    }
}

/// Stable identifier local to one mechanics record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineFunctionIdV1(pub u32);

/// Stable finalized entry-point identifier local to one mechanics record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineEntryPointIdV1(pub u32);

/// One finalized entry point and its root function.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedEntryPointV1 {
    id: MachineEntryPointIdV1,
    symbol: String,
    function: MachineFunctionIdV1,
}

impl FinalizedEntryPointV1 {
    pub fn new(
        id: MachineEntryPointIdV1,
        symbol: impl Into<String>,
        function: MachineFunctionIdV1,
    ) -> Self {
        Self {
            id,
            symbol: symbol.into(),
            function,
        }
    }

    pub const fn id(&self) -> MachineEntryPointIdV1 {
        self.id
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn function(&self) -> MachineFunctionIdV1 {
        self.function
    }
}

/// Static call target reported by the finalized mechanics extractor.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MachineCallTargetV1 {
    Direct(MachineFunctionIdV1),
    Indirect,
}

/// Address space reported for an effect-bearing operation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MachineAddressSpaceV1 {
    Global,
    Constant,
    Workgroup,
    Private,
    Generic,
    Unsupported(u16),
}

/// Non-effect operation classes represented by the first mechanics model.
///
/// [`Self::ControlFlow`] remains wire-representable so V1 can reject it with a
/// precise diagnostic. Only arithmetic, comparisons, and a terminal return are
/// accepted by the straight-line analyzer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AcceptedMachineOpcodeV1 {
    IntegerAlu,
    IntegerComparison,
    ControlFlow,
    Return,
}

/// One operation in a finalized function's bounded mechanics record.
///
/// Operations are ordered within exactly one basic block. Consequently, an
/// address derivation can justify a later access only when it appears earlier
/// in this vector. This ordering is not a CFG dominance claim.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FinalizedMachineOperationV1 {
    AddressDerivation {
        address_space: MachineAddressSpaceV1,
        address_id: u32,
        base_argument: u16,
        index_scale: u32,
        constant_offset: i64,
    },
    Read {
        address_space: MachineAddressSpaceV1,
        address_id: u32,
        byte_width: u16,
    },
    Write {
        address_space: MachineAddressSpaceV1,
        address_id: u32,
        byte_width: u16,
    },
    NoEffect(AcceptedMachineOpcodeV1),
    UnsupportedOpcode(u16),
}

/// One finalized function and its static direct-call summary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedMachineFunctionV1 {
    id: MachineFunctionIdV1,
    calls: Vec<MachineCallTargetV1>,
    operations: Vec<FinalizedMachineOperationV1>,
}

impl FinalizedMachineFunctionV1 {
    pub fn new(
        id: MachineFunctionIdV1,
        calls: Vec<MachineCallTargetV1>,
        operations: Vec<FinalizedMachineOperationV1>,
    ) -> Self {
        Self {
            id,
            calls,
            operations,
        }
    }

    pub const fn id(&self) -> MachineFunctionIdV1 {
        self.id
    }

    pub fn calls(&self) -> &[MachineCallTargetV1] {
        &self.calls
    }

    pub fn operations(&self) -> &[FinalizedMachineOperationV1] {
        &self.operations
    }
}

/// Explicit finite bound required for a function participating in recursion.
///
/// V1 records this structural precondition but does not multiply static
/// effects by the bound or prove that dynamic call depth respects it.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineRecursionBoundV1 {
    function: MachineFunctionIdV1,
    maximum_depth: u16,
}

impl MachineRecursionBoundV1 {
    pub const fn new(function: MachineFunctionIdV1, maximum_depth: u16) -> Self {
        Self {
            function,
            maximum_depth,
        }
    }

    pub const fn function(self) -> MachineFunctionIdV1 {
        self.function
    }

    pub const fn maximum_depth(self) -> u16 {
        self.maximum_depth
    }
}

/// Canonical effect accepted for one entry-point closure.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineEffectV1 {
    entry_point: MachineEntryPointIdV1,
    function: MachineFunctionIdV1,
    operation_index: u32,
    kind: MachineEffectKindV1,
}

impl MachineEffectV1 {
    pub const fn new(
        entry_point: MachineEntryPointIdV1,
        function: MachineFunctionIdV1,
        operation_index: u32,
        kind: MachineEffectKindV1,
    ) -> Self {
        Self {
            entry_point,
            function,
            operation_index,
            kind,
        }
    }

    pub const fn entry_point(&self) -> MachineEntryPointIdV1 {
        self.entry_point
    }

    pub const fn function(&self) -> MachineFunctionIdV1 {
        self.function
    }

    pub const fn operation_index(&self) -> u32 {
        self.operation_index
    }

    pub const fn kind(&self) -> MachineEffectKindV1 {
        self.kind
    }
}

/// Effect kinds understood by the first gfx942 mechanics model.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MachineEffectKindV1 {
    GlobalAddressDerivation {
        address_id: u32,
        base_argument: u16,
        index_scale: u32,
        constant_offset: i64,
    },
    GlobalRead {
        address_id: u32,
        byte_width: u16,
    },
    GlobalWrite {
        address_id: u32,
        byte_width: u16,
    },
}

/// Domain-separated identity of one exact canonical analysis input.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineEffectAnalysisInputIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl MachineEffectAnalysisInputIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Validated, bounded input to the gfx942 mechanics analyzer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MachineEffectAnalysisInputV1 {
    bindings: MachineEffectBindingsV1,
    entries: Vec<FinalizedEntryPointV1>,
    functions: Vec<FinalizedMachineFunctionV1>,
    recursion_bounds: Vec<MachineRecursionBoundV1>,
    accepted_effects: Vec<MachineEffectV1>,
    canonical_bytes: Vec<u8>,
}

impl MachineEffectAnalysisInputV1 {
    pub fn new(
        bindings: MachineEffectBindingsV1,
        mut entries: Vec<FinalizedEntryPointV1>,
        mut functions: Vec<FinalizedMachineFunctionV1>,
        mut recursion_bounds: Vec<MachineRecursionBoundV1>,
        mut accepted_effects: Vec<MachineEffectV1>,
    ) -> Result<Self, MachineEffectInputErrorV1> {
        if bindings.target == MachineTargetV1::Unsupported(1) {
            return Err(MachineEffectInputErrorV1::NonCanonicalTargetTag);
        }
        validate_counts(&entries, &functions, &recursion_bounds, &accepted_effects)?;
        entries.sort();
        for function in &mut functions {
            function.calls.sort();
        }
        functions.sort_by_key(FinalizedMachineFunctionV1::id);
        recursion_bounds.sort();
        accepted_effects.sort();
        validate_input_structure(&entries, &functions, &recursion_bounds, &accepted_effects)?;

        let mut input = Self {
            bindings,
            entries,
            functions,
            recursion_bounds,
            accepted_effects,
            canonical_bytes: Vec::new(),
        };
        input.canonical_bytes = encode_input(&input)?;
        Ok(input)
    }

    pub const fn bindings(&self) -> MachineEffectBindingsV1 {
        self.bindings
    }

    pub fn entry_points(&self) -> &[FinalizedEntryPointV1] {
        &self.entries
    }

    pub fn functions(&self) -> &[FinalizedMachineFunctionV1] {
        &self.functions
    }

    pub fn recursion_bounds(&self) -> &[MachineRecursionBoundV1] {
        &self.recursion_bounds
    }

    pub fn accepted_effects(&self) -> &[MachineEffectV1] {
        &self.accepted_effects
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn identity(&self) -> MachineEffectAnalysisInputIdentityV1 {
        calculate_input_identity(&self.canonical_bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, MachineEffectDecodeErrorV1> {
        decode_input(bytes)
    }
}

/// Domain-separated identity of one exact canonical evidence record.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineEffectEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl MachineEffectEvidenceIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Inert output from structural analysis of one bounded mechanics record.
#[derive(Clone, Eq, PartialEq)]
pub struct MachineEffectEvidenceV1 {
    bindings: MachineEffectBindingsV1,
    input_identity: MachineEffectAnalysisInputIdentityV1,
    entries: Vec<FinalizedEntryPointV1>,
    effects: Vec<MachineEffectV1>,
    canonical_bytes: Vec<u8>,
}

/// Non-authoritative source of facts consumed by this analysis slice.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MachineEffectAnalysisBasisV1 {
    UnauthenticatedCallerSuppliedFinalizedMechanics,
}

impl fmt::Debug for MachineEffectEvidenceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MachineEffectEvidenceV1")
            .field("bindings", &self.bindings)
            .field("input_identity", &self.input_identity)
            .field("entries", &self.entries)
            .field("effects", &self.effects)
            .finish_non_exhaustive()
    }
}

impl MachineEffectEvidenceV1 {
    pub const fn schema_version(&self) -> u16 {
        MACHINE_EFFECT_EVIDENCE_VERSION_V1
    }

    pub const fn analysis_basis(&self) -> MachineEffectAnalysisBasisV1 {
        MachineEffectAnalysisBasisV1::UnauthenticatedCallerSuppliedFinalizedMechanics
    }

    pub const fn bindings(&self) -> MachineEffectBindingsV1 {
        self.bindings
    }

    pub const fn input_identity(&self) -> MachineEffectAnalysisInputIdentityV1 {
        self.input_identity
    }

    pub fn entry_points(&self) -> &[FinalizedEntryPointV1] {
        &self.entries
    }

    pub fn effects(&self) -> &[MachineEffectV1] {
        &self.effects
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn identity(&self) -> MachineEffectEvidenceIdentityV1 {
        calculate_evidence_identity(&self.canonical_bytes)
    }

    /// Strictly decodes evidence and re-derives it from the expected input.
    pub fn decode_canonical_for(
        input: &MachineEffectAnalysisInputV1,
        bytes: &[u8],
    ) -> Result<Self, MachineEffectEvidenceDecodeErrorV1> {
        decode_evidence_for(input, bytes)
    }

    pub const fn authenticates_extractor(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler(&self) -> bool {
        false
    }

    pub const fn establishes_payload_refinement(&self) -> bool {
        false
    }

    pub const fn contains_general_isa_disassembly(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Analyzes one closed, bounded gfx942 mechanics graph.
pub fn analyze_gfx942_machine_effects_v1(
    input: &MachineEffectAnalysisInputV1,
) -> Result<MachineEffectEvidenceV1, MachineEffectAnalysisErrorV1> {
    if input.bindings.target != MachineTargetV1::Gfx942 {
        return Err(MachineEffectAnalysisErrorV1::UnsupportedTarget(
            input.bindings.target,
        ));
    }

    let functions = input
        .functions
        .iter()
        .map(|function| (function.id, function))
        .collect::<BTreeMap<_, _>>();
    let graph = direct_call_graph(&input.functions)?;
    let reachable = reachable_functions(&input.entries, &graph);
    if let Some(function) = functions.keys().find(|id| !reachable.contains(id)) {
        return Err(MachineEffectAnalysisErrorV1::UnreachableFunction {
            function: *function,
        });
    }

    let recursive = recursive_functions(&graph);
    let bounds = input
        .recursion_bounds
        .iter()
        .map(|bound| (bound.function, bound.maximum_depth))
        .collect::<BTreeMap<_, _>>();
    if let Some(function) = recursive.iter().find(|id| !bounds.contains_key(id)) {
        return Err(MachineEffectAnalysisErrorV1::UnboundedRecursion {
            function: *function,
        });
    }
    if let Some(function) = bounds.keys().find(|id| !recursive.contains(id)) {
        return Err(MachineEffectAnalysisErrorV1::ExtraneousRecursionBound {
            function: *function,
        });
    }

    let effect_count = preflight_effect_count(&input.entries, &functions, &graph)?;
    let mut effects = Vec::new();
    effects.try_reserve_exact(effect_count).map_err(|_| {
        MachineEffectAnalysisErrorV1::EffectAllocationFailed {
            requested: effect_count,
        }
    })?;
    for entry in &input.entries {
        let closure = reachable_from(entry.function, &graph);
        for function_id in closure {
            analyze_function_operations(entry.id, functions[&function_id], &mut effects)?;
        }
    }
    effects.sort();
    let accepted = input.accepted_effects.iter().collect::<BTreeSet<_>>();
    if let Some(effect) = effects.iter().find(|effect| !accepted.contains(effect)) {
        return Err(MachineEffectAnalysisErrorV1::EffectExpansion {
            effect: (*effect).clone(),
        });
    }

    let mut evidence = MachineEffectEvidenceV1 {
        bindings: input.bindings,
        input_identity: input.identity(),
        entries: input.entries.clone(),
        effects,
        canonical_bytes: Vec::new(),
    };
    evidence.canonical_bytes = encode_evidence(&evidence).map_err(|error| match error {
        MachineEffectInputErrorV1::RecordTooLarge { actual, maximum } => {
            MachineEffectAnalysisErrorV1::EvidenceTooLarge { actual, maximum }
        }
        _ => unreachable!("validated evidence uses no fallible input fields"),
    })?;
    Ok(evidence)
}

fn analyze_function_operations(
    entry: MachineEntryPointIdV1,
    function: &FinalizedMachineFunctionV1,
    effects: &mut Vec<MachineEffectV1>,
) -> Result<(), MachineEffectAnalysisErrorV1> {
    let mut addresses = BTreeSet::new();
    for (index, operation) in function.operations.iter().enumerate() {
        let operation_index = u32::try_from(index).expect("operation count is bounded by u32");
        match operation {
            FinalizedMachineOperationV1::AddressDerivation {
                address_space,
                address_id,
                base_argument,
                index_scale,
                constant_offset,
            } => {
                require_global(function.id, operation_index, *address_space)?;
                if *index_scale == 0 {
                    return Err(MachineEffectAnalysisErrorV1::ZeroIndexScale {
                        function: function.id,
                        operation_index,
                    });
                }
                if !addresses.insert(*address_id) {
                    return Err(MachineEffectAnalysisErrorV1::DuplicateAddress {
                        function: function.id,
                        address_id: *address_id,
                    });
                }
                effects.push(MachineEffectV1::new(
                    entry,
                    function.id,
                    operation_index,
                    MachineEffectKindV1::GlobalAddressDerivation {
                        address_id: *address_id,
                        base_argument: *base_argument,
                        index_scale: *index_scale,
                        constant_offset: *constant_offset,
                    },
                ));
            }
            FinalizedMachineOperationV1::Read {
                address_space,
                address_id,
                byte_width,
            }
            | FinalizedMachineOperationV1::Write {
                address_space,
                address_id,
                byte_width,
            } => {
                require_global(function.id, operation_index, *address_space)?;
                if *byte_width == 0 {
                    return Err(MachineEffectAnalysisErrorV1::ZeroAccessWidth {
                        function: function.id,
                        operation_index,
                    });
                }
                if !addresses.contains(address_id) {
                    return Err(MachineEffectAnalysisErrorV1::UnknownAddress {
                        function: function.id,
                        operation_index,
                        address_id: *address_id,
                    });
                }
                let kind = match operation {
                    FinalizedMachineOperationV1::Read { .. } => MachineEffectKindV1::GlobalRead {
                        address_id: *address_id,
                        byte_width: *byte_width,
                    },
                    FinalizedMachineOperationV1::Write { .. } => MachineEffectKindV1::GlobalWrite {
                        address_id: *address_id,
                        byte_width: *byte_width,
                    },
                    _ => unreachable!(),
                };
                effects.push(MachineEffectV1::new(
                    entry,
                    function.id,
                    operation_index,
                    kind,
                ));
            }
            FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::ControlFlow) => {
                return Err(MachineEffectAnalysisErrorV1::UnsupportedControlFlow {
                    function: function.id,
                    operation_index,
                });
            }
            FinalizedMachineOperationV1::NoEffect(AcceptedMachineOpcodeV1::Return) => {
                if index + 1 != function.operations.len() {
                    return Err(MachineEffectAnalysisErrorV1::NonTerminalReturn {
                        function: function.id,
                        operation_index,
                    });
                }
            }
            FinalizedMachineOperationV1::NoEffect(
                AcceptedMachineOpcodeV1::IntegerAlu | AcceptedMachineOpcodeV1::IntegerComparison,
            ) => {}
            FinalizedMachineOperationV1::UnsupportedOpcode(opcode) => {
                return Err(MachineEffectAnalysisErrorV1::UnsupportedOpcode {
                    function: function.id,
                    operation_index,
                    opcode: *opcode,
                });
            }
        }
    }
    Ok(())
}

fn require_global(
    function: MachineFunctionIdV1,
    operation_index: u32,
    address_space: MachineAddressSpaceV1,
) -> Result<(), MachineEffectAnalysisErrorV1> {
    if address_space == MachineAddressSpaceV1::Global {
        Ok(())
    } else {
        Err(MachineEffectAnalysisErrorV1::UnsupportedAddressSpace {
            function,
            operation_index,
            address_space,
        })
    }
}

fn direct_call_graph(
    functions: &[FinalizedMachineFunctionV1],
) -> Result<
    BTreeMap<MachineFunctionIdV1, BTreeSet<MachineFunctionIdV1>>,
    MachineEffectAnalysisErrorV1,
> {
    let mut graph = BTreeMap::new();
    for function in functions {
        let mut callees = BTreeSet::new();
        for call in &function.calls {
            match call {
                MachineCallTargetV1::Direct(callee) => {
                    callees.insert(*callee);
                }
                MachineCallTargetV1::Indirect => {
                    return Err(MachineEffectAnalysisErrorV1::IndirectCall {
                        function: function.id,
                    });
                }
            }
        }
        graph.insert(function.id, callees);
    }
    Ok(graph)
}

fn reachable_functions(
    entries: &[FinalizedEntryPointV1],
    graph: &BTreeMap<MachineFunctionIdV1, BTreeSet<MachineFunctionIdV1>>,
) -> BTreeSet<MachineFunctionIdV1> {
    entries
        .iter()
        .flat_map(|entry| reachable_from(entry.function, graph))
        .collect()
}

fn reachable_from(
    root: MachineFunctionIdV1,
    graph: &BTreeMap<MachineFunctionIdV1, BTreeSet<MachineFunctionIdV1>>,
) -> BTreeSet<MachineFunctionIdV1> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(function) = pending.pop() {
        if reachable.insert(function) {
            pending.extend(graph[&function].iter().rev().copied());
        }
    }
    reachable
}

fn preflight_effect_count(
    entries: &[FinalizedEntryPointV1],
    functions: &BTreeMap<MachineFunctionIdV1, &FinalizedMachineFunctionV1>,
    graph: &BTreeMap<MachineFunctionIdV1, BTreeSet<MachineFunctionIdV1>>,
) -> Result<usize, MachineEffectAnalysisErrorV1> {
    let mut total = 0_usize;
    for entry in entries {
        for function in reachable_from(entry.function, graph) {
            let local = functions[&function]
                .operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        FinalizedMachineOperationV1::AddressDerivation { .. }
                            | FinalizedMachineOperationV1::Read { .. }
                            | FinalizedMachineOperationV1::Write { .. }
                    )
                })
                .count();
            total = total.checked_add(local).ok_or(
                MachineEffectAnalysisErrorV1::EffectCountBoundExceeded {
                    actual: usize::MAX,
                    maximum: MAX_MACHINE_EFFECT_EFFECTS_V1,
                },
            )?;
            if total > MAX_MACHINE_EFFECT_EFFECTS_V1 {
                return Err(MachineEffectAnalysisErrorV1::EffectCountBoundExceeded {
                    actual: total,
                    maximum: MAX_MACHINE_EFFECT_EFFECTS_V1,
                });
            }
        }
    }
    Ok(total)
}

fn recursive_functions(
    graph: &BTreeMap<MachineFunctionIdV1, BTreeSet<MachineFunctionIdV1>>,
) -> BTreeSet<MachineFunctionIdV1> {
    fn finish_order(
        function: MachineFunctionIdV1,
        graph: &BTreeMap<MachineFunctionIdV1, BTreeSet<MachineFunctionIdV1>>,
        visited: &mut BTreeSet<MachineFunctionIdV1>,
        order: &mut Vec<MachineFunctionIdV1>,
    ) {
        if !visited.insert(function) {
            return;
        }
        for callee in &graph[&function] {
            finish_order(*callee, graph, visited, order);
        }
        order.push(function);
    }

    fn collect_component(
        function: MachineFunctionIdV1,
        reverse: &BTreeMap<MachineFunctionIdV1, BTreeSet<MachineFunctionIdV1>>,
        visited: &mut BTreeSet<MachineFunctionIdV1>,
        component: &mut Vec<MachineFunctionIdV1>,
    ) {
        if !visited.insert(function) {
            return;
        }
        component.push(function);
        for caller in &reverse[&function] {
            collect_component(*caller, reverse, visited, component);
        }
    }

    let mut order = Vec::with_capacity(graph.len());
    let mut visited = BTreeSet::new();
    for function in graph.keys() {
        finish_order(*function, graph, &mut visited, &mut order);
    }

    let mut reverse = graph
        .keys()
        .copied()
        .map(|function| (function, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (caller, callees) in graph {
        for callee in callees {
            reverse
                .get_mut(callee)
                .expect("closed call graph contains every callee")
                .insert(*caller);
        }
    }

    visited.clear();
    let mut recursive = BTreeSet::new();
    while let Some(function) = order.pop() {
        if visited.contains(&function) {
            continue;
        }
        let mut component = Vec::new();
        collect_component(function, &reverse, &mut visited, &mut component);
        if component.len() > 1 || graph[&function].contains(&function) {
            recursive.extend(component);
        }
    }
    recursive
}

fn validate_counts(
    entries: &[FinalizedEntryPointV1],
    functions: &[FinalizedMachineFunctionV1],
    recursion_bounds: &[MachineRecursionBoundV1],
    effects: &[MachineEffectV1],
) -> Result<(), MachineEffectInputErrorV1> {
    bounded_count(
        "entry points",
        entries.len(),
        MAX_MACHINE_EFFECT_ENTRY_POINTS_V1,
    )?;
    bounded_count(
        "functions",
        functions.len(),
        MAX_MACHINE_EFFECT_FUNCTIONS_V1,
    )?;
    bounded_count(
        "recursion bounds",
        recursion_bounds.len(),
        MAX_MACHINE_EFFECT_FUNCTIONS_V1,
    )?;
    bounded_count(
        "accepted effects",
        effects.len(),
        MAX_MACHINE_EFFECT_EFFECTS_V1,
    )?;
    let calls = functions.iter().try_fold(0_usize, |total, function| {
        total.checked_add(function.calls.len()).ok_or(
            MachineEffectInputErrorV1::CountBoundExceeded {
                field: "call edges",
                actual: usize::MAX,
                maximum: MAX_MACHINE_EFFECT_CALL_EDGES_V1,
            },
        )
    })?;
    let operations = functions.iter().try_fold(0_usize, |total, function| {
        total.checked_add(function.operations.len()).ok_or(
            MachineEffectInputErrorV1::CountBoundExceeded {
                field: "operations",
                actual: usize::MAX,
                maximum: MAX_MACHINE_EFFECT_OPERATIONS_V1,
            },
        )
    })?;
    bounded_count("call edges", calls, MAX_MACHINE_EFFECT_CALL_EDGES_V1)?;
    bounded_count("operations", operations, MAX_MACHINE_EFFECT_OPERATIONS_V1)
}

fn bounded_count(
    field: &'static str,
    actual: usize,
    maximum: usize,
) -> Result<(), MachineEffectInputErrorV1> {
    if actual > maximum {
        Err(MachineEffectInputErrorV1::CountBoundExceeded {
            field,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

fn validate_input_structure(
    entries: &[FinalizedEntryPointV1],
    functions: &[FinalizedMachineFunctionV1],
    recursion_bounds: &[MachineRecursionBoundV1],
    effects: &[MachineEffectV1],
) -> Result<(), MachineEffectInputErrorV1> {
    if entries.is_empty() {
        return Err(MachineEffectInputErrorV1::MissingEntryPoints);
    }
    let mut entry_ids = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for entry in entries {
        validate_symbol(&entry.symbol)?;
        if !entry_ids.insert(entry.id) {
            return Err(MachineEffectInputErrorV1::DuplicateEntryPoint(entry.id));
        }
        if !symbols.insert(entry.symbol.clone()) {
            return Err(MachineEffectInputErrorV1::DuplicateEntrySymbol(
                entry.symbol.clone(),
            ));
        }
    }

    let mut function_ids = BTreeSet::new();
    for function in functions {
        if !function_ids.insert(function.id) {
            return Err(MachineEffectInputErrorV1::DuplicateFunction(function.id));
        }
        for operation in &function.operations {
            let address_space = match operation {
                FinalizedMachineOperationV1::AddressDerivation { address_space, .. }
                | FinalizedMachineOperationV1::Read { address_space, .. }
                | FinalizedMachineOperationV1::Write { address_space, .. } => Some(*address_space),
                FinalizedMachineOperationV1::NoEffect(_)
                | FinalizedMachineOperationV1::UnsupportedOpcode(_) => None,
            };
            if let Some(MachineAddressSpaceV1::Unsupported(1..=5)) = address_space {
                return Err(MachineEffectInputErrorV1::NonCanonicalAddressSpaceTag);
            }
        }
        let mut direct = BTreeSet::new();
        let mut indirect = false;
        for call in &function.calls {
            match call {
                MachineCallTargetV1::Direct(callee) if !direct.insert(*callee) => {
                    return Err(MachineEffectInputErrorV1::DuplicateCallEdge {
                        caller: function.id,
                        callee: *callee,
                    });
                }
                MachineCallTargetV1::Indirect if indirect => {
                    return Err(MachineEffectInputErrorV1::DuplicateIndirectCall {
                        function: function.id,
                    });
                }
                MachineCallTargetV1::Indirect => indirect = true,
                MachineCallTargetV1::Direct(_) => {}
            }
        }
    }
    for entry in entries {
        if !function_ids.contains(&entry.function) {
            return Err(MachineEffectInputErrorV1::MissingEntryFunction {
                entry: entry.id,
                function: entry.function,
            });
        }
    }
    for function in functions {
        for call in &function.calls {
            if let MachineCallTargetV1::Direct(callee) = call
                && !function_ids.contains(callee)
            {
                return Err(MachineEffectInputErrorV1::UnknownDirectCallee {
                    caller: function.id,
                    callee: *callee,
                });
            }
        }
    }

    let mut bound_functions = BTreeSet::new();
    for bound in recursion_bounds {
        if !function_ids.contains(&bound.function) {
            return Err(MachineEffectInputErrorV1::UnknownRecursionFunction(
                bound.function,
            ));
        }
        if !bound_functions.insert(bound.function) {
            return Err(MachineEffectInputErrorV1::DuplicateRecursionBound(
                bound.function,
            ));
        }
        if bound.maximum_depth == 0 || bound.maximum_depth > MAX_MACHINE_EFFECT_RECURSION_DEPTH_V1 {
            return Err(MachineEffectInputErrorV1::InvalidRecursionDepth {
                function: bound.function,
                depth: bound.maximum_depth,
            });
        }
    }

    let entries = entry_ids;
    let mut unique_effects = BTreeSet::new();
    for effect in effects {
        if !entries.contains(&effect.entry_point) {
            return Err(MachineEffectInputErrorV1::UnknownEffectEntry(
                effect.entry_point,
            ));
        }
        if !function_ids.contains(&effect.function) {
            return Err(MachineEffectInputErrorV1::UnknownEffectFunction(
                effect.function,
            ));
        }
        if !unique_effects.insert(effect.clone()) {
            return Err(MachineEffectInputErrorV1::DuplicateEffect(effect.clone()));
        }
    }
    Ok(())
}

fn validate_symbol(symbol: &str) -> Result<(), MachineEffectInputErrorV1> {
    if symbol.is_empty()
        || symbol.len() > MAX_MACHINE_EFFECT_SYMBOL_BYTES_V1
        || !symbol
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$'))
    {
        Err(MachineEffectInputErrorV1::InvalidEntrySymbol(
            symbol.to_owned(),
        ))
    } else {
        Ok(())
    }
}

/// Failure to construct a bounded mechanics input.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MachineEffectInputErrorV1 {
    MissingEntryPoints,
    ZeroDigestIdentity {
        field: &'static str,
    },
    ZeroPayloadLength,
    CountBoundExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    RecordTooLarge {
        actual: usize,
        maximum: usize,
    },
    InvalidEntrySymbol(String),
    DuplicateEntryPoint(MachineEntryPointIdV1),
    DuplicateEntrySymbol(String),
    DuplicateFunction(MachineFunctionIdV1),
    MissingEntryFunction {
        entry: MachineEntryPointIdV1,
        function: MachineFunctionIdV1,
    },
    DuplicateCallEdge {
        caller: MachineFunctionIdV1,
        callee: MachineFunctionIdV1,
    },
    DuplicateIndirectCall {
        function: MachineFunctionIdV1,
    },
    UnknownDirectCallee {
        caller: MachineFunctionIdV1,
        callee: MachineFunctionIdV1,
    },
    UnknownRecursionFunction(MachineFunctionIdV1),
    DuplicateRecursionBound(MachineFunctionIdV1),
    InvalidRecursionDepth {
        function: MachineFunctionIdV1,
        depth: u16,
    },
    UnknownEffectEntry(MachineEntryPointIdV1),
    UnknownEffectFunction(MachineFunctionIdV1),
    DuplicateEffect(MachineEffectV1),
    NonCanonicalTargetTag,
    NonCanonicalAddressSpaceTag,
}

impl fmt::Display for MachineEffectInputErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid gfx942 machine-effect input: {self:?}")
    }
}

impl Error for MachineEffectInputErrorV1 {}

/// Fail-closed analysis rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MachineEffectAnalysisErrorV1 {
    UnsupportedTarget(MachineTargetV1),
    IndirectCall {
        function: MachineFunctionIdV1,
    },
    UnreachableFunction {
        function: MachineFunctionIdV1,
    },
    UnboundedRecursion {
        function: MachineFunctionIdV1,
    },
    ExtraneousRecursionBound {
        function: MachineFunctionIdV1,
    },
    UnsupportedOpcode {
        function: MachineFunctionIdV1,
        operation_index: u32,
        opcode: u16,
    },
    UnsupportedControlFlow {
        function: MachineFunctionIdV1,
        operation_index: u32,
    },
    NonTerminalReturn {
        function: MachineFunctionIdV1,
        operation_index: u32,
    },
    UnsupportedAddressSpace {
        function: MachineFunctionIdV1,
        operation_index: u32,
        address_space: MachineAddressSpaceV1,
    },
    DuplicateAddress {
        function: MachineFunctionIdV1,
        address_id: u32,
    },
    UnknownAddress {
        function: MachineFunctionIdV1,
        operation_index: u32,
        address_id: u32,
    },
    ZeroIndexScale {
        function: MachineFunctionIdV1,
        operation_index: u32,
    },
    ZeroAccessWidth {
        function: MachineFunctionIdV1,
        operation_index: u32,
    },
    EffectExpansion {
        effect: MachineEffectV1,
    },
    EffectCountBoundExceeded {
        actual: usize,
        maximum: usize,
    },
    EffectAllocationFailed {
        requested: usize,
    },
    EvidenceTooLarge {
        actual: usize,
        maximum: usize,
    },
}

impl fmt::Display for MachineEffectAnalysisErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "gfx942 machine-effect analysis rejected input: {self:?}"
        )
    }
}

impl Error for MachineEffectAnalysisErrorV1 {}

/// Failure to decode a complete canonical input record.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MachineEffectDecodeErrorV1 {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Truncated,
    InvalidDomain,
    DeclaredLengthMismatch {
        declared: usize,
        actual: usize,
    },
    UnsupportedFlags(u32),
    InvalidTag {
        field: &'static str,
        actual: u16,
    },
    InvalidUtf8,
    AllocationFailed {
        field: &'static str,
        requested: usize,
    },
    InvalidInput(Box<MachineEffectInputErrorV1>),
    NonCanonical,
}

impl fmt::Display for MachineEffectDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid canonical gfx942 machine-effect input: {self:?}"
        )
    }
}

impl Error for MachineEffectDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidInput(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Failure to decode evidence for one exact expected input.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MachineEffectEvidenceDecodeErrorV1 {
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Truncated,
    InvalidDomain,
    DeclaredLengthMismatch {
        declared: usize,
        actual: usize,
    },
    UnsupportedFlags(u32),
    InvalidTag {
        field: &'static str,
        actual: u16,
    },
    InvalidUtf8,
    AllocationFailed {
        field: &'static str,
        requested: usize,
    },
    CountBoundExceeded {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    InvalidEntrySymbol(String),
    DuplicateEntryPoint(MachineEntryPointIdV1),
    DuplicateEntrySymbol(String),
    DuplicateEffect(MachineEffectV1),
    InvalidBindings(Box<MachineEffectInputErrorV1>),
    IdentityBindingMismatch,
    InputIdentityMismatch,
    EntryPointsMismatch,
    EffectEvidenceMismatch,
    Analysis(Box<MachineEffectAnalysisErrorV1>),
    NonCanonical,
}

impl fmt::Display for MachineEffectEvidenceDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid canonical gfx942 machine-effect evidence: {self:?}"
        )
    }
}

impl Error for MachineEffectEvidenceDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Analysis(error) => Some(error.as_ref()),
            Self::InvalidBindings(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

fn calculate_input_identity(bytes: &[u8]) -> MachineEffectAnalysisInputIdentityV1 {
    let mut hash = Sha256::new();
    hash.update(MACHINE_EFFECT_INPUT_IDENTITY_DOMAIN_V1);
    hash.update(bytes);
    MachineEffectAnalysisInputIdentityV1 {
        sha256: hash.finalize().into(),
        byte_len: bytes.len() as u64,
    }
}

fn calculate_evidence_identity(bytes: &[u8]) -> MachineEffectEvidenceIdentityV1 {
    let mut hash = Sha256::new();
    hash.update(MACHINE_EFFECT_EVIDENCE_IDENTITY_DOMAIN_V1);
    hash.update(bytes);
    MachineEffectEvidenceIdentityV1 {
        sha256: hash.finalize().into(),
        byte_len: bytes.len() as u64,
    }
}

fn encode_input(
    input: &MachineEffectAnalysisInputV1,
) -> Result<Vec<u8>, MachineEffectInputErrorV1> {
    let mut output = Vec::new();
    output.extend_from_slice(MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u32(&mut output, 0);
    encode_bindings(&mut output, input.bindings);
    push_u32(&mut output, input.entries.len());
    push_u32(&mut output, input.functions.len());
    push_u32(&mut output, input.recursion_bounds.len());
    push_u32(&mut output, input.accepted_effects.len());
    for entry in &input.entries {
        encode_entry(&mut output, entry);
    }
    for function in &input.functions {
        encode_function(&mut output, function);
    }
    for bound in &input.recursion_bounds {
        push_u32(&mut output, bound.function.0 as usize);
        output.extend_from_slice(&bound.maximum_depth.to_le_bytes());
    }
    for effect in &input.accepted_effects {
        encode_effect(&mut output, effect);
    }
    finish_record(
        output,
        MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1.len(),
        MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1,
    )
}

fn encode_evidence(
    evidence: &MachineEffectEvidenceV1,
) -> Result<Vec<u8>, MachineEffectInputErrorV1> {
    let mut output = Vec::new();
    output.extend_from_slice(MACHINE_EFFECT_EVIDENCE_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u32(&mut output, 0);
    encode_bindings(&mut output, evidence.bindings);
    output.extend_from_slice(&evidence.input_identity.sha256);
    output.extend_from_slice(&evidence.input_identity.byte_len.to_le_bytes());
    push_u32(&mut output, evidence.entries.len());
    push_u32(&mut output, evidence.effects.len());
    for entry in &evidence.entries {
        encode_entry(&mut output, entry);
    }
    for effect in &evidence.effects {
        encode_effect(&mut output, effect);
    }
    finish_record(
        output,
        MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len(),
        MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1,
    )
}

fn finish_record(
    mut output: Vec<u8>,
    domain_len: usize,
    maximum: usize,
) -> Result<Vec<u8>, MachineEffectInputErrorV1> {
    if output.len() > maximum || output.len() > u32::MAX as usize {
        return Err(MachineEffectInputErrorV1::RecordTooLarge {
            actual: output.len(),
            maximum,
        });
    }
    let length = u32::try_from(output.len()).expect("record length checked against u32");
    output[domain_len..domain_len + 4].copy_from_slice(&length.to_le_bytes());
    Ok(output)
}

fn encode_bindings(output: &mut Vec<u8>, bindings: MachineEffectBindingsV1) {
    let target = match bindings.target {
        MachineTargetV1::Gfx942 => 1,
        MachineTargetV1::Unsupported(value) => value,
    };
    output.extend_from_slice(&target.to_le_bytes());
    output.extend_from_slice(&bindings.target_identity.0);
    output.extend_from_slice(&bindings.toolchain_identity.0);
    output.extend_from_slice(&bindings.analyzer_identity.0);
    output.extend_from_slice(&bindings.kernel_identity.0);
    output.extend_from_slice(&bindings.payload_identity.sha256);
    output.extend_from_slice(&bindings.payload_identity.byte_len.to_le_bytes());
    output.extend_from_slice(&bindings.descriptor_identity.0);
}

fn encode_entry(output: &mut Vec<u8>, entry: &FinalizedEntryPointV1) {
    push_u32(output, entry.id.0 as usize);
    push_u32(output, entry.function.0 as usize);
    output.extend_from_slice(&(entry.symbol.len() as u16).to_le_bytes());
    output.extend_from_slice(entry.symbol.as_bytes());
}

fn encode_function(output: &mut Vec<u8>, function: &FinalizedMachineFunctionV1) {
    push_u32(output, function.id.0 as usize);
    push_u32(output, function.calls.len());
    push_u32(output, function.operations.len());
    for call in &function.calls {
        match call {
            MachineCallTargetV1::Direct(callee) => {
                output.extend_from_slice(&1_u16.to_le_bytes());
                push_u32(output, callee.0 as usize);
            }
            MachineCallTargetV1::Indirect => {
                output.extend_from_slice(&2_u16.to_le_bytes());
                push_u32(output, 0);
            }
        }
    }
    for operation in &function.operations {
        encode_operation(output, operation);
    }
}

fn encode_operation(output: &mut Vec<u8>, operation: &FinalizedMachineOperationV1) {
    match operation {
        FinalizedMachineOperationV1::AddressDerivation {
            address_space,
            address_id,
            base_argument,
            index_scale,
            constant_offset,
        } => {
            output.extend_from_slice(&1_u16.to_le_bytes());
            encode_address_space(output, *address_space);
            push_u32(output, *address_id as usize);
            output.extend_from_slice(&base_argument.to_le_bytes());
            output.extend_from_slice(&index_scale.to_le_bytes());
            output.extend_from_slice(&constant_offset.to_le_bytes());
        }
        FinalizedMachineOperationV1::Read {
            address_space,
            address_id,
            byte_width,
        } => {
            output.extend_from_slice(&2_u16.to_le_bytes());
            encode_address_space(output, *address_space);
            push_u32(output, *address_id as usize);
            output.extend_from_slice(&byte_width.to_le_bytes());
        }
        FinalizedMachineOperationV1::Write {
            address_space,
            address_id,
            byte_width,
        } => {
            output.extend_from_slice(&3_u16.to_le_bytes());
            encode_address_space(output, *address_space);
            push_u32(output, *address_id as usize);
            output.extend_from_slice(&byte_width.to_le_bytes());
        }
        FinalizedMachineOperationV1::NoEffect(opcode) => {
            output.extend_from_slice(&4_u16.to_le_bytes());
            let opcode = match opcode {
                AcceptedMachineOpcodeV1::IntegerAlu => 1_u16,
                AcceptedMachineOpcodeV1::IntegerComparison => 2,
                AcceptedMachineOpcodeV1::ControlFlow => 3,
                AcceptedMachineOpcodeV1::Return => 4,
            };
            output.extend_from_slice(&opcode.to_le_bytes());
        }
        FinalizedMachineOperationV1::UnsupportedOpcode(opcode) => {
            output.extend_from_slice(&5_u16.to_le_bytes());
            output.extend_from_slice(&opcode.to_le_bytes());
        }
    }
}

fn encode_address_space(output: &mut Vec<u8>, address_space: MachineAddressSpaceV1) {
    let value = match address_space {
        MachineAddressSpaceV1::Global => 1,
        MachineAddressSpaceV1::Constant => 2,
        MachineAddressSpaceV1::Workgroup => 3,
        MachineAddressSpaceV1::Private => 4,
        MachineAddressSpaceV1::Generic => 5,
        MachineAddressSpaceV1::Unsupported(value) => value,
    };
    output.extend_from_slice(&value.to_le_bytes());
}

fn encode_effect(output: &mut Vec<u8>, effect: &MachineEffectV1) {
    push_u32(output, effect.entry_point.0 as usize);
    push_u32(output, effect.function.0 as usize);
    push_u32(output, effect.operation_index as usize);
    match effect.kind {
        MachineEffectKindV1::GlobalAddressDerivation {
            address_id,
            base_argument,
            index_scale,
            constant_offset,
        } => {
            output.extend_from_slice(&1_u16.to_le_bytes());
            push_u32(output, address_id as usize);
            output.extend_from_slice(&base_argument.to_le_bytes());
            output.extend_from_slice(&index_scale.to_le_bytes());
            output.extend_from_slice(&constant_offset.to_le_bytes());
        }
        MachineEffectKindV1::GlobalRead {
            address_id,
            byte_width,
        } => {
            output.extend_from_slice(&2_u16.to_le_bytes());
            push_u32(output, address_id as usize);
            output.extend_from_slice(&byte_width.to_le_bytes());
        }
        MachineEffectKindV1::GlobalWrite {
            address_id,
            byte_width,
        } => {
            output.extend_from_slice(&3_u16.to_le_bytes());
            push_u32(output, address_id as usize);
            output.extend_from_slice(&byte_width.to_le_bytes());
        }
    }
}

fn decode_input(bytes: &[u8]) -> Result<MachineEffectAnalysisInputV1, MachineEffectDecodeErrorV1> {
    if bytes.len() > MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1 {
        return Err(MachineEffectDecodeErrorV1::TooLarge {
            actual: bytes.len(),
            maximum: MAX_MACHINE_EFFECT_ANALYSIS_INPUT_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<{ MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1.len() }>()?
        != MACHINE_EFFECT_ANALYSIS_INPUT_DOMAIN_V1
    {
        return Err(MachineEffectDecodeErrorV1::InvalidDomain);
    }
    validate_input_header(&mut reader, bytes.len())?;
    let bindings = decode_bindings(&mut reader)?;
    let entry_count = reader.bounded_count("entry points", MAX_MACHINE_EFFECT_ENTRY_POINTS_V1)?;
    let function_count = reader.bounded_count("functions", MAX_MACHINE_EFFECT_FUNCTIONS_V1)?;
    let recursion_count =
        reader.bounded_count("recursion bounds", MAX_MACHINE_EFFECT_FUNCTIONS_V1)?;
    let effect_count = reader.bounded_count("accepted effects", MAX_MACHINE_EFFECT_EFFECTS_V1)?;
    reader.require_remaining(minimum_remaining_bytes(&[
        (entry_count, MIN_ENTRY_POINT_RECORD_BYTES_V1),
        (function_count, MIN_FUNCTION_RECORD_BYTES_V1),
        (recursion_count, RECURSION_BOUND_RECORD_BYTES_V1),
        (effect_count, MIN_EFFECT_RECORD_BYTES_V1),
    ])?)?;
    let entries = decode_entries(&mut reader, entry_count)?;
    let mut functions = Vec::new();
    try_reserve_exact(&mut functions, function_count, "functions")?;
    let mut total_calls = 0_usize;
    let mut total_operations = 0_usize;
    for _ in 0..function_count {
        let function = decode_function(&mut reader, &mut total_calls, &mut total_operations)?;
        functions.push(function);
    }
    let mut recursion_bounds = Vec::new();
    try_reserve_exact(&mut recursion_bounds, recursion_count, "recursion bounds")?;
    for _ in 0..recursion_count {
        recursion_bounds.push(MachineRecursionBoundV1::new(
            MachineFunctionIdV1(reader.u32()?),
            reader.u16()?,
        ));
    }
    let effects = decode_effects(&mut reader, effect_count)?;
    if !reader.is_finished() {
        return Err(MachineEffectDecodeErrorV1::DeclaredLengthMismatch {
            declared: reader.position(),
            actual: bytes.len(),
        });
    }
    let input =
        MachineEffectAnalysisInputV1::new(bindings, entries, functions, recursion_bounds, effects)
            .map_err(invalid_input)?;
    if input.canonical_bytes != bytes {
        return Err(MachineEffectDecodeErrorV1::NonCanonical);
    }
    Ok(input)
}

fn validate_input_header(
    reader: &mut Reader<'_>,
    actual: usize,
) -> Result<(), MachineEffectDecodeErrorV1> {
    let declared = reader.u32()? as usize;
    if declared != actual {
        return Err(MachineEffectDecodeErrorV1::DeclaredLengthMismatch { declared, actual });
    }
    let flags = reader.u32()?;
    if flags != 0 {
        return Err(MachineEffectDecodeErrorV1::UnsupportedFlags(flags));
    }
    Ok(())
}

fn decode_bindings(
    reader: &mut Reader<'_>,
) -> Result<MachineEffectBindingsV1, MachineEffectDecodeErrorV1> {
    let target = match reader.u16()? {
        1 => MachineTargetV1::Gfx942,
        value => MachineTargetV1::Unsupported(value),
    };
    MachineEffectBindingsV1::new(
        target,
        MachineTargetIdentityV1(reader.fixed()?),
        MachineToolchainIdentityV1(reader.fixed()?),
        MachineAnalyzerIdentityV1(reader.fixed()?),
        MachineKernelIdentityV1(reader.fixed()?),
        MachinePayloadIdentityV1::from_parts(reader.fixed()?, reader.u64()?),
        MachineDescriptorIdentityV1(reader.fixed()?),
    )
    .map_err(invalid_input)
}

fn decode_entries(
    reader: &mut Reader<'_>,
    count: usize,
) -> Result<Vec<FinalizedEntryPointV1>, MachineEffectDecodeErrorV1> {
    reader.require_remaining(minimum_remaining_bytes(&[(
        count,
        MIN_ENTRY_POINT_RECORD_BYTES_V1,
    )])?)?;
    let mut entries = Vec::new();
    try_reserve_exact(&mut entries, count, "entry points")?;
    for _ in 0..count {
        let id = MachineEntryPointIdV1(reader.u32()?);
        let function = MachineFunctionIdV1(reader.u32()?);
        let length = reader.u16()? as usize;
        if length > MAX_MACHINE_EFFECT_SYMBOL_BYTES_V1 {
            return Err(invalid_input(
                MachineEffectInputErrorV1::InvalidEntrySymbol("<oversized>".to_owned()),
            ));
        }
        let symbol = std::str::from_utf8(reader.take(length)?)
            .map_err(|_| MachineEffectDecodeErrorV1::InvalidUtf8)?
            .to_owned();
        entries.push(FinalizedEntryPointV1::new(id, symbol, function));
    }
    Ok(entries)
}

fn decode_function(
    reader: &mut Reader<'_>,
    total_calls: &mut usize,
    total_operations: &mut usize,
) -> Result<FinalizedMachineFunctionV1, MachineEffectDecodeErrorV1> {
    let id = MachineFunctionIdV1(reader.u32()?);
    let call_count = reader.bounded_count("function calls", MAX_MACHINE_EFFECT_CALL_EDGES_V1)?;
    let operation_count =
        reader.bounded_count("function operations", MAX_MACHINE_EFFECT_OPERATIONS_V1)?;
    *total_calls = checked_aggregate_count(
        "call edges",
        *total_calls,
        call_count,
        MAX_MACHINE_EFFECT_CALL_EDGES_V1,
    )?;
    *total_operations = checked_aggregate_count(
        "operations",
        *total_operations,
        operation_count,
        MAX_MACHINE_EFFECT_OPERATIONS_V1,
    )?;
    reader.require_remaining(minimum_remaining_bytes(&[
        (call_count, CALL_RECORD_BYTES_V1),
        (operation_count, MIN_OPERATION_RECORD_BYTES_V1),
    ])?)?;
    let mut calls = Vec::new();
    try_reserve_exact(&mut calls, call_count, "function calls")?;
    for _ in 0..call_count {
        let tag = reader.u16()?;
        let target = MachineFunctionIdV1(reader.u32()?);
        calls.push(match tag {
            1 => MachineCallTargetV1::Direct(target),
            2 if target.0 == 0 => MachineCallTargetV1::Indirect,
            _ => {
                return Err(MachineEffectDecodeErrorV1::InvalidTag {
                    field: "call target",
                    actual: tag,
                });
            }
        });
    }
    let mut operations = Vec::new();
    try_reserve_exact(&mut operations, operation_count, "function operations")?;
    for _ in 0..operation_count {
        operations.push(decode_operation(reader)?);
    }
    Ok(FinalizedMachineFunctionV1::new(id, calls, operations))
}

fn decode_operation(
    reader: &mut Reader<'_>,
) -> Result<FinalizedMachineOperationV1, MachineEffectDecodeErrorV1> {
    let tag = reader.u16()?;
    match tag {
        1 => Ok(FinalizedMachineOperationV1::AddressDerivation {
            address_space: decode_address_space(reader.u16()?),
            address_id: reader.u32()?,
            base_argument: reader.u16()?,
            index_scale: reader.u32()?,
            constant_offset: reader.i64()?,
        }),
        2 => Ok(FinalizedMachineOperationV1::Read {
            address_space: decode_address_space(reader.u16()?),
            address_id: reader.u32()?,
            byte_width: reader.u16()?,
        }),
        3 => Ok(FinalizedMachineOperationV1::Write {
            address_space: decode_address_space(reader.u16()?),
            address_id: reader.u32()?,
            byte_width: reader.u16()?,
        }),
        4 => {
            let opcode = reader.u16()?;
            let opcode = match opcode {
                1 => AcceptedMachineOpcodeV1::IntegerAlu,
                2 => AcceptedMachineOpcodeV1::IntegerComparison,
                3 => AcceptedMachineOpcodeV1::ControlFlow,
                4 => AcceptedMachineOpcodeV1::Return,
                actual => {
                    return Err(MachineEffectDecodeErrorV1::InvalidTag {
                        field: "accepted opcode",
                        actual,
                    });
                }
            };
            Ok(FinalizedMachineOperationV1::NoEffect(opcode))
        }
        5 => Ok(FinalizedMachineOperationV1::UnsupportedOpcode(
            reader.u16()?,
        )),
        actual => Err(MachineEffectDecodeErrorV1::InvalidTag {
            field: "operation",
            actual,
        }),
    }
}

fn decode_address_space(value: u16) -> MachineAddressSpaceV1 {
    match value {
        1 => MachineAddressSpaceV1::Global,
        2 => MachineAddressSpaceV1::Constant,
        3 => MachineAddressSpaceV1::Workgroup,
        4 => MachineAddressSpaceV1::Private,
        5 => MachineAddressSpaceV1::Generic,
        other => MachineAddressSpaceV1::Unsupported(other),
    }
}

fn decode_effects(
    reader: &mut Reader<'_>,
    count: usize,
) -> Result<Vec<MachineEffectV1>, MachineEffectDecodeErrorV1> {
    reader.require_remaining(minimum_remaining_bytes(&[(
        count,
        MIN_EFFECT_RECORD_BYTES_V1,
    )])?)?;
    let mut effects = Vec::new();
    try_reserve_exact(&mut effects, count, "effects")?;
    for _ in 0..count {
        effects.push(decode_effect(reader)?);
    }
    Ok(effects)
}

fn decode_effect(reader: &mut Reader<'_>) -> Result<MachineEffectV1, MachineEffectDecodeErrorV1> {
    let entry = MachineEntryPointIdV1(reader.u32()?);
    let function = MachineFunctionIdV1(reader.u32()?);
    let operation_index = reader.u32()?;
    let tag = reader.u16()?;
    let kind = match tag {
        1 => MachineEffectKindV1::GlobalAddressDerivation {
            address_id: reader.u32()?,
            base_argument: reader.u16()?,
            index_scale: reader.u32()?,
            constant_offset: reader.i64()?,
        },
        2 => MachineEffectKindV1::GlobalRead {
            address_id: reader.u32()?,
            byte_width: reader.u16()?,
        },
        3 => MachineEffectKindV1::GlobalWrite {
            address_id: reader.u32()?,
            byte_width: reader.u16()?,
        },
        actual => {
            return Err(MachineEffectDecodeErrorV1::InvalidTag {
                field: "effect",
                actual,
            });
        }
    };
    Ok(MachineEffectV1::new(entry, function, operation_index, kind))
}

fn minimum_remaining_bytes(
    records: &[(usize, usize)],
) -> Result<usize, MachineEffectDecodeErrorV1> {
    records.iter().try_fold(0_usize, |total, (count, width)| {
        count
            .checked_mul(*width)
            .and_then(|bytes| total.checked_add(bytes))
            .ok_or(MachineEffectDecodeErrorV1::Truncated)
    })
}

fn checked_aggregate_count(
    field: &'static str,
    prior: usize,
    additional: usize,
    maximum: usize,
) -> Result<usize, MachineEffectDecodeErrorV1> {
    let actual = prior.saturating_add(additional);
    if actual > maximum {
        Err(invalid_input(
            MachineEffectInputErrorV1::CountBoundExceeded {
                field,
                actual,
                maximum,
            },
        ))
    } else {
        Ok(actual)
    }
}

fn try_reserve_exact<T>(
    values: &mut Vec<T>,
    requested: usize,
    field: &'static str,
) -> Result<(), MachineEffectDecodeErrorV1> {
    values
        .try_reserve_exact(requested)
        .map_err(|_| MachineEffectDecodeErrorV1::AllocationFailed { field, requested })
}

fn invalid_input(error: MachineEffectInputErrorV1) -> MachineEffectDecodeErrorV1 {
    MachineEffectDecodeErrorV1::InvalidInput(Box::new(error))
}

fn decode_evidence_for(
    input: &MachineEffectAnalysisInputV1,
    bytes: &[u8],
) -> Result<MachineEffectEvidenceV1, MachineEffectEvidenceDecodeErrorV1> {
    if bytes.len() > MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1 {
        return Err(MachineEffectEvidenceDecodeErrorV1::TooLarge {
            actual: bytes.len(),
            maximum: MAX_MACHINE_EFFECT_EVIDENCE_BYTES_V1,
        });
    }
    let mut reader = Reader::new(bytes);
    if reader
        .fixed::<{ MACHINE_EFFECT_EVIDENCE_DOMAIN_V1.len() }>()
        .map_err(map_evidence_decode)?
        != MACHINE_EFFECT_EVIDENCE_DOMAIN_V1
    {
        return Err(MachineEffectEvidenceDecodeErrorV1::InvalidDomain);
    }
    let declared = reader.u32().map_err(map_evidence_decode)? as usize;
    if declared != bytes.len() {
        return Err(MachineEffectEvidenceDecodeErrorV1::DeclaredLengthMismatch {
            declared,
            actual: bytes.len(),
        });
    }
    let flags = reader.u32().map_err(map_evidence_decode)?;
    if flags != 0 {
        return Err(MachineEffectEvidenceDecodeErrorV1::UnsupportedFlags(flags));
    }
    let bindings = decode_bindings(&mut reader).map_err(map_evidence_decode)?;
    let input_identity = MachineEffectAnalysisInputIdentityV1 {
        sha256: reader.fixed().map_err(map_evidence_decode)?,
        byte_len: reader.u64().map_err(map_evidence_decode)?,
    };
    if bindings != input.bindings {
        return Err(MachineEffectEvidenceDecodeErrorV1::IdentityBindingMismatch);
    }
    if input_identity != input.identity() {
        return Err(MachineEffectEvidenceDecodeErrorV1::InputIdentityMismatch);
    }
    let entry_count = reader
        .bounded_count("evidence entry points", MAX_MACHINE_EFFECT_ENTRY_POINTS_V1)
        .map_err(map_evidence_decode)?;
    let effect_count = reader
        .bounded_count("evidence effects", MAX_MACHINE_EFFECT_EFFECTS_V1)
        .map_err(map_evidence_decode)?;
    reader
        .require_remaining(
            minimum_remaining_bytes(&[
                (entry_count, MIN_ENTRY_POINT_RECORD_BYTES_V1),
                (effect_count, MIN_EFFECT_RECORD_BYTES_V1),
            ])
            .map_err(map_evidence_decode)?,
        )
        .map_err(map_evidence_decode)?;
    let entries = decode_entries(&mut reader, entry_count).map_err(map_evidence_decode)?;
    let effects = decode_effects(&mut reader, effect_count).map_err(map_evidence_decode)?;
    if !reader.is_finished() {
        return Err(MachineEffectEvidenceDecodeErrorV1::DeclaredLengthMismatch {
            declared: reader.position(),
            actual: bytes.len(),
        });
    }
    validate_evidence_uniqueness(&entries, &effects)?;
    if entries != input.entries {
        return Err(MachineEffectEvidenceDecodeErrorV1::EntryPointsMismatch);
    }
    let expected = analyze_gfx942_machine_effects_v1(input)
        .map_err(|error| MachineEffectEvidenceDecodeErrorV1::Analysis(Box::new(error)))?;
    if effects != expected.effects {
        return Err(MachineEffectEvidenceDecodeErrorV1::EffectEvidenceMismatch);
    }
    if bytes != expected.canonical_bytes {
        return Err(MachineEffectEvidenceDecodeErrorV1::NonCanonical);
    }
    Ok(expected)
}

fn validate_evidence_uniqueness(
    entries: &[FinalizedEntryPointV1],
    effects: &[MachineEffectV1],
) -> Result<(), MachineEffectEvidenceDecodeErrorV1> {
    let mut ids = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    for entry in entries {
        validate_symbol(&entry.symbol).map_err(|_| {
            MachineEffectEvidenceDecodeErrorV1::InvalidEntrySymbol(entry.symbol.clone())
        })?;
        if !ids.insert(entry.id) {
            return Err(MachineEffectEvidenceDecodeErrorV1::DuplicateEntryPoint(
                entry.id,
            ));
        }
        if !symbols.insert(entry.symbol.clone()) {
            return Err(MachineEffectEvidenceDecodeErrorV1::DuplicateEntrySymbol(
                entry.symbol.clone(),
            ));
        }
    }
    let mut unique = BTreeSet::new();
    for effect in effects {
        if !unique.insert(effect.clone()) {
            return Err(MachineEffectEvidenceDecodeErrorV1::DuplicateEffect(
                effect.clone(),
            ));
        }
    }
    Ok(())
}

fn map_evidence_decode(error: MachineEffectDecodeErrorV1) -> MachineEffectEvidenceDecodeErrorV1 {
    match error {
        MachineEffectDecodeErrorV1::TooLarge { actual, maximum } => {
            MachineEffectEvidenceDecodeErrorV1::TooLarge { actual, maximum }
        }
        MachineEffectDecodeErrorV1::Truncated => MachineEffectEvidenceDecodeErrorV1::Truncated,
        MachineEffectDecodeErrorV1::InvalidDomain => {
            MachineEffectEvidenceDecodeErrorV1::InvalidDomain
        }
        MachineEffectDecodeErrorV1::DeclaredLengthMismatch { declared, actual } => {
            MachineEffectEvidenceDecodeErrorV1::DeclaredLengthMismatch { declared, actual }
        }
        MachineEffectDecodeErrorV1::UnsupportedFlags(flags) => {
            MachineEffectEvidenceDecodeErrorV1::UnsupportedFlags(flags)
        }
        MachineEffectDecodeErrorV1::InvalidTag { field, actual } => {
            MachineEffectEvidenceDecodeErrorV1::InvalidTag { field, actual }
        }
        MachineEffectDecodeErrorV1::InvalidUtf8 => MachineEffectEvidenceDecodeErrorV1::InvalidUtf8,
        MachineEffectDecodeErrorV1::AllocationFailed { field, requested } => {
            MachineEffectEvidenceDecodeErrorV1::AllocationFailed { field, requested }
        }
        MachineEffectDecodeErrorV1::InvalidInput(error) => match *error {
            MachineEffectInputErrorV1::CountBoundExceeded {
                field,
                actual,
                maximum,
            } => MachineEffectEvidenceDecodeErrorV1::CountBoundExceeded {
                field,
                actual,
                maximum,
            },
            MachineEffectInputErrorV1::InvalidEntrySymbol(symbol) => {
                MachineEffectEvidenceDecodeErrorV1::InvalidEntrySymbol(symbol)
            }
            other => MachineEffectEvidenceDecodeErrorV1::InvalidBindings(Box::new(other)),
        },
        MachineEffectDecodeErrorV1::NonCanonical => {
            MachineEffectEvidenceDecodeErrorV1::NonCanonical
        }
    }
}

fn push_u32(output: &mut Vec<u8>, value: usize) {
    output.extend_from_slice(&(value as u32).to_le_bytes());
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], MachineEffectDecodeErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(MachineEffectDecodeErrorV1::Truncated)?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or(MachineEffectDecodeErrorV1::Truncated)?;
        self.offset = end;
        Ok(result)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], MachineEffectDecodeErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| MachineEffectDecodeErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, MachineEffectDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, MachineEffectDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, MachineEffectDecodeErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn i64(&mut self) -> Result<i64, MachineEffectDecodeErrorV1> {
        Ok(i64::from_le_bytes(self.fixed()?))
    }

    fn bounded_count(
        &mut self,
        field: &'static str,
        maximum: usize,
    ) -> Result<usize, MachineEffectDecodeErrorV1> {
        let actual = self.u32()? as usize;
        if actual > maximum {
            return Err(invalid_input(
                MachineEffectInputErrorV1::CountBoundExceeded {
                    field,
                    actual,
                    maximum,
                },
            ));
        }
        Ok(actual)
    }

    fn require_remaining(&self, minimum: usize) -> Result<(), MachineEffectDecodeErrorV1> {
        if self.remaining() < minimum {
            Err(MachineEffectDecodeErrorV1::Truncated)
        } else {
            Ok(())
        }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    const fn position(&self) -> usize {
        self.offset
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
