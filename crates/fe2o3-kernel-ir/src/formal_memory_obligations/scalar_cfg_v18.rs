use super::*;
use crate::{FunctionRole, StorageLayoutKindV1, Terminator, VerifiedCanonicalKernelIrModuleV18};

/// A refusal from the bounded, actual-owner scalar CFG formal entrance.
#[derive(Debug)]
pub enum CanonicalScalarCfgFormalErrorV18 {
    Unsupported(&'static str),
    Formal(FormalMemoryObligationError),
}

impl fmt::Display for CanonicalScalarCfgFormalErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical scalar CFG formal V18: {self:?}")
    }
}

impl Error for CanonicalScalarCfgFormalErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Unsupported(_) => None,
            Self::Formal(error) => Some(error),
        }
    }
}

type ResultV18<T> = Result<T, CanonicalScalarCfgFormalErrorV18>;
const MAX_NODES_V18: usize = 65_536;
const MAX_WORK_V18: usize = 1_048_576;

fn add_v18(total: &mut usize, count: usize) -> ResultV18<()> {
    *total = total
        .checked_add(count)
        .filter(|&value| value <= MAX_NODES_V18)
        .ok_or(CanonicalScalarCfgFormalErrorV18::Unsupported(
            "formal module node bound",
        ))?;
    Ok(())
}

fn scalar_v18(ty: &Type) -> bool {
    matches!(ty, Type::Unit | Type::Scalar(_)) && *ty != Type::INDEX
}

// Separate formal-engine admission policy, not canonical-ledger credit.
// The unmetered definition/type indexes and scalar origin/operand walks are
// bounded by the squared complete census and ordered-map lookup depth. CFG
// dominance and guarded analysis retain their own existing metered policies.
// Names cover root/function selection; roots include every allowed replay.
fn work_v18(nodes: usize, names: usize, roots: usize) -> ResultV18<usize> {
    let log = usize::BITS as usize - nodes.max(1).leading_zeros() as usize + 1;
    nodes
        .checked_mul(nodes)
        .and_then(|n| n.checked_mul(log))
        .and_then(|n| n.checked_add(names.checked_mul(nodes)?))
        .and_then(|n| n.checked_mul(roots.checked_add(1)?))
        .filter(|&n| n <= MAX_WORK_V18)
        .ok_or(CanonicalScalarCfgFormalErrorV18::Unsupported(
            "formal module work bound",
        ))
}

/// A borrowed V18 owner with a complete memory-free, fixed-width scalar CFG
/// census. Branches, joins, switches and loops remain in the actual graph.
///
/// This runs the existing formal/effects engines, rather than constructing a
/// successful obligation list from the census. Unknown launch inputs still
/// produce incomplete analyses. It proves neither termination nor runtime
/// launch validity, source equivalence, native legality or final publication.
/// Memory, INDEX expressions, calls, and execution/source-role operations stay
/// unsupported, including in unreachable blocks or unused metadata.
pub struct CanonicalScalarCfgFormalScopeV18<'owner> {
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    effects: crate::InterproceduralEffectAnalysisV1,
    remaining: usize,
    identity_bytes: usize,
}

impl<'owner> CanonicalScalarCfgFormalScopeV18<'owner> {
    pub fn new(owner: &'owner VerifiedCanonicalKernelIrModuleV18) -> ResultV18<Self> {
        use CanonicalScalarCfgFormalErrorV18::Unsupported;
        let module = owner.module();
        if module.kernels.is_empty() || module.functions.len() != module.kernels.len() {
            return Err(Unsupported("non-root functions"));
        }
        if module.functions.len() > crate::MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1 {
            return Err(Unsupported("formal function bound"));
        }
        let mut nodes = 1;
        let mut names = 0;
        add_v18(&mut nodes, module.functions.len())?;
        add_v18(&mut nodes, module.kernels.len())?;
        add_v18(&mut nodes, module.storage_layouts.len())?;
        for kernel in &module.kernels {
            add_v18(&mut names, kernel.id.as_str().len())?;
            add_v18(&mut names, kernel.entry.as_str().len())?;
        }
        for layout in &module.storage_layouts {
            match &layout.kind {
                StorageLayoutKindV1::Scalar(ty) if *ty != ScalarType::Index => (),
                StorageLayoutKindV1::Record(fields) if fields.is_empty() && layout.size == 0 => (),
                _ => return Err(Unsupported("non-scalar storage metadata")),
            }
        }
        for function in &module.functions {
            add_v18(&mut names, function.id.as_str().len())?;
            if function.role != FunctionRole::KernelEntry {
                return Err(Unsupported("non-root function"));
            }
            add_v18(&mut nodes, function.signature.parameters.len())?;
            add_v18(&mut nodes, function.signature.results.len())?;
            if !function.signature.parameters.iter().all(scalar_v18)
                || !function.signature.results.is_empty()
            {
                return Err(Unsupported("non-scalar or returned signature"));
            }
            let body = function
                .body
                .as_ref()
                .ok_or(Unsupported("function declaration"))?;
            add_v18(&mut nodes, body.blocks.len())?;
            for block in &body.blocks {
                add_v18(&mut nodes, block.parameters.len())?;
                add_v18(&mut nodes, block.operations.len())?;
                if !block.parameters.iter().all(|value| scalar_v18(&value.ty)) {
                    return Err(Unsupported("non-scalar block parameter"));
                }
                for operation in &block.operations {
                    add_v18(&mut nodes, operation.results.len())?;
                    if !operation.results.iter().all(|value| scalar_v18(&value.ty)) {
                        return Err(Unsupported("non-scalar operation result"));
                    }
                    match operation.kind {
                        OperationKind::Constant(_)
                        | OperationKind::Unary { .. }
                        | OperationKind::Binary { .. }
                        | OperationKind::Compare { .. }
                        | OperationKind::Cast { .. }
                        | OperationKind::Select { .. } => (),
                        _ => return Err(Unsupported("memory or unsupported scalar operation")),
                    }
                    add_v18(&mut nodes, operation.kind.operand_count())?;
                }
                let terminator = block
                    .terminator
                    .as_ref()
                    .ok_or(Unsupported("missing terminator"))?;
                let edges = match terminator {
                    Terminator::Branch { .. } => Some(1),
                    Terminator::ConditionalBranch { .. } => Some(2),
                    Terminator::Switch { cases, .. } => cases.len().checked_add(1),
                    Terminator::IntegerSwitch { cases, .. } => cases.len().checked_add(1),
                    Terminator::Return { values } if values.is_empty() => Some(0),
                    Terminator::Unreachable => Some(0),
                    _ => return Err(Unsupported("returned values")),
                };
                add_v18(
                    &mut nodes,
                    edges.ok_or(Unsupported("formal edge arithmetic"))?,
                )?;
                terminator.try_visit_operands(|_| add_v18(&mut nodes, 1))?;
            }
        }
        work_v18(nodes, names, module.kernels.len())?;
        let mut roots = BTreeSet::new();
        for kernel in &module.kernels {
            if !roots.insert(&kernel.entry) {
                return Err(Unsupported("duplicate root entry"));
            }
        }
        if module
            .functions
            .iter()
            .any(|function| !roots.contains(&function.id))
        {
            return Err(Unsupported("uncovered function"));
        }
        drop(roots);
        let effects =
            crate::interprocedural_effects::analyze_interprocedural_effects_from_storage_v18(
                owner.verified_storage_module_ref_v1(),
            )
            .map_err(|error| {
                CanonicalScalarCfgFormalErrorV18::Formal(
                    FormalMemoryObligationError::InvalidModule(error),
                )
            })?;
        Ok(Self {
            owner,
            effects,
            remaining: module.kernels.len(),
            identity_bytes: names,
        })
    }

    pub fn owner(&self) -> &'owner VerifiedCanonicalKernelIrModuleV18 {
        self.owner
    }

    pub fn derive(
        &mut self,
        kernel: &KernelId,
        launch: ExplicitLaunchExtent,
        index_width: FormalIndexWidth,
    ) -> ResultV18<FormalMemoryObligationAnalysis> {
        self.remaining =
            self.remaining
                .checked_sub(1)
                .ok_or(CanonicalScalarCfgFormalErrorV18::Unsupported(
                    "formal root replay bound",
                ))?;
        if kernel.as_str().len() > self.identity_bytes {
            return Err(CanonicalScalarCfgFormalErrorV18::Unsupported(
                "formal query identity bound",
            ));
        }
        derive_kernel_memory_obligations_from_authenticated_module(
            self.owner.module(),
            kernel,
            launch,
            index_width,
            None,
            None,
            &self.effects,
        )
        .map_err(CanonicalScalarCfgFormalErrorV18::Formal)
    }
}

#[cfg(test)]
#[path = "scalar_cfg_v18_tests.rs"]
mod tests;
