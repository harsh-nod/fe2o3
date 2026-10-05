use super::*;
use crate::{StorageLayoutKindV1, Terminator, VerifiedCanonicalKernelIrModuleV18};

/// Closed-subset refusal, distinct from a failed formal extraction.
#[derive(Debug)]
pub enum CanonicalClosedScalarFormalErrorV18 {
    Unsupported(&'static str),
    Formal(FormalMemoryObligationError),
}
impl fmt::Display for CanonicalClosedScalarFormalErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical closed scalar formal V18: {self:?}")
    }
}
impl Error for CanonicalClosedScalarFormalErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Formal(error) => Some(error),
            Self::Unsupported(_) => None,
        }
    }
}

/// Extracts fresh obligations from the actual immutable V18 owner, without a
/// legacy token, reencoding, table removal or caller-provided completion.
///
/// This intentionally admits only scalar/unit metadata and kernel-only,
/// single-block constant/return graphs. Memory, calls and control stay refused.
/// Launch arguments remain descriptive, not runtime authority. The existing
/// formal/effects engines retain their own bounded allocation/work policy.
pub fn derive_canonical_closed_scalar_memory_obligations_v18(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    kernel_id: &KernelId,
    launch_extent: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
) -> Result<FormalMemoryObligationAnalysis, CanonicalClosedScalarFormalErrorV18> {
    CanonicalClosedScalarFormalScopeV18::new(owner)?.derive(kernel_id, launch_extent, index_width)
}

/// Separate formal-engine work policy, not canonical ledger credit. The
/// preflight caps structural/name input before indexing and bounds all allowed
/// root replays, including the common engine's linear root/function selection.
const MAX_SCALAR_FORMAL_NODES_V18: usize = 65_536;
const MAX_SCALAR_FORMAL_WORK_V18: usize = 1_048_576;

fn checked_work_v18(
    nodes: usize,
    roots: usize,
) -> Result<usize, CanonicalClosedScalarFormalErrorV18> {
    let logarithm = usize::BITS as usize - nodes.max(1).leading_zeros() as usize + 1;
    nodes
        .checked_mul(roots.checked_add(1).ok_or(
            CanonicalClosedScalarFormalErrorV18::Unsupported("formal work arithmetic"),
        )?)
        .and_then(|work| work.checked_mul(logarithm))
        .filter(|&work| work <= MAX_SCALAR_FORMAL_WORK_V18)
        .ok_or(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "formal module work bound",
        ))
}

fn add_nodes_v18(
    nodes: &mut usize,
    count: usize,
) -> Result<(), CanonicalClosedScalarFormalErrorV18> {
    *nodes = nodes
        .checked_add(count)
        .filter(|&n| n <= MAX_SCALAR_FORMAL_NODES_V18)
        .ok_or(CanonicalClosedScalarFormalErrorV18::Unsupported(
            "formal module node bound",
        ))?;
    Ok(())
}

/// Actual-owner borrow with one validated closed-module census and one effects
/// extraction. No V1 token, generic validated flag or copied Module is stored.
/// The fixed work bound covers indexing plus at most one root-count of common
/// engine replays; each attempted derive consumes a slot before engine work.
pub struct CanonicalClosedScalarFormalScopeV18<'owner> {
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    effects: crate::InterproceduralEffectAnalysisV1,
    remaining: usize,
    nodes: usize,
}

impl<'owner> CanonicalClosedScalarFormalScopeV18<'owner> {
    pub fn new(
        owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    ) -> Result<Self, CanonicalClosedScalarFormalErrorV18> {
        use CanonicalClosedScalarFormalErrorV18 as E;
        let module = owner.module();
        if module.functions.len() != module.kernels.len() || module.kernels.is_empty() {
            return Err(E::Unsupported("non-root functions"));
        }
        if module.functions.len() > crate::MAX_INTERPROCEDURAL_EFFECT_FUNCTIONS_V1 {
            return Err(E::Unsupported("formal function bound"));
        }
        let mut nodes = 1usize;
        add_nodes_v18(&mut nodes, module.storage_layouts.len())?;
        add_nodes_v18(&mut nodes, module.functions.len())?;
        add_nodes_v18(&mut nodes, module.kernels.len())?;
        // Count lengths before any index allocation or name comparison. This
        // bounded linear census cannot recursively inspect arbitrary types.
        for root in &module.kernels {
            add_nodes_v18(&mut nodes, root.id.as_str().len())?;
            add_nodes_v18(&mut nodes, root.entry.as_str().len())?;
        }
        for function in &module.functions {
            add_nodes_v18(&mut nodes, function.id.as_str().len())?;
            add_nodes_v18(&mut nodes, function.signature.parameters.len())?;
            add_nodes_v18(&mut nodes, function.signature.results.len())?;
            let Some(body) = &function.body else {
                return Err(E::Unsupported("function declaration"));
            };
            if body.blocks.len() != 1 {
                return Err(E::Unsupported("control flow"));
            }
            add_nodes_v18(&mut nodes, body.blocks[0].parameters.len())?;
            add_nodes_v18(&mut nodes, body.blocks[0].operations.len())?;
            for operation in &body.blocks[0].operations {
                add_nodes_v18(&mut nodes, operation.results.len())?;
            }
        }
        checked_work_v18(nodes, module.kernels.len())?;
        let mut entries = BTreeSet::new();
        for root in &module.kernels {
            if !entries.insert(&root.entry) {
                return Err(E::Unsupported("duplicate root entry"));
            }
        }
        for row in &module.storage_layouts {
            if !matches!(&row.kind, StorageLayoutKindV1::Scalar(_))
                && !matches!(&row.kind, StorageLayoutKindV1::Record(fields) if fields.is_empty() && row.size == 0)
            {
                return Err(E::Unsupported("non-scalar storage metadata"));
            }
        }
        for function in &module.functions {
            if !entries.contains(&function.id) {
                return Err(E::Unsupported("non-root function"));
            }
            if function
                .signature
                .parameters
                .iter()
                .chain(&function.signature.results)
                .any(|ty| !matches!(ty, Type::Unit | Type::Scalar(_)))
            {
                return Err(E::Unsupported("non-scalar signature"));
            }
            let Some(body) = &function.body else {
                return Err(E::Unsupported("function declaration"));
            };
            if body.blocks.len() != 1 {
                return Err(E::Unsupported("control flow"));
            }
            let block = &body.blocks[0];
            if !block.parameters.is_empty()
                || !matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty())
            {
                return Err(E::Unsupported("control or returned values"));
            }
            for operation in &block.operations {
                if !matches!(&operation.kind, OperationKind::Constant(_))
                    || operation
                        .results
                        .iter()
                        .any(|result| !matches!(&result.ty, Type::Unit | Type::Scalar(_)))
                {
                    return Err(E::Unsupported("non-constant scalar operation"));
                }
            }
        }
        drop(entries);
        let effects =
            crate::interprocedural_effects::analyze_interprocedural_effects_from_storage_v18(
                owner.verified_storage_module_ref_v1(),
            )
            .map_err(|error| E::Formal(FormalMemoryObligationError::InvalidModule(error)))?;
        Ok(Self {
            owner,
            effects,
            remaining: module.kernels.len(),
            nodes,
        })
    }

    pub fn derive(
        &mut self,
        kernel_id: &KernelId,
        launch_extent: ExplicitLaunchExtent,
        index_width: FormalIndexWidth,
    ) -> Result<FormalMemoryObligationAnalysis, CanonicalClosedScalarFormalErrorV18> {
        self.remaining = self.remaining.checked_sub(1).ok_or(
            CanonicalClosedScalarFormalErrorV18::Unsupported("formal root replay bound"),
        )?;
        // The engine clones a missing requested identity into its diagnostic.
        // Bound that caller input as well as every owner-side comparison.
        if kernel_id.as_str().len() > self.nodes {
            return Err(CanonicalClosedScalarFormalErrorV18::Unsupported(
                "formal query identity bound",
            ));
        }
        derive_kernel_memory_obligations_from_authenticated_module(
            self.owner.module(),
            kernel_id,
            launch_extent,
            index_width,
            None,
            None,
            &self.effects,
        )
        .map_err(CanonicalClosedScalarFormalErrorV18::Formal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("closed_scalar_v18_tests.rs");
}
