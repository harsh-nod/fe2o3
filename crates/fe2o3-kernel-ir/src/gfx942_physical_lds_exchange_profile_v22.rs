//! Whole actual-block physical LDS-exchange proof for exact KIR22.
//! Conditional structure/provenance only: no source, runtime alias or launch authority.
use crate::{
    AccessMode, AddressSpace, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole,
    Gfx942PhysicalEntryRegisterV20 as Register, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalLdsExchangeDeclarationV1 as Declaration,
    Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode, Gfx942PhysicalLdsExchangeStepV1 as Step, Module,
    Operation, OperationKind, ScalarType, TargetCapability, Terminator, Type, ValueId, WaveWidth,
    WorkgroupSize,
};
use std::collections::BTreeSet;
#[path = "gfx942_physical_lds_exchange_state_v22.rs"]
mod state;
#[path = "gfx942_physical_lds_exchange_steps_v22.rs"]
mod steps;
use state::{Fact, Phase, State};

/// Covers192*191/2 identity +42*41/2 site comparisons and40*131 state visits,
/// including fixed strings/rosters. Prepaid before bounded state/definition growth.
const PROFILE_WORK_V22: usize = 262_144;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalLdsExchangeProfileErrorV22 {
    Resource(Resource),
    Invalid(&'static str),
}
impl From<Resource> for PhysicalLdsExchangeProfileErrorV22 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type Error = PhysicalLdsExchangeProfileErrorV22;
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
                    || (namespace
                        == crate::AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAMESPACE_V22
                        && name == crate::AMDGPU_GFX942_PHYSICAL_LDS_EXCHANGE_CAPABILITY_NAME_V22)
            }
            _ => false,
        })
}
struct Definitions {
    ids: [ValueId; 192],
    count: usize,
}
impl Definitions {
    fn insert(&mut self, id: ValueId) -> Result<()> {
        require(self.count < 192, "LDS exchange definition bound")?;
        require(
            !self.ids[..self.count].contains(&id),
            "LDS exchange duplicate SSA definition",
        )?;
        self.ids[self.count] = id;
        self.count += 1;
        Ok(())
    }
}
struct Sites {
    values: [Site; 42],
    count: usize,
}
impl Sites {
    fn insert(&mut self, site: Site) -> Result<()> {
        require(
            crate::gfx942_physical_lds_exchange_v22::lds_exchange_site_v22(site)
                && self.count < 42
                && usize::from(site.occurrence) == self.count,
            "LDS exchange dense bounded source census",
        )?;
        require(
            self.values[..self.count].iter().all(|old| {
                old.raw_block != site.raw_block
                    && old.semantic_block_index != site.semantic_block_index
            }),
            "LDS exchange repeated raw/semantic source call block",
        )?;
        self.values[self.count] = site;
        self.count += 1;
        Ok(())
    }
}
pub(crate) fn check_gfx942_physical_lds_exchange_function_v22(
    module: &Module,
    function: &Function,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let storage = std::mem::size_of::<State>()
        .checked_add(std::mem::size_of::<Definitions>())
        .and_then(|n| n.checked_add(std::mem::size_of::<Sites>()))
        .and_then(|n| n.checked_add(4096))
        .ok_or(Resource::Arithmetic)?;
    let floor = budget.storage_checkpoint();
    budget.with_prepaid_scope(floor, 1, PROFILE_WORK_V22, storage, |_budget| {
        check(module, function)
    })
}
fn check(module: &Module, function: &Function) -> Result<()> {
    let [actual] = module.functions.as_slice() else {
        return Err(invalid("LDS exchange exact one function"));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(invalid("LDS exchange exact one kernel"));
    };
    require(
        std::ptr::eq(actual, function)
            && function.role == FunctionRole::KernelEntry
            && kernel.entry == function.id
            && kernel.id.as_str() == function.id.as_str(),
        "LDS exchange actual entry identity",
    )?;
    let symbol = function.id.as_str().as_bytes();
    require(
        !symbol.is_empty()
            && symbol.len() <= 128
            && (symbol[0].is_ascii_alphabetic() || symbol[0] == b'_')
            && symbol
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_'),
        "LDS exchange bounded entry symbol",
    )?;
    require(
        capabilities(&module.required_capabilities)
            && capabilities(&function.required_capabilities)
            && capabilities(&kernel.required_capabilities),
        "LDS exchange exact three capability scopes",
    )?;
    require(
        kernel.domain.rank() == 1 && kernel.workgroup_size == Some(WorkgroupSize::new(128, 1, 1)),
        "LDS exchange exact launch rank/workgroup",
    )?;
    let [input, output] = function.signature.parameters.as_slice() else {
        return Err(invalid("LDS exchange two logical parameters"));
    };
    for (ty, access) in [
        (input, AccessMode::ReadOnly),
        (output, AccessMode::ReadWrite),
    ] {
        require(
            matches!(ty,Type::Slice(slice) if slice.address_space==AddressSpace::Global
            && slice.access==access && *slice.element==Type::Scalar(ScalarType::U32)),
            "LDS exchange exact input/output slice types",
        )?;
    }
    require(
        function.signature.results.is_empty(),
        "LDS exchange void result",
    )?;
    let body = function
        .body
        .as_ref()
        .ok_or(invalid("LDS exchange absent body"))?;
    let [block] = body.blocks.as_slice() else {
        return Err(invalid("LDS exchange exact one block"));
    };
    require(
        body.parameters.len() == 2
            && block.id == BlockId(0)
            && block.parameters.is_empty()
            && block.operations.len() == 32,
        "LDS exchange block/operation bounds",
    )?;
    let first = &block.operations[0];
    let OperationKind::Gfx942PhysicalLdsExchangeDeclaration(declaration) = &first.kind else {
        return Err(invalid("LDS exchange actual first declaration"));
    };
    require(
        declaration.native_instruction_count == 32,
        "LDS exchange exact initial32 native rows",
    )?;
    declaration
        .validate_shape()
        .map_err(|_| invalid("LDS exchange declaration shape"))?;
    require(
        declaration.parameters.as_slice() == body.parameters.as_slice(),
        "LDS exchange exact declaration parameter join",
    )?;
    let mut definitions = Definitions {
        ids: [ValueId(0); 192],
        count: 0,
    };
    for value in &body.parameters {
        definitions.insert(*value)?;
    }
    for operation in &block.operations {
        require(
            operation.results.len() <= if std::ptr::eq(operation, first) { 5 } else { 4 },
            "LDS exchange result bound",
        )?;
        for value in &operation.results {
            definitions.insert(value.id)?;
        }
    }
    let mut sites = Sites {
        values: [Site::ZERO; 42],
        count: 0,
    };
    sites.insert(declaration.begin_site)?;
    sites.insert(declaration.block.label_site)?;
    let mut state = state::initialize(first)?;
    let mut native = 0u8;
    for operation in block.operations.iter().skip(1) {
        let OperationKind::Gfx942PhysicalLdsExchangeStep(step) = &operation.kind else {
            return Err(invalid("LDS exchange unexpected executable operation"));
        };
        sites.insert(step.site)?;
        require(
            step.native_ordinal == native,
            "LDS exchange dense native ordinal",
        )?;
        native = native
            .checked_add(1)
            .ok_or(invalid("LDS exchange ordinal overflow"))?;
        require(native < 40, "LDS exchange nonterminal native bound")?;
        steps::verify_step(&mut state, operation, step)?;
    }
    sites.insert(declaration.block.terminator_site)?;
    require(
        declaration.block.native_ordinal == Some(native)
            && declaration.native_instruction_count == native + 1,
        "LDS exchange exact terminal/native census",
    )?;
    require(
        sites.count == usize::from(declaration.native_instruction_count) + 2,
        "LDS exchange exact source/native occurrence count",
    )?;
    require(
        state.phase == Phase::End
            && state.ready()
            && state.read.is_some()
            && state.lds_write.is_some()
            && state.publication.is_some()
            && state.lds_read.is_some()
            && state.kernarg_roster == 15
            && matches!(state.get(Register::Exec)?.fact, Fact::FullExec(_))
            && matches!(&block.terminator,Some(Terminator::Return{values}) if values.is_empty()),
        "LDS exchange exact waited/restored Return terminal",
    )
}

/// Membership of the actual immutable verified owner, not source or host authority.
pub fn gfx942_physical_lds_exchange_declaration_v22(
    owner: &crate::VerifiedCanonicalKernelIrModuleV22,
) -> Option<&Declaration> {
    let [function] = owner.module().functions.as_slice() else {
        return None;
    };
    let body = function.body.as_ref()?;
    match &body.blocks.first()?.operations.first()?.kind {
        OperationKind::Gfx942PhysicalLdsExchangeDeclaration(declaration) => Some(declaration),
        _ => None,
    }
}
