//! Private source-owned nominal transport and helper tensor correspondence.
//! No receipt constructor, dispatcher, target lowering or scalar MFMA expansion.
use super::native_helper_value_template_v1::{Ledger, Meter};
use super::*;

type Error = &'static str;

/// Actual graph rows borrowed only through the intact materialized owner.
pub(super) struct Rows<'a> {
    owner: &'a ProductionPreRankedKirOwnerV1,
    emission: Bf16CallInstanceEmissionViewV1<'a>,
    root: &'a Function,
    helper: &'a Function,
    call: &'a Operation,
    matrix: &'a Operation,
}
fn operation(function: &Function, site: (BlockId, u32)) -> Result<&Operation, Error> {
    function
        .body
        .as_ref()
        .and_then(|body| body.blocks.iter().find(|block| block.id == site.0))
        .and_then(|block| block.operations.get(site.1 as usize))
        .ok_or("nominal translation exact operation absent")
}
/// Authenticate the only permitted declaration without allocating a comparison
/// Function, String, capability set, or alternative graph.
pub(super) fn exact_trap_declaration(
    function: &Function,
    meter: &mut dyn Meter,
) -> Result<bool, Error> {
    meter.work(7)?;
    if function.role != fe2o3_kernel_ir::FunctionRole::ExternalImport
        || function.body.is_some()
        || !function.signature.parameters.is_empty()
        || !function.signature.results.is_empty()
        || function.required_capabilities.len() != 1
    {
        return Ok(false);
    }
    let Some(fe2o3_kernel_ir::TargetCapability::Extension { namespace, name }) =
        function.required_capabilities.iter().next()
    else {
        return Ok(false);
    };
    // from_intrinsic_call searches the closed eight-row diagnostic catalog.
    // This zero-argument lookup cannot construct the allocating Print variant.
    let work = function
        .id
        .as_str()
        .len()
        .checked_add(2)
        .and_then(|n| n.checked_mul(8))
        .and_then(|n| n.checked_add(namespace.len()))
        .and_then(|n| n.checked_add(fe2o3_kernel_ir::AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len()))
        .and_then(|n| n.checked_add(name.len()))
        .and_then(|n| n.checked_add(fe2o3_kernel_ir::AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len()))
        .and_then(|n| n.checked_add(2))
        .ok_or("nominal Trap declaration work overflow")?;
    meter.work(work)?;
    Ok(matches!(
        AmdGpuDiagnosticOperation::from_intrinsic_call(&function.id, &[]),
        Some(AmdGpuDiagnosticOperation::Trap)
    ) && namespace == fe2o3_kernel_ir::AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
        && name == fe2o3_kernel_ir::AMDGPU_DIAGNOSTICS_CAPABILITY_NAME)
}

/// Exactly the two selected executable bodies, plus at most the exact Trap
/// declaration emitted by the same source engine. No other import or body.
pub(super) fn closed_function_roster(
    module: &Module,
    root: &Function,
    helper: &Function,
    meter: &mut dyn Meter,
) -> Result<bool, Error> {
    meter.work(
        module
            .functions
            .len()
            .checked_add(1)
            .ok_or("nominal function roster work overflow")?,
    )?;
    if !(2..=3).contains(&module.functions.len()) || std::ptr::eq(root, helper) {
        return Err("nominal translation finite executable roster differs");
    }
    let (mut roots, mut helpers, mut declaration) = (0usize, 0usize, false);
    for function in &module.functions {
        if std::ptr::eq(function, root) {
            if function.body.is_none() {
                return Err("nominal translation root body absent");
            }
            roots += 1;
        } else if std::ptr::eq(function, helper) {
            if function.body.is_none() {
                return Err("nominal translation helper body absent");
            }
            helpers += 1;
        } else if !declaration && exact_trap_declaration(function, meter)? {
            declaration = true;
        } else {
            return Err("nominal translation additional declaration or body");
        }
    }
    if roots != 1 || helpers != 1 {
        return Err("nominal translation selected bodies not in actual roster");
    }
    Ok(declaration)
}

pub(super) fn trap_presence(declaration: bool, calls: usize) -> Result<(), Error> {
    if declaration == (calls != 0) {
        Ok(())
    } else {
        Err("nominal translation Trap declaration/call presence differs")
    }
}

/// The header-counting pass is itself paid before traversal. Its length
/// arithmetic only reads already selected Vec headers; operation contents are
/// traversed later, after the separately derived lookup/census debit succeeds.
pub(super) fn prepay_graph_visits(module: &Module, meter: &mut dyn Meter) -> Result<(), Error> {
    meter.work(module.functions.len())?;
    let mut visits = 0usize;
    for function in &module.functions {
        // The complete roster is authenticated separately before this counting
        // pass. An exact external declaration has a header but no body visits.
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        meter.work(body.blocks.len())?;
        for block in &body.blocks {
            visits = visits
                .checked_add(1)
                .and_then(|n| n.checked_add(block.operations.len()))
                .ok_or("nominal translation graph work overflow")?;
        }
    }
    meter.work(
        visits
            .checked_mul(4)
            .ok_or("nominal translation graph work overflow")?,
    )
}

impl<'a> Rows<'a> {
    pub(super) fn resolve(
        owner: &'a ProductionPreRankedKirOwnerV1,
        kernel: &str,
        meter: &mut dyn Meter,
    ) -> Result<Self, Error> {
        meter.work(8)?;
        if owner.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::Bf16Nominal {
            return Err("nominal translation requires intact nominal owner");
        }
        let emission = owner
            .bf16_call_instance_emission_v1()
            .ok_or("nominal translation sealed emission absent")?;
        let module = owner.executable().module();
        if !std::ptr::eq(emission.owner(), owner)
            || module.kernels.len() != 1
            || !(2..=3).contains(&module.functions.len())
        {
            return Err("nominal translation owner or finite roster differs");
        }
        let names = module
            .functions
            .iter()
            .try_fold(kernel.len(), |total, function| {
                total.checked_add(function.id.as_str().len())
            })
            .and_then(|n| n.checked_add(module.kernels[0].id.as_str().len()))
            .ok_or("nominal translation name work overflow")?;
        meter.work(
            names
                .checked_mul(8)
                .ok_or("nominal translation name work overflow")?,
        )?;
        if module.kernels[0].id.as_str() != kernel
            || &module.kernels[0].entry != emission.root_function()
        {
            return Err("nominal translation selected root differs");
        }
        let root = module
            .functions
            .iter()
            .find(|f| &f.id == emission.root_function())
            .ok_or("nominal translation root absent")?;
        let helper = module
            .functions
            .iter()
            .find(|f| &f.id == emission.helper_function())
            .ok_or("nominal translation helper absent")?;
        let trap_declaration = closed_function_roster(module, root, helper, meter)?;
        // Prepay every later block/operation lookup and census visit. The actual
        // immutable canonical owner already guarantees unique IDs and ordinals.
        prepay_graph_visits(module, meter)?;
        let call = operation(root, emission.call_site())?;
        let matrix = operation(helper, emission.matrix_site())?;
        let mut calls = 0usize;
        let mut matrices = 0usize;
        let mut traps = 0usize;
        meter.work(module.functions.len())?;
        for function in &module.functions {
            // closed_function_roster authenticated the only body-less row.
            let Some(body) = function.body.as_ref() else {
                continue;
            };
            for block in &body.blocks {
                for candidate in &block.operations {
                    match &candidate.kind {
                        OperationKind::Call { callee, arguments } => {
                            meter.work(
                                callee
                                    .as_str()
                                    .len()
                                    .checked_add(2)
                                    .and_then(|n| n.checked_mul(8))
                                    .and_then(|n| n.checked_add(arguments.len()))
                                    .and_then(|n| n.checked_add(1))
                                    .ok_or("nominal call scan work overflow")?,
                            )?;
                            if std::ptr::eq(candidate, call) && std::ptr::eq(function, root) {
                                calls += 1;
                            } else if std::ptr::eq(function, root)
                                && candidate.results.is_empty()
                                && arguments.is_empty()
                                && matches!(
                                    AmdGpuDiagnosticOperation::from_intrinsic_call(
                                        callee, arguments
                                    ),
                                    Some(AmdGpuDiagnosticOperation::Trap)
                                )
                            {
                                traps = traps
                                    .checked_add(1)
                                    .ok_or("nominal translation Trap count overflow")?;
                            } else {
                                return Err("nominal translation additional non-Trap call");
                            }
                        }
                        OperationKind::Matrix(_) => {
                            if !std::ptr::eq(candidate, matrix) || !std::ptr::eq(function, helper) {
                                return Err("nominal translation additional Matrix");
                            }
                            matrices += 1;
                        }
                        _ => {}
                    }
                }
            }
        }
        meter.work(1)?;
        trap_presence(trap_declaration, traps)?;
        if calls != 1 || matrices != 1 {
            return Err("nominal translation exact Call/Matrix roster differs");
        }
        Ok(Self {
            owner,
            emission,
            root,
            helper,
            call,
            matrix,
        })
    }

    pub(super) fn check_components(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        bf16_query_components_v1(
            self.emission.relation,
            self.root,
            self.helper,
            self.call,
            self.matrix,
            budget,
        )
        .map_err(component_error)
    }
    pub(super) fn bind(self, meter: &mut dyn Meter) -> Result<Context<'a>, Error> {
        Ok(Context {
            rows: self,
            ledger: meter.identity()?,
            floor: meter.storage()?,
        })
    }
}

pub(super) fn component_error(error: Bf16NominalCallQueryErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        Bf16NominalCallQueryErrorV1::Resource(error) => error.into(),
        Bf16NominalCallQueryErrorV1::Unavailable(_)
        | Bf16NominalCallQueryErrorV1::CallbackPanicked => {
            ProductionSemanticKirErrorV1::CorrespondenceMismatch
        }
    }
}

/// No public constructor or detached source/budget identity.
pub(super) struct Context<'a> {
    rows: Rows<'a>,
    ledger: Ledger,
    floor: usize,
}
pub(super) fn tensor_matches(
    matrix: &MatrixOperation,
    contract: TensorLayoutContractV1,
    convergence: dialect_kernel::TensorConvergenceAttr,
    active_lanes: u32,
) -> bool {
    let ranked_scope = match convergence {
        dialect_kernel::TensorConvergenceAttr::UniformSubgroup => 1,
        dialect_kernel::TensorConvergenceAttr::UniformWorkgroup => 2,
        dialect_kernel::TensorConvergenceAttr::Divergent
        | dialect_kernel::TensorConvergenceAttr::Opaque => return false,
    };
    matrix.tensor_layout == Some(contract)
        && matrix.active_lanes == active_lanes
        && normalize_kir_scope_v1(matrix.convergence.scope()) == ranked_scope
}
pub(super) fn tensor_row(
    count: &mut usize,
    matrix: &MatrixOperation,
    contract: TensorLayoutContractV1,
    convergence: dialect_kernel::TensorConvergenceAttr,
    active_lanes: u32,
) -> Result<(), Error> {
    if *count != 0 || !tensor_matches(matrix, contract, convergence, active_lanes) {
        return Err("nominal translation exact singleton tensor differs");
    }
    *count = 1;
    Ok(())
}
pub(super) fn tensor_count(count: usize) -> Result<usize, Error> {
    if count == 1 {
        Ok(count)
    } else {
        Err("nominal translation tensor roster differs")
    }
}
impl Context<'_> {
    pub(super) fn tensors(
        &self,
        function: &Function,
        recipe: &fe2o3_pliron::ProductionRankedKernelV1,
        meter: &mut dyn Meter,
    ) -> Result<usize, Error> {
        meter.work(4)?;
        if meter.identity()? != self.ledger
            || meter.storage()? < self.floor
            || !std::ptr::eq(function, self.rows.root)
            || !std::ptr::eq(self.rows.emission.owner(), self.rows.owner)
        {
            return Err("nominal translation context or live account substituted");
        }
        let OperationKind::Matrix(matrix) = &self.rows.matrix.kind else {
            return Err("nominal translation Matrix changed");
        };
        let mut count = 0usize;
        for block in recipe.blocks() {
            meter.work(1)?;
            for operation in block.operations() {
                meter.work(1)?;
                if let ProductionRankedOperationV1::TensorLayout {
                    contract,
                    convergence,
                    active_lanes,
                    ..
                } = operation
                {
                    tensor_row(&mut count, matrix, *contract, *convergence, *active_lanes)?;
                }
            }
        }
        tensor_count(count)
    }
}
