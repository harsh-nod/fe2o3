//! Whole actual-SSA/CFG physical-entry proof for exact KIR20.
//! This is conditional structural ABI safety, not authenticated source custody.
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole,
    Gfx942PhysicalEntryBranchEncodingVNext as Encoding,
    Gfx942PhysicalEntryDeclarationVNext as Declaration, Gfx942PhysicalEntryOpcodeV20 as Opcode,
    Gfx942PhysicalEntryRegisterV20 as Register, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalEntryStepVNext as Step, Module, Operation, OperationKind, ScalarType,
    TargetCapability, Terminator, Type, ValueId, WaveWidth, WorkgroupSize,
};
use std::collections::BTreeSet;
#[path = "gfx942_physical_entry_state_v20.rs"]
mod state;
#[path = "gfx942_physical_entry_steps_v20.rs"]
mod steps;
use state::{Fact, Phase, State};

/// Includes <=768*767/2 identity comparisons, <=73*72/2 source coordinate
/// comparisons, <=64*131 state operations, four 131-cell joins and fixed strings.
const PROFILE_WORK_V20: usize = 524_288;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalEntryProfileErrorV20 {
    Resource(Resource),
    Invalid(&'static str),
}
impl From<Resource> for PhysicalEntryProfileErrorV20 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type Error = PhysicalEntryProfileErrorV20;
type Result<T> = std::result::Result<T, Error>;
const fn invalid(message: &'static str) -> Error {
    Error::Invalid(message)
}
fn require(value: bool, message: &'static str) -> Result<()> {
    if value { Ok(()) } else { Err(invalid(message)) }
}
fn capabilities(values: &BTreeSet<TargetCapability>) -> bool {
    values.len() == 3
        && values.iter().all(|value| match value {
            TargetCapability::WaveWidth(WaveWidth::Wave64) => true,
            TargetCapability::Extension { namespace, name } => {
                (namespace == crate::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && name == crate::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME)
                    || (namespace == crate::AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAMESPACE_V20
                        && name == crate::AMDGPU_GFX942_PHYSICAL_ENTRY_CAPABILITY_NAME_V20)
            }
            _ => false,
        })
}
struct Definitions {
    ids: [ValueId; 768],
    count: usize,
}
impl Definitions {
    fn insert(&mut self, value: ValueId) -> Result<()> {
        require(self.count < self.ids.len(), "physical definition bound")?;
        require(
            !self.ids[..self.count].contains(&value),
            "physical duplicate SSA definition",
        )?;
        self.ids[self.count] = value;
        self.count += 1;
        Ok(())
    }
}
struct Sites {
    values: [Site; 73],
    count: usize,
}
impl Sites {
    fn insert(&mut self, site: Site) -> Result<()> {
        require(
            site.is_complete() && self.count < 73 && usize::from(site.occurrence) == self.count,
            "physical exact dense source census",
        )?;
        require(
            self.values[..self.count].iter().all(|old| {
                old.raw_block != site.raw_block
                    && old.semantic_block_index != site.semantic_block_index
            }),
            "physical repeated raw/semantic source call block",
        )?;
        self.values[self.count] = site;
        self.count += 1;
        Ok(())
    }
}

pub(crate) fn check_gfx942_physical_entry_function_v20(
    module: &Module,
    function: &Function,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let storage = std::mem::size_of::<[State; 5]>()
        .checked_add(std::mem::size_of::<Definitions>())
        .and_then(|n| n.checked_add(std::mem::size_of::<Sites>()))
        .and_then(|n| n.checked_add(2048))
        .ok_or(Resource::Arithmetic)?;
    let floor = budget.storage_checkpoint();
    budget.with_prepaid_scope(floor, 1, PROFILE_WORK_V20, storage, |_budget| {
        check(module, function)
    })
}
fn check(module: &Module, function: &Function) -> Result<()> {
    let [actual] = module.functions.as_slice() else {
        return Err(invalid("physical exact one function"));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(invalid("physical exact one kernel"));
    };
    require(
        std::ptr::eq(actual, function)
            && function.role == FunctionRole::KernelEntry
            && kernel.entry == function.id
            && kernel.id.as_str() == function.id.as_str(),
        "physical actual entry identity",
    )?;
    let symbol = function.id.as_str().as_bytes();
    require(
        !symbol.is_empty()
            && symbol.len() <= 128
            && (symbol[0].is_ascii_alphabetic() || symbol[0] == b'_')
            && symbol
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_'),
        "physical bounded entry symbol",
    )?;
    require(
        capabilities(&module.required_capabilities)
            && capabilities(&function.required_capabilities)
            && capabilities(&kernel.required_capabilities),
        "physical exact three capability scopes",
    )?;
    require(
        kernel.domain.rank() == 1 && kernel.workgroup_size == Some(WorkgroupSize::new(64, 1, 1)),
        "physical exact launch rank/workgroup",
    )?;
    let [output, a, b, c, selector] = function.signature.parameters.as_slice() else {
        return Err(invalid("physical logical parameter roster"));
    };
    require(
        matches!(output, Type::Slice(slice) if slice.address_space == AddressSpace::Global
        && slice.access == AccessMode::ReadWrite && *slice.element == Type::Scalar(ScalarType::U32))
            && [a, b, c, selector]
                .iter()
                .all(|ty| **ty == Type::Scalar(ScalarType::U32))
            && function.signature.results.is_empty(),
        "physical logical signature",
    )?;
    let body = function
        .body
        .as_ref()
        .ok_or(invalid("physical body absent"))?;
    require(
        matches!(body.blocks.len(), 1 | 4) && body.parameters.len() == 5,
        "physical one block or selector diamond",
    )?;
    let entry = &body.blocks[0];
    let first = entry
        .operations
        .first()
        .ok_or(invalid("physical declaration absent"))?;
    let OperationKind::Gfx942PhysicalEntryDeclaration(declaration) = &first.kind else {
        return Err(invalid("physical actual entry declaration"));
    };
    declaration
        .validate_shape()
        .map_err(|_| invalid("physical declaration shape"))?;
    require(
        declaration.parameters.as_slice() == body.parameters.as_slice()
            && usize::from(declaration.block_count) == body.blocks.len(),
        "physical declaration parameter/block join",
    )?;
    let mut definitions = Definitions {
        ids: [ValueId(0); 768],
        count: 0,
    };
    for value in &body.parameters {
        definitions.insert(*value)?;
    }
    let mut total = 0usize;
    for (ordinal, block) in body.blocks.iter().enumerate() {
        require(
            block.id == BlockId(ordinal as u32)
                && block.parameters.len() <= 131
                && block.operations.len() <= 65,
            "physical block/resource bound",
        )?;
        total += block.operations.len();
        require(total <= 65, "physical total operation bound")?;
        if ordinal != 3 {
            require(
                block.parameters.is_empty(),
                "physical unexpected entry/arm parameters",
            )?;
        }
        for value in &block.parameters {
            definitions.insert(value.id)?;
        }
        for operation in &block.operations {
            require(
                operation.results.len()
                    <= if ordinal == 0 && std::ptr::eq(operation, first) {
                        5
                    } else {
                        4
                    },
                "physical operation result bound",
            )?;
            for value in &operation.results {
                definitions.insert(value.id)?;
            }
        }
    }
    let mut sites = Sites {
        values: [Site::ZERO; 73],
        count: 0,
    };
    sites.insert(declaration.begin_site)?;
    let mut outgoing = [State::EMPTY; 4];
    let mut native = 0u8;
    let select = body.blocks.len() == 4;
    for (ordinal, block) in body.blocks.iter().enumerate() {
        sites.insert(declaration.blocks[ordinal].label_site)?;
        let mut state = match ordinal {
            0 => state::initialize(first)?,
            1 | 2 => outgoing[0],
            3 => {
                let arguments = [
                    edge_arguments(&body.blocks[1])?,
                    edge_arguments(&body.blocks[2])?,
                ];
                state::join([&outgoing[1], &outgoing[2]], block, arguments)?
            }
            _ => return Err(invalid("physical block count")),
        };
        for operation in block.operations.iter().skip(usize::from(ordinal == 0)) {
            let OperationKind::Gfx942PhysicalEntryStep(step) = &operation.kind else {
                return Err(invalid("physical unexpected executable operation"));
            };
            sites.insert(step.site)?;
            require(
                step.native_ordinal == native,
                "physical dense instruction ordinal",
            )?;
            native = native
                .checked_add(1)
                .ok_or(invalid("physical instruction count overflow"))?;
            require(native <= 64, "physical instruction bound")?;
            steps::verify_step(
                &mut state,
                operation,
                step,
                select,
                ordinal + 1 == body.blocks.len(),
            )?;
        }
        let contract = declaration.blocks[ordinal];
        sites.insert(contract.terminator_site)?;
        if contract.encoding == Encoding::Fallthrough {
            require(
                contract.native_ordinal.is_none(),
                "physical fallthrough native ordinal",
            )?;
        } else {
            require(
                contract.native_ordinal == Some(native),
                "physical terminator native ordinal",
            )?;
            native = native
                .checked_add(1)
                .ok_or(invalid("physical native count overflow"))?;
        }
        terminator(block, ordinal, select, contract.encoding, &state)?;
        outgoing[ordinal] = state;
    }
    require(
        native == declaration.native_instruction_count && native <= 64,
        "physical exact native instruction count",
    )
}
fn edge_arguments(block: &BasicBlock) -> Result<&[ValueId]> {
    match &block.terminator {
        Some(Terminator::Branch {
            target: BlockId(3),
            arguments,
        }) => Ok(arguments),
        _ => Err(invalid("physical arm must join actual block3")),
    }
}
fn terminator(
    block: &BasicBlock,
    ordinal: usize,
    select: bool,
    encoding: Encoding,
    state: &State,
) -> Result<()> {
    require(state.ready(), "physical pending loads at control boundary")?;
    if select && ordinal == 0 {
        state.full()?;
        let current = state.get(Register::Scc)?;
        require(
            current.fact == Fact::SelectorZero,
            "physical branch needs current selector SCC",
        )?;
        return require(
            matches!(&block.terminator,
            Some(Terminator::ConditionalBranch { condition, then_target: BlockId(2), then_arguments,
                else_target: BlockId(1), else_arguments })
                if *condition == current.id && then_arguments.is_empty() && else_arguments.is_empty())
                && encoding == Encoding::Scc1,
            "physical exact SCC diamond edges/fallthrough",
        );
    }
    if select && matches!(ordinal, 1 | 2) {
        state.full()?;
        return require(
            matches!(
                &block.terminator,
                Some(Terminator::Branch {
                    target: BlockId(3),
                    ..
                })
            ) && encoding
                == if ordinal == 1 {
                    Encoding::Jump
                } else {
                    Encoding::Fallthrough
                },
            "physical exact arm encoding/edge",
        );
    }
    require(
        state.phase == Phase::End
            && matches!(state.get(Register::Exec)?.fact, Fact::FullExec(_))
            && encoding == Encoding::Endpgm0
            && matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty()),
        "physical exact waited/restored terminal",
    )
}

/// Read-only membership of an already fully verified exact canonical owner.
/// Does not grant source, pointer, artifact or launch authority.
pub fn gfx942_physical_entry_declaration_v20(
    owner: &crate::VerifiedCanonicalKernelIrModuleV20,
) -> Option<&Declaration> {
    let [function] = owner.module().functions.as_slice() else {
        return None;
    };
    let body = function.body.as_ref()?;
    match &body.blocks.first()?.operations.first()?.kind {
        OperationKind::Gfx942PhysicalEntryDeclaration(declaration) => Some(declaration),
        _ => None,
    }
}
