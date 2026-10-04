use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, VerifiedKernelIrModuleV1};
use std::marker::PhantomData;

/// Complete entry correspondence borrowing the original owners, not view scratch.
/// Inert traces cannot create this relation without independent replay.
/// This checks the complete ABI traversal, not executable-body equivalence.
/// Consumers must retain their existing whole-source translation replay.
/// The work lifetime keeps the original meter borrowed while its identity is
/// retained; it does not hold the resource-budget view exclusively.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionSourceArgumentRelationV1;
/// fn forge(relation: &ProductionSourceArgumentRelationV1<'_, '_>) {
///     let _ = ProductionSourceArgumentRelationV1 { source: relation.source };
/// }
/// ```
pub struct ProductionSourceArgumentRelationV1<'source, 'work> {
    source: &'source ProductionSemanticSsaOwnerV1,
    module: &'source Module,
    function: &'source Function,
    association: &'source SemanticKirFunctionCorrespondenceV1,
    trace: ArgumentTraceV1<'source>,
    ledger: ArgumentLedgerV1,
    live_work: PhantomData<&'work CanonicalKernelIrWorkBudgetV1>,
}

/// Whole source argument checked against one actual canonical parameter.
/// The nominal binding retains exact owner identities; getters are descriptive.
///
/// ```compile_fail
/// use fe2o3_pliron::ProductionSourceArgumentBindingV1;
/// fn forge(binding: &ProductionSourceArgumentBindingV1<'_, '_>) {
///     let _ = ProductionSourceArgumentBindingV1 { source: binding.source };
/// }
/// ```
pub struct ProductionSourceArgumentBindingV1<'source, 'work> {
    source: &'source ProductionSemanticSsaOwnerV1,
    module: &'source Module,
    function: &'source Function,
    association: &'source SemanticKirFunctionCorrespondenceV1,
    parameter: u32,
    value: ValueId,
    argument: ProductionWholeSourceArgumentV1,
    ledger: ArgumentLedgerV1,
    live_work: PhantomData<&'work CanonicalKernelIrWorkBudgetV1>,
}

impl ProductionSemanticSsaOwnerV1 {
    /// Independently checks the complete proposed argument trace on this ledger.
    /// Borrowed owners remain in the caller's allocation domain. This retains
    /// no scratch allocation and does not reconstruct an executable graph.
    pub fn check_source_arguments_v1<'source, 'work>(
        &'source self,
        canonical: VerifiedKernelIrModuleV1<'source>,
        association: &'source SemanticKirFunctionCorrespondenceV1,
        trace: ArgumentTraceV1<'source>,
        budget: &mut ArgumentBudgetV1<'work>,
    ) -> Result<ProductionSourceArgumentRelationV1<'source, 'work>, ProductionSourceArgumentErrorV1>
    {
        budget.charge_work(4)?;
        if association.role != SemanticKirFunctionRoleV1::KernelEntry {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        let module = canonical.module();
        budget.charge_work(argument_product_v1(
            module.functions.len(),
            argument_sum_v1(&[association.kernel_ir_function.as_str().len(), 1])?,
        )?)?;
        let function = module
            .function(&association.kernel_ir_function)
            .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(argument_product_v1(
            module.kernels.len(),
            argument_sum_v1(&[function.id.as_str().len(), 1])?,
        )?)?;
        if !module
            .kernels
            .iter()
            .any(|kernel| kernel.entry == function.id)
        {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        let relation = ProductionSourceArgumentRelationV1 {
            source: self,
            module,
            function,
            association,
            trace,
            ledger: budget.work_ledger_identity_v1(),
            live_work: PhantomData,
        };
        relation.replay_v1(module, function, budget)?;
        Ok(relation)
    }
}

impl<'source, 'work> ProductionSourceArgumentRelationV1<'source, 'work> {
    pub const fn source_owner(&self) -> &'source ProductionSemanticSsaOwnerV1 {
        self.source
    }
    pub const fn canonical_module(&self) -> &'source Module {
        self.module
    }
    pub const fn canonical_function(&self) -> &'source Function {
        self.function
    }
    pub const fn association(&self) -> &'source SemanticKirFunctionCorrespondenceV1 {
        self.association
    }
    pub const fn source_semantic_identity(&self) -> &'source [u8; 32] {
        self.source.source_semantic_sha256()
    }

    /// Exact pointer equality, not a hash-equivalence substitute for custody.
    pub fn require_source_owner_v1(
        &self,
        source: &ProductionSemanticSsaOwnerV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        budget.charge_work(2)?;
        if budget.work_ledger_identity_v1() != self.ledger {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if !std::ptr::eq(source, self.source) {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }

    pub fn replay_v1(
        &self,
        module: &Module,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        self.replay_with_v1(module, function, budget, |_, _| Ok(()))
    }

    pub fn bind_whole_parameter_v1(
        &self,
        parameter: u32,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ProductionSourceArgumentBindingV1<'source, 'work>, ProductionSourceArgumentErrorV1>
    {
        let slot = usize::try_from(parameter).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let argument =
            self.replay_with_v1(self.module, self.function, budget, |mut view, budget| {
                view.whole_parameter_v1(budget, slot, value)?
                    .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)
            })?;
        Ok(ProductionSourceArgumentBindingV1 {
            source: self.source,
            module: self.module,
            function: self.function,
            association: self.association,
            parameter,
            value,
            argument,
            ledger: self.ledger,
            live_work: PhantomData,
        })
    }

    pub fn require_binding_v1(
        &self,
        binding: &ProductionSourceArgumentBindingV1<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        budget.charge_work(6)?;
        if self.ledger != binding.ledger || budget.work_ledger_identity_v1() != self.ledger {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if !std::ptr::eq(self.source, binding.source)
            || !std::ptr::eq(self.module, binding.module)
            || !std::ptr::eq(self.function, binding.function)
            || !std::ptr::eq(self.association, binding.association)
        {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        let replay = self.bind_whole_parameter_v1(binding.parameter, binding.value, budget)?;
        if replay.argument != binding.argument {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }

    fn replay_with_v1<'w, R>(
        &self,
        module: &Module,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'w>,
        use_view: impl for<'s> FnOnce(
            ProductionArgumentViewV1<'s>,
            &'s mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSourceArgumentErrorV1>,
    ) -> Result<R, ProductionSourceArgumentErrorV1> {
        budget.charge_work(5)?;
        if budget.work_ledger_identity_v1() != self.ledger {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if !std::ptr::eq(module, self.module) || !std::ptr::eq(function, self.function) {
            return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
        }
        // Resolve the actual root/body selection, including transparent wrappers.
        check_source_root_v1(self.source.source_semantic(), self.association, budget)?;
        with_parameter_correspondence_v1(
            self.source.source_semantic(),
            self.association,
            self.function,
            self.trace,
            budget,
            use_view,
        )
    }
}

impl ProductionSourceArgumentBindingV1<'_, '_> {
    pub const fn canonical_parameter(&self) -> u32 {
        self.parameter
    }
    pub const fn canonical_value(&self) -> ValueId {
        self.value
    }
    pub const fn source_argument(&self) -> u32 {
        self.argument.source_argument()
    }
    pub const fn adjusted_argument(&self) -> u32 {
        self.argument.adjusted_argument()
    }
    pub const fn semantic_local(&self) -> SemanticLocalIdV1 {
        self.argument.semantic_local()
    }
    pub const fn semantic_type(&self) -> SemanticTypeIdV1 {
        self.argument.semantic_type()
    }
}

fn check_source_root_v1(
    semantic: &AdmittedInertSemanticMirV1,
    association: &SemanticKirFunctionCorrespondenceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSourceArgumentErrorV1> {
    charge_source_body_selection_v1(semantic, association, budget)?;
    let selected = semantic
        .select_kernel_body_for_root_v1(association.correspondence_owner)
        .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
    if selected.body() != association.semantic_function {
        return Err(ProductionSourceArgumentErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}

/// Prepay the existing source root/body selector; this is not a checked relation.
pub fn charge_source_body_selection_v1(
    semantic: &AdmittedInertSemanticMirV1,
    association: &SemanticKirFunctionCorrespondenceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSourceArgumentErrorV1> {
    budget.charge_work(argument_sum_v1(&[32, semantic.roots().len()])?)?;
    let root = semantic
        .functions()
        .get(association.correspondence_owner.index() as usize)
        .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
    let body = semantic
        .functions()
        .get(association.semantic_function.index() as usize)
        .ok_or(ProductionSourceArgumentErrorV1::CorrespondenceMismatch)?;
    budget.charge_work(argument_product_v1(
        argument_sum_v1(&[
            root.abi().source_input_types().len(),
            body.abi().source_input_types().len(),
            root.abi().source_argument_ownership().len(),
            body.abi().source_argument_ownership().len(),
            root.blocks().len(),
        ])?,
        8,
    )?)?;
    for block in root.blocks() {
        budget.charge_work(argument_product_v1(block.statements().len(), 4)?)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            budget.charge_work(argument_product_v1(call.arguments().len(), 8)?)?;
        }
    }
    Ok(())
}
