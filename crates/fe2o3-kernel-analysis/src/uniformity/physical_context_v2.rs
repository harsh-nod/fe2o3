use super::*;

mod selector;
#[cfg(test)]
mod tests;

/// Conditional launch mathematics, borrowed from one exact module/kernel/entry.
///
/// This constructor does not authenticate a host descriptor, dispatch, target,
/// or Index-width claim. A production consumer must establish those contracts
/// before relying on the returned report. Logical Static extents do not bound
/// physical invocations. The report grants no memory or publication authority.
pub struct UniformityPhysicalLaunchV2<'a> {
    module: &'a Module,
    kernel: &'a fe2o3_kernel_ir::Kernel,
    function: &'a Function,
    workgroup: u32,
    max_grid: u32,
    global_max: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UniformityPhysicalLaunchErrorV2 {
    ForeignKernel,
    EntryClosure,
    Geometry,
    IndexWidth,
}

impl<'a> UniformityPhysicalLaunchV2<'a> {
    /// Checks exact borrowed membership and D1 coordinate arithmetic only.
    pub fn new(
        module: &'a Module,
        kernel: &'a fe2o3_kernel_ir::Kernel,
        max_grid: [u32; 3],
        index_width: fe2o3_kernel_ir::FormalIndexWidth,
    ) -> Result<Self, UniformityPhysicalLaunchErrorV2> {
        use UniformityPhysicalLaunchErrorV2 as Error;
        if !module
            .kernels
            .iter()
            .any(|candidate| std::ptr::eq(candidate, kernel))
        {
            return Err(Error::ForeignKernel);
        }
        let mut functions = module
            .functions
            .iter()
            .filter(|function| function.id == kernel.entry);
        let function = functions.next().ok_or(Error::EntryClosure)?;
        if functions.next().is_some()
            || module
                .kernels
                .iter()
                .filter(|candidate| candidate.entry == kernel.entry)
                .count()
                != 1
            || function.role != FunctionRole::KernelEntry
            || function.body.is_none()
        {
            return Err(Error::EntryClosure);
        }
        if index_width != fe2o3_kernel_ir::FormalIndexWidth::Bits64 {
            return Err(Error::IndexWidth);
        }
        let group = kernel.workgroup_size.ok_or(Error::Geometry)?;
        if !matches!(kernel.domain, fe2o3_kernel_ir::LaunchDomain::D1 { .. })
            || group.x == 0
            || group.y != 1
            || group.z != 1
            || max_grid[0] == 0
            || max_grid[1] != 1
            || max_grid[2] != 1
        {
            return Err(Error::Geometry);
        }
        let extent = u64::from(group.x)
            .checked_mul(u64::from(max_grid[0]))
            .ok_or(Error::Geometry)?;
        let global_max = extent.checked_sub(1).ok_or(Error::Geometry)?;
        Ok(Self {
            module,
            kernel,
            function,
            workgroup: group.x,
            max_grid: max_grid[0],
            global_max,
        })
    }

    pub fn module(&self) -> &'a Module {
        self.module
    }
    pub fn kernel(&self) -> &'a fe2o3_kernel_ir::Kernel {
        self.kernel
    }
    pub fn function(&self) -> &'a Function {
        self.function
    }

    /// Analyzes exactly the borrowed entry under the conditional input contract.
    pub fn analyze(&self) -> AnalysisReport {
        analyze_kernel_entry_with_physical_context_v2(self.module, self.function, Some(self))
    }

    fn invocation_range(&self, kind: IndexKind, axis: Axis) -> Option<UnsignedRange> {
        let (min, max) = match (kind, axis) {
            (IndexKind::Global, Axis::X) => (0, u128::from(self.global_max)),
            (IndexKind::Local, Axis::X) => (0, u128::from(self.workgroup - 1)),
            (IndexKind::Workgroup, Axis::X) => (0, u128::from(self.max_grid - 1)),
            (IndexKind::WorkgroupSize, Axis::X) => {
                (u128::from(self.workgroup), u128::from(self.workgroup))
            }
            (IndexKind::WorkgroupCount, Axis::X) => (1, u128::from(self.max_grid)),
            (IndexKind::Global | IndexKind::Local | IndexKind::Workgroup, Axis::Y | Axis::Z) => {
                (0, 0)
            }
            (IndexKind::WorkgroupSize | IndexKind::WorkgroupCount, Axis::Y | Axis::Z) => (1, 1),
        };
        Some(UnsignedRange { min, max })
    }
}

pub(super) fn refine_successors(
    context: &UniformityPhysicalLaunchV2<'_>,
    body: &FunctionBody,
    incoming: &BTreeMap<BlockId, Vec<Edge>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    types: &BTreeMap<ValueId, Type>,
    definitions: &BTreeMap<ValueId, &Operation>,
    effective: &mut BTreeMap<BlockId, BTreeSet<BlockId>>,
) {
    if !context
        .function
        .body
        .as_ref()
        .is_some_and(|actual| std::ptr::eq(actual, body))
    {
        return;
    }
    let _ = selector::refine(
        context,
        body,
        incoming,
        dominators,
        types,
        definitions,
        effective,
        MAX_RELATIONAL_PROOF_WORK,
        MAX_RELATIONAL_PROOF_WORK,
    );
}
