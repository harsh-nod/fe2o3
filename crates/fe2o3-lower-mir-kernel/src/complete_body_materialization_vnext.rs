//! Sole-importer canonical construction for the exact complete-body profile.
//! Input facts are inert. Only the backend retains actual source custody; this
//! constructor neither authenticates those facts nor grants a verified owner.
//! No Module/Kernel/launch/proof/emitter constructor is exposed by this leaf.
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole,
    Gfx942CompleteBodyBlockV1 as SourceBlock, Gfx942CompleteBodyOriginVNext,
    Gfx942CompleteBodyPackedV1, Gfx942CompleteBodyTerminatorV1 as End,
    Gfx942OrderedProgramRegistersV1, Gfx942ProgramRoleV1 as Role, ScalarType, Signature, Type,
    ValueDef, ValueId,
};

#[path = "complete_body_materialization_emit_vnext.rs"]
mod emit;
#[path = "complete_body_materialization_storage_vnext.rs"]
mod storage;
use storage::Scope;

/// Logical ABI/identity input to the sole importer, not a source receipt.
pub struct CompleteBodyCanonicalInputVNext<'a> {
    /// Exact source-origin observations; not independently authenticated.
    pub origin: Gfx942CompleteBodyOriginVNext,
    /// Bounded source export symbol.
    pub export_name: &'a str,
    /// Physical authored VGPR-role assignment.
    pub registers: Gfx942OrderedProgramRegistersV1,
    /// Fixed packed source instruction and block records.
    pub packed: Gfx942CompleteBodyPackedV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Closed shape, packing, register or resource refusal.
pub enum CompleteBodyMaterializationErrorVNext {
    /// The caller's cumulative resource ledger refused.
    Resource(Resource),
    /// At least one required source-origin identity is absent.
    IncompleteOrigin,
    /// The export symbol is outside the supported grammar.
    ExportName,
    /// An authored role overlaps the compiler-reserved VGPR prefix.
    Registers,
    /// The packed instruction or block grammar is invalid.
    Packing,
    /// The graph is not one block or the exact four-block diamond.
    UnsupportedShape,
    /// An authored source label repeats.
    DuplicateLabel,
    /// A role is read without a definition on every incoming path.
    UndefinedRole,
    /// Reconstruction or retained-storage accounting differs.
    InternalAccounting,
}
impl From<Resource> for CompleteBodyMaterializationErrorVNext {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl std::fmt::Display for CompleteBodyMaterializationErrorVNext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pending complete-body materialization: {self:?}")
    }
}
impl std::error::Error for CompleteBodyMaterializationErrorVNext {}
type Error = CompleteBodyMaterializationErrorVNext;

/// This receipt counts logical requested heap payload and this owner header.
/// It excludes allocator excess, borrowed source owners, stack and rustc/RSS.
/// The constructor restores the incoming floor; reserve this receipt while
/// retaining the returned pending owner alongside subsequent checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteBodyMaterializationStorageVNext {
    bytes: usize,
}
impl CompleteBodyMaterializationStorageVNext {
    /// Unreserved logical retained payload in bytes.
    pub const fn retained_storage(self) -> usize {
        self.bytes
    }
}

/// Move-only pending canonical function and exact source-ordinal placement.
/// No public into-Module/verified-owner/emitter conversion. Readonly inspection
/// is inert; the registered source owner adds full source/SSA/general/effect
/// verification before normal checked continuation can consume this function.
pub struct PendingCompleteBodyMaterializationVNext {
    function: Function,
    source_labels: [u8; 8],
    authored_blocks: u8,
    authored_instructions: u8,
    storage: CompleteBodyMaterializationStorageVNext,
}
impl PendingCompleteBodyMaterializationVNext {
    /// Readonly pending function; this alone grants no executable custody.
    pub fn function(&self) -> &Function {
        &self.function
    }
    /// Crate-private transport only. The source importer reconstructs this from
    /// its retained semantic owner; no public checked-owner/Module conversion.
    pub(crate) fn into_function_for_source_import_vnext(self) -> Function {
        self.function
    }

    /// Authored labels in canonical block order.
    pub fn source_labels(&self) -> &[u8] {
        &self.source_labels[..usize::from(self.authored_blocks)]
    }
    /// Number of authored instructions, excluding the compiler-owned tail.
    pub const fn instruction_count(&self) -> u8 {
        self.authored_instructions
    }
    /// Unreserved logical pending-function storage receipt.
    pub const fn storage(&self) -> CompleteBodyMaterializationStorageVNext {
        self.storage
    }
    /// Always false: pending construction grants no publication or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Exact reconstruction, not digest-only matching. The caller must reserve
    /// this live owner's receipt first; independent new scratch is then prepaid.
    /// Rebuilding never grants missing source or canonical verifier authority.
    pub fn verify_reconstruction(
        &self,
        input: &CompleteBodyCanonicalInputVNext<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        if budget.storage() < self.storage.bytes {
            return Err(Error::InternalAccounting);
        }
        budget.charge_work(MATERIALIZATION_WORK)?;
        let rebuilt = materialize_complete_body_function_vnext(input, budget)?;
        let mut compare_scope = Scope::new(budget);
        compare_scope.reserve(rebuilt.storage.bytes)?;
        let differs = self.function != rebuilt.function
            || self.source_labels != rebuilt.source_labels
            || self.authored_blocks != rebuilt.authored_blocks
            || self.authored_instructions != rebuilt.authored_instructions;
        drop(rebuilt);
        if differs {
            Err(Error::InternalAccounting)
        } else {
            Ok(())
        }
    }
}

/// Fixed prepaid work for one bounded materialization/reconstruction.
pub const COMPLETE_BODY_MATERIALIZATION_WORK_VNEXT: usize = 4096;
const MATERIALIZATION_WORK: usize = COMPLETE_BODY_MATERIALIZATION_WORK_VNEXT;
const PARAMETERS: [ValueId; 5] = [ValueId(0), ValueId(1), ValueId(2), ValueId(3), ValueId(4)];
const EMPTY: SourceBlock<'static> = SourceBlock {
    label: fe2o3_kernel_ir::Gfx942CompleteBodyLabelV1(0),
    instructions: &[],
    terminator: End::GuardedStoreOutputAndEnd,
};

/// Builds one real KIR CFG/SSA, not an executable descriptor interpreter.
/// Supported subset: one terminal block, or four-block selector diamond with
/// entry->arms(1,2)->merge(3). All source labels and instruction ordinals remain
/// explicit. The registered KIR19 inverse and closed profile verify the result.
pub fn materialize_complete_body_function_vnext(
    input: &CompleteBodyCanonicalInputVNext<'_>,
    budget: &mut Budget<'_>,
) -> Result<PendingCompleteBodyMaterializationVNext, Error> {
    // Fixed upper bound prepays decoder, bounded shape/SSA passes, string copy
    // and later small structural comparison. Never debit only after traversal.
    budget.charge_work(MATERIALIZATION_WORK)?;
    if !input.origin.is_complete() {
        return Err(Error::IncompleteOrigin);
    }
    if input.export_name.is_empty()
        || input.export_name.len() > 256
        || !input
            .export_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(Error::ExportName);
    }
    if [
        input.registers.scratch(),
        input.registers.output(),
        input.registers.inputs()[0],
        input.registers.inputs()[1],
        input.registers.inputs()[2],
    ]
    .into_iter()
    .any(|register| register < 8)
    {
        return Err(Error::Registers);
    }
    let decoded = input.packed.decode().map_err(|_| Error::Packing)?;
    let mut sources = [EMPTY; 8];
    for (place, value) in sources.iter_mut().zip(decoded.blocks()) {
        *place = value;
    }
    let count = decoded.block_count();
    let sources = &sources[..count];
    check_shape(sources)?;
    let mut scope = Scope::new(budget);
    scope.reserve(std::mem::size_of::<PendingCompleteBodyMaterializationVNext>())?;
    let mut labels = [0_u8; 8];
    let mut masks_in = [0_u8; 8];
    let mut masks_out = [0_u8; 8];
    for (ordinal, source) in sources.iter().enumerate() {
        labels[ordinal] = source.label.0;
        masks_in[ordinal] = match ordinal {
            0 => 0b00111,
            1 | 2 => masks_out[0],
            3 => masks_out[1] & masks_out[2],
            _ => return Err(Error::UnsupportedShape),
        };
        let mut mask = masks_in[ordinal];
        for instruction in source.instructions {
            // This is required to construct the SSA role environment; actual
            // source admission still runs the unchanged Layer A model checker.
            let reads = match instruction {
                fe2o3_kernel_ir::Gfx942ProgramInstructionV1::Move { source, .. } => bit(*source),
                fe2o3_kernel_ir::Gfx942ProgramInstructionV1::Binary { left, right, .. } => {
                    bit(*left) | bit(*right)
                }
            };
            if reads & !mask != 0 {
                return Err(Error::UndefinedRole);
            }
            mask |= bit(instruction.destination().role());
        }
        if source.terminator == End::GuardedStoreOutputAndEnd && mask & bit(Role::Output) == 0 {
            return Err(Error::UndefinedRole);
        }
        masks_out[ordinal] = mask;
    }
    let mut next = 5_u32;
    let mut incoming = [[None; 5]; 8];
    let mut blocks = scope.vec::<BasicBlock>(count)?;
    for ordinal in 0..count {
        let mut block = BasicBlock::new(BlockId(ordinal as u32));
        incoming[ordinal][..3].copy_from_slice(&[
            Some(PARAMETERS[1]),
            Some(PARAMETERS[2]),
            Some(PARAMETERS[3]),
        ]);
        let parameters = if ordinal == 0 {
            0
        } else {
            (masks_in[ordinal] & 0b11000).count_ones() as usize
        };
        block.parameters = scope.vec(parameters)?;
        for (role, incoming_role) in incoming[ordinal].iter_mut().enumerate().skip(3) {
            if ordinal != 0 && masks_in[ordinal] & (1 << role) != 0 {
                let value = fresh(&mut next)?;
                block
                    .parameters
                    .push(ValueDef::new(value, Type::Scalar(ScalarType::U32)));
                *incoming_role = Some(value);
            }
        }
        blocks.push(block);
    }
    let mut authored = 0_u8;
    for (ordinal, block) in blocks.iter_mut().enumerate() {
        emit::fill_block(
            input,
            sources,
            ordinal,
            incoming,
            block,
            &mut next,
            &mut authored,
            &mut scope,
        )?;
    }
    if usize::from(authored) != decoded.instruction_count() {
        return Err(Error::InternalAccounting);
    }
    let mut parameter_types = scope.vec(5)?;
    // Existing KIR logical DisjointSlice representation; provenance remains in
    // the retained source owner, not in this Slice type or a new alias flag.
    scope.reserve(std::mem::size_of::<Type>())?;
    parameter_types.push(Type::slice(
        Type::Scalar(ScalarType::U32),
        fe2o3_kernel_ir::AddressSpace::Global,
        fe2o3_kernel_ir::AccessMode::ReadWrite,
    ));
    for _ in 0..4 {
        parameter_types.push(Type::Scalar(ScalarType::U32));
    }
    let parameters = scope.copy(&PARAMETERS)?;
    let name = scope.string(input.export_name)?;
    let function = Function {
        id: name.into(),
        signature: Signature::new(parameter_types, Vec::new()),
        role: FunctionRole::KernelEntry,
        body: Some(fe2o3_kernel_ir::FunctionBody { parameters, blocks }),
        required_capabilities: std::collections::BTreeSet::new(),
    };
    // No kernel/launch domain is invented: the registered normal module builder
    // must consume current authenticated source-launch/root agreement.
    let storage = CompleteBodyMaterializationStorageVNext {
        bytes: scope.bytes(),
    };
    Ok(PendingCompleteBodyMaterializationVNext {
        function,
        source_labels: labels,
        authored_blocks: count as u8,
        authored_instructions: authored,
        storage,
    })
}

fn bit(role: Role) -> u8 {
    1 << role as u8
}
fn fresh(next: &mut u32) -> Result<ValueId, Error> {
    let value = ValueId(*next);
    *next = next.checked_add(1).ok_or(Error::InternalAccounting)?;
    Ok(value)
}
fn check_shape(blocks: &[SourceBlock<'_>]) -> Result<(), Error> {
    for (ordinal, block) in blocks.iter().enumerate() {
        if blocks[..ordinal]
            .iter()
            .any(|previous| previous.label == block.label)
        {
            return Err(Error::DuplicateLabel);
        }
    }
    match blocks {
        [only] if only.terminator == End::GuardedStoreOutputAndEnd => Ok(()),
        [entry, zero, nonzero, merge]
            if entry.terminator
                == (End::BranchSelectorZero {
                    zero: zero.label,
                    nonzero: nonzero.label,
                })
                && zero.terminator == End::Jump(merge.label)
                && nonzero.terminator == End::Jump(merge.label)
                && merge.terminator == End::GuardedStoreOutputAndEnd =>
        {
            Ok(())
        }
        _ => Err(Error::UnsupportedShape),
    }
}

#[cfg(test)]
#[path = "complete_body_materialization_vnext_tests.rs"]
mod tests;
