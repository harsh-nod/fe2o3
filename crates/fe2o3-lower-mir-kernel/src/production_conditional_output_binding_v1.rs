/// Failure to bind a conditional coverage result to its exact source argument.
#[derive(Debug)]
pub enum ProductionConditionalOutputBindingErrorV1 {
    /// The facts borrow a different executable, even if its bytes are identical.
    ForeignSubject,
    /// No unique kernel-entry/source-function association exists.
    KernelAssociation,
    /// The source root, transparent wrapper, or launch association disagrees.
    SourceAssociation,
    /// The output is not one complete, directly represented source argument.
    OutputArgument,
    /// Existing argument correspondence or shared-resource checking failed.
    Correspondence(ProductionSemanticKirErrorV1),
}

impl fmt::Display for ProductionConditionalOutputBindingErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignSubject => {
                out.write_str("conditional output facts belong to another executable")
            }
            Self::KernelAssociation => {
                out.write_str("conditional output has no unique kernel-entry association")
            }
            Self::SourceAssociation => {
                out.write_str("conditional output source or launch association changed")
            }
            Self::OutputArgument => out.write_str(
                "conditional output is not one directly represented whole source argument",
            ),
            Self::Correspondence(error) => error.fmt(out),
        }
    }
}

impl std::error::Error for ProductionConditionalOutputBindingErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Correspondence(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ProductionSemanticKirErrorV1> for ProductionConditionalOutputBindingErrorV1 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        Self::Correspondence(error)
    }
}

impl From<ArgumentResourceV1> for ProductionConditionalOutputBindingErrorV1 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Correspondence(error.into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConditionalOutputArgumentV1 {
    source: u32,
    adjusted: u32,
    local: SemanticLocalIdV1,
    ty: SemanticTypeIdV1,
}

/// A conditional structural result tied to one retained source/executable owner.
///
/// Original source ordinals and physical KIR slots are distinct. Neither is
/// automatically an ordinal in a generated host packing plan. Transparent
/// Result wrappers must preserve the original source-argument order.
///
/// This fixed-size borrow is not a value proof, runtime precondition discharge,
/// optimized-graph receipt, artifact custody or launch permission. Consumers
/// must still join the exact reference effect and generated ABI, then use the
/// existing evidence gate and final-graph replay.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionConditionalOutputBindingV1, ProductionPreRankedKirOwnerV1};
/// fn substitute<'a>(binding: &mut ProductionConditionalOutputBindingV1<'a>, owner: &'a ProductionPreRankedKirOwnerV1) {
///     binding.owner = owner;
/// }
/// ```
pub struct ProductionConditionalOutputBindingV1<'owner> {
    owner: &'owner ProductionPreRankedKirOwnerV1,
    facts: fe2o3_kernel_ir::ConditionalTotalViewFactsV1<'owner>,
    association: &'owner SemanticKirFunctionCorrespondenceV1,
    launch: &'owner crate::ProductionSourceLaunchRootV1,
    argument: ConditionalOutputArgumentV1,
}

impl fmt::Debug for ProductionConditionalOutputBindingV1<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ProductionConditionalOutputBindingV1")
            .field("association", &self.association)
            .field("argument", &self.argument)
            .field("facts", &self.facts)
            .finish()
    }
}

impl<'owner> ProductionConditionalOutputBindingV1<'owner> {
    /// The exact retained source, SSA, correspondence and canonical executable.
    pub const fn owner(&self) -> &'owner ProductionPreRankedKirOwnerV1 {
        self.owner
    }
    /// The unchanged coverage result, including its address-arithmetic premise.
    pub const fn coverage(&self) -> &fe2o3_kernel_ir::ConditionalTotalViewFactsV1<'owner> {
        &self.facts
    }
    /// The exact root-qualified semantic body and physical entry association.
    pub const fn association(&self) -> &'owner SemanticKirFunctionCorrespondenceV1 {
        self.association
    }
    /// Retained source geometry; it does not discharge runtime launch values.
    pub const fn source_launch(&self) -> &'owner crate::ProductionSourceLaunchRootV1 {
        self.launch
    }
    /// Whole Rust source-argument ordinal, before ignored-argument erasure.
    pub const fn source_argument(&self) -> u32 {
        self.argument.source
    }
    /// The selected body's adjusted FnAbi ordinal, not a kernarg byte offset.
    pub const fn adjusted_argument(&self) -> u32 {
        self.argument.adjusted
    }
    /// Exact local supplying the complete physical output parameter.
    pub const fn semantic_local(&self) -> SemanticLocalIdV1 {
        self.argument.local
    }
    /// Exact source argument type, including a nominal capability carrier.
    pub const fn semantic_type(&self) -> SemanticTypeIdV1 {
        self.argument.ty
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Joins already-derived facts through the existing checked argument view.
    ///
    /// The caller must reserve this owner's `retained_analysis_storage_v1()`
    /// before the call. All new traversal work and argument-view scratch use
    /// the same ledger. Ordinary errors restore the incoming scratch floor;
    /// the existing argument view does not claim allocator/RSS or unwind bounds.
    /// No second graph, copied argument map, wire record or authority is made.
    pub fn bind_conditional_output_v1<'owner>(
        &'owner self,
        facts: fe2o3_kernel_ir::ConditionalTotalViewFactsV1<'owner>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionConditionalOutputBindingV1<'owner>,
        ProductionConditionalOutputBindingErrorV1,
    > {
        use ProductionConditionalOutputBindingErrorV1 as Error;
        budget.charge_work(4)?;
        if !std::ptr::eq(facts.module(), self.executable.module()) {
            return Err(Error::ForeignSubject);
        }
        if budget.storage() < self.retained_analysis_storage_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let mut association = None;
        for row in self.correspondence.lowered_functions() {
            budget.charge_work(argument_sum_v1(&[
                4,
                row.kernel_ir_function().as_str().len(),
                facts.function().id.as_str().len(),
            ])?)?;
            if row.role() == SemanticKirFunctionRoleV1::KernelEntry
                && row.kernel_ir_function() == &facts.function().id
                && association.replace(row).is_some()
            {
                return Err(Error::KernelAssociation);
            }
        }
        let association = association.ok_or(Error::KernelAssociation)?;
        let mut launch = None;
        budget.charge_work(argument_product_v1(self.source_launch.roots().len(), 3)?)?;
        for row in self.source_launch.roots() {
            if row.selected_root() == association.correspondence_owner()
                && launch.replace(row).is_some()
            {
                return Err(Error::SourceAssociation);
            }
        }
        let launch = launch.ok_or(Error::SourceAssociation)?;
        if launch.source_rank() != 1
            || launch.layout().global_extents() != [dialect_kernel::DYNAMIC_EXTENT, 1, 1]
        {
            return Err(Error::SourceAssociation);
        }
        let semantic = self.semantic_ssa.source_semantic();
        charge_conditional_body_selection_v1(semantic, association, budget)?;
        let selection = semantic
            .select_kernel_body_for_root_v1(association.correspondence_owner())
            .ok_or(Error::SourceAssociation)?;
        if selection.body() != association.semantic_function()
            || semantic.functions()[selection.root().index() as usize].identity()
                != launch.semantic_root_identity()
        {
            return Err(Error::SourceAssociation);
        }
        let argument = self
            .with_checked_arguments_v1(selection.root(), selection.body(), budget, |view| {
                conditional_output_argument_v1(
                    view,
                    facts.output_parameter_index() as usize,
                    facts.output_value(),
                )
            })?
            .ok_or(Error::OutputArgument)?;
        Ok(ProductionConditionalOutputBindingV1 {
            owner: self,
            facts,
            association,
            launch,
            argument,
        })
    }
}

fn charge_conditional_body_selection_v1(
    semantic: &AdmittedInertSemanticMirV1,
    association: &SemanticKirFunctionCorrespondenceV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionConditionalOutputBindingErrorV1> {
    source_arguments_v1::charge_source_body_selection_v1(semantic, association, budget).map_err(
        |error| match error {
            source_arguments_v1::ProductionSourceArgumentErrorV1::CorrespondenceMismatch => {
                ProductionConditionalOutputBindingErrorV1::SourceAssociation
            }
            error => ProductionConditionalOutputBindingErrorV1::Correspondence(error.into()),
        },
    )
}

fn conditional_output_argument_v1(
    view: &mut ProductionArgumentViewV1<'_, '_>,
    slot: usize,
    value: ValueId,
) -> Result<Option<ConditionalOutputArgumentV1>, ProductionSemanticKirErrorV1> {
    view.data
        .whole_parameter_v1(view.budget, slot, value)
        .map(|argument| {
            argument.map(|argument| ConditionalOutputArgumentV1 {
                source: argument.source_argument(),
                adjusted: argument.adjusted_argument(),
                local: argument.semantic_local(),
                ty: argument.semantic_type(),
            })
        })
        .map_err(Into::into)
}
