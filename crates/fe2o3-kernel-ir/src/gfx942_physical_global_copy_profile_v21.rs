//! Whole actual-block physical global-copy proof for exact KIR21.
//! Conditional structure/provenance only: no source, runtime alias or launch authority.
use crate::{
    AccessMode, AddressSpace, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Function, FunctionRole,
    Gfx942PhysicalEntryRegisterV20 as Register, Gfx942PhysicalEntrySourceSiteVNext as Site,
    Gfx942PhysicalGlobalCopyDeclarationV1 as Declaration,
    Gfx942PhysicalGlobalCopyOpcodeV1 as Opcode, Gfx942PhysicalGlobalCopyStepV1 as Step, Module,
    Operation, OperationKind, ScalarType, TargetCapability, Terminator, Type, ValueId, WaveWidth,
    WorkgroupSize,
};
use std::collections::BTreeSet;
#[path = "gfx942_physical_global_copy_state_v21.rs"]
mod state;
#[path = "gfx942_physical_global_copy_steps_v21.rs"]
mod steps;
use state::{Fact, Phase, State};

/// More than160*159/2 identity +34*33/2 site comparisons and32*131 state visits,
/// including fixed strings/rosters. Prepaid before bounded state/definition growth.
const PROFILE_WORK_V21: usize = 262_144;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PhysicalGlobalCopyProfileErrorV21 {
    Resource(Resource),
    Invalid(&'static str),
}
impl From<Resource> for PhysicalGlobalCopyProfileErrorV21 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type Error = PhysicalGlobalCopyProfileErrorV21;
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
                        == crate::AMDGPU_GFX942_PHYSICAL_GLOBAL_COPY_CAPABILITY_NAMESPACE_V21
                        && name == crate::AMDGPU_GFX942_PHYSICAL_GLOBAL_COPY_CAPABILITY_NAME_V21)
            }
            _ => false,
        })
}
struct Definitions {
    ids: [ValueId; 160],
    count: usize,
}
impl Definitions {
    fn insert(&mut self, id: ValueId) -> Result<()> {
        require(self.count < 160, "global copy definition bound")?;
        require(
            !self.ids[..self.count].contains(&id),
            "global copy duplicate SSA definition",
        )?;
        self.ids[self.count] = id;
        self.count += 1;
        Ok(())
    }
}
struct Sites {
    values: [Site; 34],
    count: usize,
}
impl Sites {
    fn insert(&mut self, site: Site) -> Result<()> {
        require(
            crate::gfx942_physical_global_copy_v21::global_copy_site_v21(site)
                && self.count < 34
                && usize::from(site.occurrence) == self.count,
            "global copy dense bounded source census",
        )?;
        require(
            self.values[..self.count].iter().all(|old| {
                old.raw_block != site.raw_block
                    && old.semantic_block_index != site.semantic_block_index
            }),
            "global copy repeated raw/semantic source call block",
        )?;
        self.values[self.count] = site;
        self.count += 1;
        Ok(())
    }
}
pub(crate) fn check_gfx942_physical_global_copy_function_v21(
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
    budget.with_prepaid_scope(floor, 1, PROFILE_WORK_V21, storage, |_budget| {
        check(module, function)
    })
}
fn check(module: &Module, function: &Function) -> Result<()> {
    let [actual] = module.functions.as_slice() else {
        return Err(invalid("global copy exact one function"));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(invalid("global copy exact one kernel"));
    };
    require(
        std::ptr::eq(actual, function)
            && function.role == FunctionRole::KernelEntry
            && kernel.entry == function.id
            && kernel.id.as_str() == function.id.as_str(),
        "global copy actual entry identity",
    )?;
    let symbol = function.id.as_str().as_bytes();
    require(
        !symbol.is_empty()
            && symbol.len() <= 128
            && (symbol[0].is_ascii_alphabetic() || symbol[0] == b'_')
            && symbol
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || *byte == b'_'),
        "global copy bounded entry symbol",
    )?;
    require(
        capabilities(&module.required_capabilities)
            && capabilities(&function.required_capabilities)
            && capabilities(&kernel.required_capabilities),
        "global copy exact three capability scopes",
    )?;
    require(
        kernel.domain.rank() == 1 && kernel.workgroup_size == Some(WorkgroupSize::new(64, 1, 1)),
        "global copy exact launch rank/workgroup",
    )?;
    let [input, output] = function.signature.parameters.as_slice() else {
        return Err(invalid("global copy two logical parameters"));
    };
    for (ty, access) in [
        (input, AccessMode::ReadOnly),
        (output, AccessMode::ReadWrite),
    ] {
        require(
            matches!(ty,Type::Slice(slice) if slice.address_space==AddressSpace::Global
            && slice.access==access && *slice.element==Type::Scalar(ScalarType::U32)),
            "global copy exact input/output slice types",
        )?;
    }
    require(
        function.signature.results.is_empty(),
        "global copy void result",
    )?;
    let body = function
        .body
        .as_ref()
        .ok_or(invalid("global copy absent body"))?;
    let [block] = body.blocks.as_slice() else {
        return Err(invalid("global copy exact one block"));
    };
    require(
        body.parameters.len() == 2
            && block.id == BlockId(0)
            && block.parameters.is_empty()
            && (1..=32).contains(&block.operations.len()),
        "global copy block/operation bounds",
    )?;
    let first = &block.operations[0];
    let OperationKind::Gfx942PhysicalGlobalCopyDeclaration(declaration) = &first.kind else {
        return Err(invalid("global copy actual first declaration"));
    };
    declaration
        .validate_shape()
        .map_err(|_| invalid("global copy declaration shape"))?;
    require(
        declaration.parameters.as_slice() == body.parameters.as_slice(),
        "global copy exact declaration parameter join",
    )?;
    let mut definitions = Definitions {
        ids: [ValueId(0); 160],
        count: 0,
    };
    for value in &body.parameters {
        definitions.insert(*value)?;
    }
    for operation in &block.operations {
        require(
            operation.results.len() <= if std::ptr::eq(operation, first) { 5 } else { 4 },
            "global copy result bound",
        )?;
        for value in &operation.results {
            definitions.insert(value.id)?;
        }
    }
    let mut sites = Sites {
        values: [Site::ZERO; 34],
        count: 0,
    };
    sites.insert(declaration.begin_site)?;
    sites.insert(declaration.block.label_site)?;
    let mut state = state::initialize(first)?;
    let mut native = 0u8;
    for operation in block.operations.iter().skip(1) {
        let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &operation.kind else {
            return Err(invalid("global copy unexpected executable operation"));
        };
        sites.insert(step.site)?;
        require(
            step.native_ordinal == native,
            "global copy dense native ordinal",
        )?;
        native = native
            .checked_add(1)
            .ok_or(invalid("global copy ordinal overflow"))?;
        require(native < 32, "global copy nonterminal native bound")?;
        steps::verify_step(&mut state, operation, step)?;
    }
    sites.insert(declaration.block.terminator_site)?;
    require(
        declaration.block.native_ordinal == Some(native)
            && declaration.native_instruction_count == native + 1,
        "global copy exact terminal/native census",
    )?;
    require(
        sites.count == usize::from(declaration.native_instruction_count) + 2,
        "global copy exact source/native occurrence count",
    )?;
    require(
        state.phase == Phase::End
            && state.ready()
            && state.read.is_some()
            && state.kernarg_roster == 15
            && matches!(state.get(Register::Exec)?.fact, Fact::FullExec(_))
            && matches!(&block.terminator,Some(Terminator::Return{values}) if values.is_empty()),
        "global copy exact waited/restored Return terminal",
    )
}

/// Membership of the actual immutable verified owner, not source or host authority.
pub fn gfx942_physical_global_copy_declaration_v21(
    owner: &crate::VerifiedCanonicalKernelIrModuleV21,
) -> Option<&Declaration> {
    let [function] = owner.module().functions.as_slice() else {
        return None;
    };
    let body = function.body.as_ref()?;
    match &body.blocks.first()?.operations.first()?.kind {
        OperationKind::Gfx942PhysicalGlobalCopyDeclaration(declaration) => Some(declaration),
        _ => None,
    }
}
