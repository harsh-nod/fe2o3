// Additive exact-V16 pre-ranked custody. Existing V12 materialized/Policy3 and
// Policy4 consumers are intentionally unchanged and cannot borrow this as V12.

#[cfg(test)]
#[path = "production_ordered_region_pre_ranked_v16_tests.rs"]
mod ordered_region_pre_ranked_v16_tests;

/// Failure of closed-region context, ordinary lowering, or exact V16 custody.
#[derive(Debug)]
pub enum ProductionOrderedRegionPreRankedErrorV16 {
    /// The source context or normal semantic lowering was not admitted.
    Lowering(ProductionSemanticKirErrorV1),
    /// Allocation-budgeted exact canonical verification did not succeed.
    Canonical(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV16),
}

impl fmt::Display for ProductionOrderedRegionPreRankedErrorV16 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lowering(error) => error.fmt(formatter),
            Self::Canonical(error) => error.fmt(formatter),
        }
    }
}
impl Error for ProductionOrderedRegionPreRankedErrorV16 {}
impl From<ProductionSemanticKirErrorV1> for ProductionOrderedRegionPreRankedErrorV16 {
    fn from(error: ProductionSemanticKirErrorV1) -> Self {
        Self::Lowering(error)
    }
}
impl From<ArgumentResourceV1> for ProductionOrderedRegionPreRankedErrorV16 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Lowering(error.into())
    }
}

/// One normal semantic-to-KIR materialization for the closed V31 source profile.
///
/// The sole executable Module is freshly decoded and verified under V16. Source
/// SSA plans, launch agreement and correspondence retain their existing meaning;
/// these are not a second executable graph. This pre-ranked owner supplies no
/// ranked/functional proof, optimizer, protected artifact or launch authority.
/// Exact source/target authentication remains the frontend's responsibility.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOrderedRegionPreRankedKirOwnerV16;
/// fn cloned<T: Clone>() {}
/// cloned::<ProductionOrderedRegionPreRankedKirOwnerV16>();
/// ```
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOrderedRegionPreRankedKirOwnerV16;
/// fn mutate(owner: &mut ProductionOrderedRegionPreRankedKirOwnerV16) {
///     owner.executable().module().functions.clear();
/// }
/// ```
#[derive(Debug)]
#[must_use = "dropping this owner abandons source/executable correspondence"]
pub struct ProductionOrderedRegionPreRankedKirOwnerV16 {
    semantic_ssa: ProductionSemanticSsaOwnerV1,
    source_launch: crate::ProductionSourceLaunchRosterV1,
    executable: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV16,
    executable_storage: fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV16,
    call_correspondence_storage: usize,
    correspondence: SemanticKirCorrespondenceV1,
}

impl ProductionOrderedRegionPreRankedKirOwnerV16 {
    /// Reuses the normal lowering engine and its source/operation bounds, then
    /// the full allocation-budgeted V16 inverse. The cumulative ledger covers
    /// the added context scan, launch/call transport and canonical verification;
    /// existing source-SSA replay and lowering use their separate limits, not
    /// this ledger for all of their allocations. This is not an RSS bound.
    /// All Result exits restore the caller's logical storage floor. Reserve the
    /// executable_storage and call_correspondence_storage receipts while
    /// retaining the returned owner. Other source/correspondence allocations
    /// remain bounded by the existing lowering limits, not these receipts.
    pub fn try_materialize_with_budget(
        semantic_ssa: ProductionSemanticSsaOwnerV1,
        source_launch: crate::ProductionSourceLaunchRosterV1,
        limits: ProductionSemanticKirLimitsV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionOrderedRegionPreRankedErrorV16> {
        let floor = budget.storage();
        let result = (|| {
            validate_ordered_region_context_v31(
                semantic_ssa.source_semantic(),
                &source_launch,
                limits,
                budget,
            )?;
            // Context admission established a singleton roster. The existing
            // helper collects an exact-size iterator, then compacts to Box.
            // Reserve logical Vec/Box overlap before calling it, following the
            // existing call-transport convention (allocator excess is excluded).
            let root_bytes = std::mem::size_of::<RetainedRankedLaunchRootV1>();
            budget.charge_work(1)?;
            budget.reserve_storage(argument_product_v1(root_bytes, 2)?)?;
            let roots = materialization_launch_roots_v1(&semantic_ssa, &source_launch)?;
            if roots.len() != 1 || std::mem::size_of_val(roots.as_ref()) != root_bytes {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.release_storage(root_bytes)?;
            let (module, correspondence) = lower_module_with_call_budget_v1(
                &semantic_ssa,
                limits,
                Some(&roots),
                None,
                budget,
            )?;
            // A unit root still retains exit attribution, despite having no
            // helper calls or result transport. Keep this exact charged storage
            // live during canonical admission and expose its reservation receipt.
            budget.charge_work(correspondence.call_returns.len())?;
            if !correspondence.call_result_components.is_empty()
                || correspondence.call_returns.iter().any(|row| {
                    !matches!(
                        row.kind,
                        SemanticKirCallReturnKindV1::Return {
                            components: CallComponentSpanV1::EMPTY
                        }
                    )
                })
            {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let call_correspondence_storage = CallReturnBufferV1::bytes(
                correspondence.call_returns.len(),
                correspondence.call_result_components.len(),
            )?;
            drop(roots);
            budget.release_storage(root_bytes)?;
            let (executable, executable_storage) =
                fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV16::
                    from_module_ref_with_verification_budget_v16(&module, budget)
                    .map_err(ProductionOrderedRegionPreRankedErrorV16::Canonical)?;
            // The canonical constructor checks exact inverse equality; retain
            // only that actual decoded Module, never the transient source graph.
            drop(module);
            Ok(Self {
                semantic_ssa,
                source_launch,
                executable,
                executable_storage,
                call_correspondence_storage,
                correspondence,
            })
        })();
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }

    /// Borrows the exact freshly decoded and verified executable owner.
    pub const fn executable(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV16 {
        &self.executable
    }
    /// Returns the canonical retained-storage reservation receipt.
    pub const fn executable_storage(&self) -> fe2o3_kernel_ir::CanonicalKernelIrReplayStorageV16 {
        self.executable_storage
    }
    /// Returns the separately retained, logical root-exit attribution allocation
    /// bytes. Reserve these in addition to `executable_storage` while retaining
    /// the owner; this is not a receipt for all source/correspondence storage.
    pub const fn call_correspondence_storage(&self) -> usize {
        self.call_correspondence_storage
    }
    /// Borrows the ordinary source semantic SSA owner.
    pub const fn semantic_ssa(&self) -> &ProductionSemanticSsaOwnerV1 {
        &self.semantic_ssa
    }
    /// Borrows the exact source launch agreement used during lowering.
    pub const fn source_launch(&self) -> &crate::ProductionSourceLaunchRosterV1 {
        &self.source_launch
    }
    /// Borrows operation attribution; this is not an independent proof receipt.
    pub const fn correspondence(&self) -> &SemanticKirCorrespondenceV1 {
        &self.correspondence
    }
    /// Pre-ranked materialization never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn validate_ordered_region_context_v31(
    semantic: &AdmittedInertSemanticMirV1,
    launch: &crate::ProductionSourceLaunchRosterV1,
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticMirWireVersionV1, SemanticTargetArchitectureV1,
    };
    let fail = |detail| unsupported(0, None, None, detail);
    budget.charge_work(8)?;
    if semantic.wire_version() != SemanticMirWireVersionV1::V31
        || semantic.target().architecture() != SemanticTargetArchitectureV1::AmdGpuGfx942
        || semantic.functions().len() != 1
        || semantic.roots().len() != 1
        || launch.roots().len() != 1
        || launch.semantic_sha256() != semantic.semantic_sha256().as_bytes()
    {
        return Err(fail(
            "ordered region requires one exact V31 gfx942 source and launch root",
        ));
    }
    let root = semantic.roots()[0];
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or_else(|| fail("ordered region root is out of range"))?;
    let layout = launch.roots()[0].layout();
    if function.role() != SemanticFunctionRoleV1::KernelRoot
        || launch.roots()[0].selected_root() != root
        || layout.workgroup_extents() != [64, 1, 1]
        || layout.subgroup_size() != 64
        || !layout.full_physical_workgroups()
        || function.blocks().len() > limits.max_blocks
    {
        return Err(fail(
            "ordered region requires bounded direct-root wave64/full64 launch geometry",
        ));
    }
    let mut selected = None;
    for (index, block) in function.blocks().iter().enumerate() {
        budget.charge_work(1)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && matches!(
                semantic.callables().get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(_),
                    ..
                })
            )
        {
            budget.charge_work(32)?;
            let index = u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            semantic
                .checked_gfx942_ordered_region_call_v31(root, SemanticBlockIdV1::from_index(index))
                .map_err(|_| {
                    fail("ordered region call no longer matches the immutable semantic owner")
                })?;
            if selected.replace(index as usize).is_some() {
                return Err(fail("ordered region profile admits exactly one occurrence"));
            }
        }
    }
    let selected = selected.ok_or_else(|| fail("ordered region profile has no occurrence"))?;
    let floor = budget.storage();
    let result = (|| {
        let count = function.blocks().len();
        let bytes = argument_product_v1(count, std::mem::size_of::<usize>())?;
        budget.reserve_storage(bytes)?;
        budget.charge_work(count)?;
        let mut prefix = Vec::new();
        prefix
            .try_reserve_exact(count)
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        // Reserve any allocator-reported capacity excess before initializing or
        // traversing the storage. All exits restore the caller's floor.
        let excess = prefix
            .capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.reserve_storage(argument_product_v1(excess, std::mem::size_of::<usize>())?)?;
        prefix.resize(count, usize::MAX);
        let mut current = function.entry().index() as usize;
        let mut ordinal = 0;
        let mut statement_count = 0_usize;
        loop {
            budget.charge_work(1)?;
            let slot = prefix
                .get_mut(current)
                .ok_or_else(|| fail("ordered region prefix leaves the CFG"))?;
            if *slot != usize::MAX {
                return Err(fail("ordered region prefix is cyclic"));
            }
            *slot = ordinal;
            let block = &function.blocks()[current];
            // Prefixes may contain ordinary scalar assignments and old closed
            // NoMemory markers, but no pre-region branch/store/helper call.
            for statement in block.statements() {
                // A closed, nonrecursive rvalue inspection touches at most two
                // direct scalar operands and their indexed type declarations.
                budget.charge_work(8)?;
                statement_count = statement_count
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                enforce_limit(
                    ProductionSemanticKirResourceV1::Statements,
                    statement_count,
                    limits.max_statements,
                )?;
                match statement.kind() {
                    SemanticStatementKindV1::Assign(assign)
                        if ordered_region_scalar_prefix_assignment_v31(
                            semantic.types(),
                            assign,
                        ) => {}
                    SemanticStatementKindV1::StorageLive(_)
                    | SemanticStatementKindV1::StorageDead(_)
                    | SemanticStatementKindV1::Nop => {}
                    _ => {
                        return Err(fail(
                            "ordered region prefix contains a memory/control effect",
                        ));
                    }
                }
            }
            if current == selected {
                break;
            }
            let edge = match block.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) => *edge,
                SemanticTerminatorKindV1::Call(call)
                    if matches!(
                        call.unwind(),
                        SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
                    ) && matches!(
                        semantic.callables().get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::CompilerIntrinsic {
                            operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_),
                            ..
                        })
                    ) =>
                {
                    budget.charge_work(6)?;
                    let destination = call
                        .destination()
                        .ok_or_else(|| fail("prefix call has no return edge"))?;
                    if !destination.place().projections().is_empty()
                        || call.arguments().len() > 2
                        || call.arguments().iter().any(|operand| {
                            !ordered_region_direct_scalar_operand_v31(semantic.types(), operand)
                        })
                    {
                        return Err(fail(
                            "prefix marker inputs and destination must be direct scalar locals or constants",
                        ));
                    }
                    destination.edge()
                }
                _ => {
                    return Err(fail(
                        "ordered region is not on an unconditional entry prefix",
                    ));
                }
            };
            current = edge.target().index() as usize;
            ordinal += 1;
        }
        // A later backedge into any prefix block could execute the region again.
        // One linear edge census rejects it, including unreachable extra entries.
        for (index, block) in function.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            block.terminator().kind().try_for_each_edge(|edge| {
                budget.charge_work(1)?;
                let destination = *prefix
                    .get(edge.target().index() as usize)
                    .ok_or_else(|| fail("ordered region edge leaves the CFG"))?;
                if destination != usize::MAX
                    && (destination == 0
                        || prefix[index] == usize::MAX
                        || prefix[index] + 1 != destination)
                {
                    return Err(fail(
                        "ordered region prefix has an alternate entry or backedge",
                    ));
                }
                Ok(())
            })?;
        }
        Ok(())
    })();
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    result
}

// No projected reads, memory loads, borrowing, pointer/FP operations, unchecked
// arithmetic or potential traps can hide inside an assignment before the unit.
// General arithmetic can be admitted only by a separately reviewed exact rule.
fn ordered_region_scalar_prefix_assignment_v31(
    types: &[SemanticTypeDeclV1],
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
) -> bool {
    let scalar = |ty: SemanticTypeIdV1| {
        matches!(
            types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Scalar(
                SemanticScalarTypeV1::Integer { .. } | SemanticScalarTypeV1::Bool
            ))
        )
    };
    let operand =
        |value: &SemanticOperandV1| ordered_region_direct_scalar_operand_v31(types, value);
    if !assignment.destination().projections().is_empty()
        || !scalar(assignment.destination().ty())
        || !scalar(assignment.value().result_type())
    {
        return false;
    }
    match assignment.value().kind() {
        SemanticRvalueKindV1::Use(value) => operand(value),
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::Not,
            operand: value,
        } => operand(value),
        SemanticRvalueKindV1::Binary {
            operation:
                SemanticBinaryOpV1::BitAnd | SemanticBinaryOpV1::BitOr | SemanticBinaryOpV1::BitXor,
            left,
            right,
        } => operand(left) && operand(right),
        _ => false,
    }
}

fn ordered_region_direct_scalar_operand_v31(
    types: &[SemanticTypeDeclV1],
    value: &SemanticOperandV1,
) -> bool {
    matches!(
        types
            .get(value.ty().index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(
            SemanticScalarTypeV1::Integer { .. } | SemanticScalarTypeV1::Bool
        ))
    ) && match value {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            place.projections().is_empty()
        }
        SemanticOperandV1::Constant(constant) => {
            matches!(constant.value(), SemanticConstantValueV1::Scalar(_))
        }
    }
}
