// Borrowed inspection only. No persisted transport or executable owner projection.
mod ordered_region_inspection_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAME,
        AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAMESPACE,
        AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirBlockCoordinateV1,
        CanonicalKirFunctionCoordinateV1, CanonicalKirOperationCoordinateV1,
        Gfx942OrderedRegionErrorV1, Gfx942OrderedRegionProfileV1, TargetCapability,
        ValidatedGfx942OrderedRegionV1, VerifiedCanonicalKernelIrIdentityV16, WaveWidth,
        validate_gfx942_ordered_region_v1,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticMirWireVersionV1, SemanticSourceProvenanceV1, SemanticTargetArchitectureV1,
    };

    /// Hard per-query scan bound, in precharged logical work units, not time.
    pub const MAX_PRODUCTION_ORDERED_REGION_INSPECTION_WORK_V1: usize = 1_048_576;

    /// Additional local ceiling on the existing cumulative verification ledger.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ProductionOrderedRegionInspectionLimitsV1 {
        /// Positive bound no larger than the hard inspection ceiling.
        pub max_work: usize,
    }
    impl Default for ProductionOrderedRegionInspectionLimitsV1 {
        fn default() -> Self {
            Self {
                max_work: MAX_PRODUCTION_ORDERED_REGION_INSPECTION_WORK_V1,
            }
        }
    }

    /// Bounded failures; none carries copied source text or a substitute owner.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionOrderedRegionInspectionErrorV1 {
        /// Zero or above-hard-ceiling local bound.
        InvalidLimit,
        /// Rejected next local charge; accepted cumulative work is retained.
        WorkLimit {
            /// Attempted local total.
            actual: usize,
            /// Admitted local ceiling.
            limit: usize,
        },
        /// Existing cumulative work/storage ledger failure.
        Resource(Resource),
        /// Both digest and canonical length must identify this immutable owner.
        StaleIdentity,
        /// The direct-root target, launch or capability profile is unsupported.
        UnsupportedProfile,
        /// Exactly one ordered region must exist.
        MissingOrAmbiguousRegion,
        /// A supplied roster coordinate does not name that exact operation.
        WrongCoordinate,
        /// The retained semantic/function/terminator association is not exact.
        InvalidCorrespondence,
        /// Actual operand definitions or the fixed instruction contract disagree.
        InvalidDescriptor(Gfx942OrderedRegionErrorV1),
    }
    impl fmt::Display for ProductionOrderedRegionInspectionErrorV1 {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "ordered-region inspection: {self:?}")
        }
    }
    impl Error for ProductionOrderedRegionInspectionErrorV1 {}
    impl From<Resource> for ProductionOrderedRegionInspectionErrorV1 {
        fn from(error: Resource) -> Self {
            Self::Resource(error)
        }
    }
    type InspectionError = ProductionOrderedRegionInspectionErrorV1;

    /// Explicit availability, never inferred physical observations or authority.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum ProductionOrderedRegionInspectionAvailabilityV1 {
        /// Operation attribution from the retained semantic lowering owner only.
        RetainedSemanticCorrespondence,
        /// Not provided by this borrowed inspection.
        Unavailable,
    }

    /// Exact fixed logical view payload; not heap/RSS or all retained source data.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ProductionOrderedRegionInspectionStorageV1(usize);
    impl ProductionOrderedRegionInspectionStorageV1 {
        /// Reserve while retaining the view, in addition to the input receipts.
        pub const fn retained_storage(self) -> usize {
            self.0
        }
    }

    /// Live immutable plan/source association. Planned VGPR numbers and high-water
    /// are not register contents, lifetimes, occupancy or final machine mappings.
    /// Inert semantic fixtures can create the underlying owner, so this view does
    /// not authenticate source origin, launch fields, or compiler custody.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionOrderedRegionInspectionV1;
    /// let forged = ProductionOrderedRegionInspectionV1 {};
    /// ```
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionOrderedRegionInspectionV1;
    /// fn escape<'a>(view: ProductionOrderedRegionInspectionV1<'a>)
    ///     -> ProductionOrderedRegionInspectionV1<'static> { view }
    /// ```
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionOrderedRegionInspectionV1;
    /// fn change(view: &mut ProductionOrderedRegionInspectionV1<'_>) {
    ///     *view.region() = *view.region();
    /// }
    /// ```
    #[derive(Debug)]
    pub struct ProductionOrderedRegionInspectionV1<'owner> {
        owner: &'owner ProductionOrderedRegionPreRankedKirOwnerV16,
        coordinate: CanonicalKirOperationCoordinateV1,
        span: SemanticKirTerminatorOperationSpanV1,
        region: ValidatedGfx942OrderedRegionV1,
        provenance: SemanticSourceProvenanceV1,
        launch: &'owner crate::ProductionSourceLaunchRootV1,
    }
    const _: () = assert!(std::mem::size_of::<ProductionOrderedRegionInspectionV1<'_>>() <= 1024);

    impl ProductionOrderedRegionInspectionV1<'_> {
        /// Exact V16 digest and length of the borrowed executable owner.
        pub fn canonical_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV16 {
            self.owner.executable().canonical().identity()
        }
        /// Retained semantic graph identity, not source-origin authentication.
        pub fn semantic_sha256(&self) -> &[u8; 32] {
            self.owner.correspondence().semantic_sha256()
        }
        /// Function, block and operation roster ordinals, never raw IDs.
        pub const fn coordinate(&self) -> CanonicalKirOperationCoordinateV1 {
            self.coordinate
        }
        /// Raw block ID, explicitly distinct from the coordinate's block ordinal.
        pub const fn kernel_ir_block(&self) -> BlockId {
            self.span.kernel_ir_block()
        }
        /// Exact semantic function selected by retained correspondence.
        pub const fn semantic_function(&self) -> SemanticFunctionIdV1 {
            self.span.semantic_function()
        }
        /// Exact semantic block containing the source call terminator.
        pub const fn semantic_block(&self) -> SemanticBlockIdV1 {
            self.span.semantic_block()
        }
        /// Fixed two-step plan with actual logical operands and region-local VGPR bindings.
        pub const fn region(&self) -> &ValidatedGfx942OrderedRegionV1 {
            &self.region
        }
        /// Captured coordinates only; no path is opened or source text inferred.
        pub const fn source_provenance(&self) -> SemanticSourceProvenanceV1 {
            self.provenance
        }
        /// Complete emitted terminator span; constants remain separate operations.
        pub const fn terminator_span(&self) -> SemanticKirTerminatorOperationSpanV1 {
            self.span
        }
        /// Declared closed instruction profile, not a device observation.
        pub const fn declared_profile(&self) -> Gfx942OrderedRegionProfileV1 {
            self.region.profile()
        }
        /// Exact target declaration checked on module, kernel and function.
        pub const fn declared_target(&self) -> &'static str {
            "gfx942:xnack-"
        }
        /// Exact required wave width, not observed execution configuration.
        pub const fn declared_wave_width(&self) -> WaveWidth {
            WaveWidth::Wave64
        }
        /// Retained source launch agreement, without origin authentication.
        pub const fn source_launch(&self) -> &crate::ProductionSourceLaunchRootV1 {
            self.launch
        }
        /// Association available from this owner, not an independent equivalence proof.
        pub const fn source_association(&self) -> ProductionOrderedRegionInspectionAvailabilityV1 {
            ProductionOrderedRegionInspectionAvailabilityV1::RetainedSemanticCorrespondence
        }
        /// Physical register values are never supplied by a static plan.
        pub const fn physical_values(&self) -> ProductionOrderedRegionInspectionAvailabilityV1 {
            ProductionOrderedRegionInspectionAvailabilityV1::Unavailable
        }
        /// No final-artifact mapping is available.
        pub const fn final_artifact(&self) -> ProductionOrderedRegionInspectionAvailabilityV1 {
            ProductionOrderedRegionInspectionAvailabilityV1::Unavailable
        }
        /// Source insertion/resume is not implemented by this view.
        pub const fn source_insertion(&self) -> ProductionOrderedRegionInspectionAvailabilityV1 {
            ProductionOrderedRegionInspectionAvailabilityV1::Unavailable
        }
        /// A direct-root-only marker cannot use the existing helper materializer.
        pub const fn can_materialize_helper(&self) -> bool {
            false
        }
        /// Inert source correspondence is not compiler/source authentication.
        pub const fn authenticates_source(&self) -> bool {
            false
        }
        /// Inspection grants neither proof nor compilation-resume authority.
        pub const fn grants_proof_or_resume_authority(&self) -> bool {
            false
        }
        /// Inspection grants no artifact, loading or launch authority.
        pub const fn grants_artifact_or_launch_authority(&self) -> bool {
            false
        }
    }

    struct InspectionMeter<'a, 'work> {
        budget: &'a mut Budget<'work>,
        work: usize,
        limit: usize,
    }
    impl InspectionMeter<'_, '_> {
        fn charge(&mut self, amount: usize) -> Result<(), InspectionError> {
            let actual = self.work.checked_add(amount).ok_or(Resource::Arithmetic)?;
            if actual > self.limit {
                return Err(InspectionError::WorkLimit {
                    actual,
                    limit: self.limit,
                });
            }
            self.budget.charge_work(amount)?;
            self.work = actual;
            Ok(())
        }
        fn names(&mut self, a: &str, b: &str) -> Result<(), InspectionError> {
            self.charge(
                a.len()
                    .checked_add(b.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )
        }
    }

    impl ProductionOrderedRegionPreRankedKirOwnerV16 {
        /// Finds the exact sole region in this live owner and checks its retained
        /// semantic association under one cumulative budget and local scan bound.
        /// `None` discovers the coordinate; `Some` must name that exact operation.
        ///
        /// Reserve this owner's executable and call-correspondence receipts before
        /// entry. Every result/unwind restores the incoming storage floor without
        /// resetting work, peak or previous failure history. Reserve the returned
        /// fixed view receipt before subsequent allocation while retaining the view.
        /// Source engines retain their existing separate bounds; this is not RSS.
        pub fn inspect_ordered_region_v1<'owner>(
            &'owner self,
            expected: &VerifiedCanonicalKernelIrIdentityV16,
            requested: Option<CanonicalKirOperationCoordinateV1>,
            limits: ProductionOrderedRegionInspectionLimitsV1,
            budget: &mut Budget<'_>,
        ) -> Result<
            (
                ProductionOrderedRegionInspectionV1<'owner>,
                ProductionOrderedRegionInspectionStorageV1,
            ),
            InspectionError,
        > {
            if limits.max_work == 0
                || limits.max_work > MAX_PRODUCTION_ORDERED_REGION_INSPECTION_WORK_V1
            {
                return Err(InspectionError::InvalidLimit);
            }
            let floor = budget.storage();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut meter = InspectionMeter {
                    budget,
                    work: 0,
                    limit: limits.max_work,
                };
                meter.charge(4)?;
                let minimum = self
                    .executable_storage()
                    .retained_storage()
                    .checked_add(self.call_correspondence_storage())
                    .ok_or(Resource::Arithmetic)?;
                if floor < minimum {
                    return Err(Resource::Accounting.into());
                }
                let bytes = std::mem::size_of::<ProductionOrderedRegionInspectionV1<'_>>();
                meter.budget.reserve_storage(bytes)?;
                meter.charge(33)?;
                if expected != self.executable().canonical().identity() {
                    return Err(InspectionError::StaleIdentity);
                }
                let view = inspect(self, requested, &mut meter)?;
                Ok((view, ProductionOrderedRegionInspectionStorageV1(bytes)))
            }));
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?,
            )?;
            match result {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        }
    }

    fn capabilities(
        values: &std::collections::BTreeSet<TargetCapability>,
        meter: &mut InspectionMeter<'_, '_>,
    ) -> Result<(), InspectionError> {
        let (mut target, mut region, mut wave) = (false, false, false);
        for value in values {
            meter.charge(4)?;
            match value {
                TargetCapability::WaveWidth(WaveWidth::Wave64) => wave = true,
                TargetCapability::WaveWidth(_) => return Err(InspectionError::UnsupportedProfile),
                TargetCapability::SubgroupSize(size) if *size != 64 => {
                    return Err(InspectionError::UnsupportedProfile);
                }
                TargetCapability::Extension { namespace, name } => {
                    // Charge all variable bytes before the closed namespace/name comparisons.
                    meter.charge(
                        namespace
                            .len()
                            .checked_add(name.len())
                            .and_then(|n| n.checked_mul(2))
                            .and_then(|n| n.checked_add(128))
                            .ok_or(Resource::Arithmetic)?,
                    )?;
                    if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE {
                        if name != AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME {
                            return Err(InspectionError::UnsupportedProfile);
                        }
                        target = true;
                    }
                    if namespace == AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAMESPACE
                        && name == AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAME
                    {
                        region = true;
                    }
                }
                _ => {}
            }
        }
        if !(target && region && wave) {
            return Err(InspectionError::UnsupportedProfile);
        }
        Ok(())
    }

    fn definition(
        inputs: &[ValueId; 3],
        id: ValueId,
        ty: &Type,
        found: &mut [Option<ScalarType>; 3],
        seen: &mut [bool; 3],
        meter: &mut InspectionMeter<'_, '_>,
    ) -> Result<(), InspectionError> {
        meter.charge(7)?;
        for index in 0..3 {
            if inputs[index] == id {
                if seen[index] {
                    return Err(InspectionError::InvalidDescriptor(
                        Gfx942OrderedRegionErrorV1::InputType,
                    ));
                }
                seen[index] = true;
                found[index] = ty.as_scalar();
            }
        }
        Ok(())
    }

    fn inspect<'owner>(
        owner: &'owner ProductionOrderedRegionPreRankedKirOwnerV16,
        requested: Option<CanonicalKirOperationCoordinateV1>,
        meter: &mut InspectionMeter<'_, '_>,
    ) -> Result<ProductionOrderedRegionInspectionV1<'owner>, InspectionError> {
        let module = owner.executable().module();
        let semantic = owner.semantic_ssa().source_semantic();
        let correspondence = owner.correspondence();
        meter.charge(112)?;
        if module.kernels.len() != 1
            || module.functions.len() != 1
            || semantic.roots().len() != 1
            || semantic.functions().len() != 1
            || owner.source_launch().roots().len() != 1
            || semantic.wire_version() != SemanticMirWireVersionV1::V31
            || semantic.target().architecture() != SemanticTargetArchitectureV1::AmdGpuGfx942
            || correspondence.semantic_sha256() != semantic.semantic_sha256().as_bytes()
            || owner.source_launch().semantic_sha256() != semantic.semantic_sha256().as_bytes()
        {
            return Err(InspectionError::UnsupportedProfile);
        }
        let root = semantic.roots()[0];
        let source_function = semantic
            .functions()
            .get(root.index() as usize)
            .ok_or(InspectionError::InvalidCorrespondence)?;
        let launch = &owner.source_launch().roots()[0];
        let layout = launch.layout();
        meter.charge(40)?;
        if source_function.role() != SemanticFunctionRoleV1::KernelRoot
            || launch.selected_root() != root
            || launch.semantic_root_identity() != source_function.identity()
            || layout.workgroup_extents() != [64, 1, 1]
            || layout.subgroup_size() != 64
            || !layout.full_physical_workgroups()
            || module.kernels[0].workgroup_size
                != Some(fe2o3_kernel_ir::WorkgroupSize::new(64, 1, 1))
        {
            return Err(InspectionError::UnsupportedProfile);
        }
        let mut selected = None;
        for (function_index, function) in module.functions.iter().enumerate() {
            meter.charge(1)?;
            let Some(body) = &function.body else {
                continue;
            };
            for (block_index, block) in body.blocks.iter().enumerate() {
                meter.charge(1)?;
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    meter.charge(1)?;
                    if matches!(operation.kind, OperationKind::Gfx942OrderedRegion(_)) {
                        let coordinate = CanonicalKirOperationCoordinateV1 {
                            block: CanonicalKirBlockCoordinateV1 {
                                function: CanonicalKirFunctionCoordinateV1(
                                    u32::try_from(function_index)
                                        .map_err(|_| Resource::Arithmetic)?,
                                ),
                                block: u32::try_from(block_index)
                                    .map_err(|_| Resource::Arithmetic)?,
                            },
                            operation: u32::try_from(operation_index)
                                .map_err(|_| Resource::Arithmetic)?,
                        };
                        if selected
                            .replace((function, body, block, operation, coordinate))
                            .is_some()
                        {
                            return Err(InspectionError::MissingOrAmbiguousRegion);
                        }
                    }
                }
            }
        }
        let (function, body, block, operation, coordinate) =
            selected.ok_or(InspectionError::MissingOrAmbiguousRegion)?;
        meter.names(function.id.as_str(), module.kernels[0].entry.as_str())?;
        if function.id != module.kernels[0].entry
            || function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
        {
            return Err(InspectionError::UnsupportedProfile);
        }
        for declarations in [
            &module.required_capabilities,
            &module.kernels[0].required_capabilities,
            &function.required_capabilities,
        ] {
            capabilities(declarations, meter)?;
        }
        let OperationKind::Gfx942OrderedRegion(payload) = &operation.kind else {
            unreachable!()
        };
        let (mut found, mut seen) = ([None; 3], [false; 3]);
        meter.charge(1)?;
        if body.parameters.len() != function.signature.parameters.len() {
            return Err(InspectionError::InvalidDescriptor(
                Gfx942OrderedRegionErrorV1::InputType,
            ));
        }
        for (id, ty) in body.parameters.iter().zip(&function.signature.parameters) {
            definition(payload.inputs(), *id, ty, &mut found, &mut seen, meter)?;
        }
        for candidate in &body.blocks {
            meter.charge(1)?;
            for value in &candidate.parameters {
                definition(
                    payload.inputs(),
                    value.id,
                    &value.ty,
                    &mut found,
                    &mut seen,
                    meter,
                )?;
            }
            for candidate_operation in &candidate.operations {
                meter.charge(1)?;
                for value in &candidate_operation.results {
                    definition(
                        payload.inputs(),
                        value.id,
                        &value.ty,
                        &mut found,
                        &mut seen,
                        meter,
                    )?;
                }
            }
        }
        // Four 32-byte source comparisons, five bounded register checks/pairs,
        // result checks, and three fixed three-slot operand lookups fit this fee.
        meter.charge(256)?;
        let region = validate_gfx942_ordered_region_v1(operation, |id| {
            payload
                .inputs()
                .iter()
                .position(|input| *input == id)
                .and_then(|index| found[index])
        })
        .map_err(InspectionError::InvalidDescriptor)?;
        let mut mapped = None;
        for row in correspondence.lowered_functions() {
            meter.names(row.kernel_ir_function().as_str(), function.id.as_str())?;
            if row.kernel_ir_function() == &function.id && mapped.replace(row).is_some() {
                return Err(InspectionError::InvalidCorrespondence);
            }
        }
        let mapped = mapped.ok_or(InspectionError::InvalidCorrespondence)?;
        meter.charge(3)?;
        if mapped.semantic_function() != root
            || mapped.correspondence_owner() != root
            || mapped.role() != SemanticKirFunctionRoleV1::KernelEntry
        {
            return Err(InspectionError::InvalidCorrespondence);
        }
        let mut selected_span = None;
        for span in correspondence.terminator_operation_spans() {
            meter.charge(9)?;
            if span.correspondence_owner() != mapped.correspondence_owner()
                || span.semantic_function() != mapped.semantic_function()
                || span.kernel_ir_block() != block.id
            {
                continue;
            }
            let end = span
                .first_operation_ordinal()
                .checked_add(span.operation_count())
                .ok_or(InspectionError::InvalidCorrespondence)?;
            if end as usize > block.operations.len() {
                return Err(InspectionError::InvalidCorrespondence);
            }
            if span.first_operation_ordinal() <= coordinate.operation
                && coordinate.operation < end
                && selected_span.replace(*span).is_some()
            {
                return Err(InspectionError::InvalidCorrespondence);
            }
        }
        let span = selected_span.ok_or(InspectionError::InvalidCorrespondence)?;
        // The shared checker inspects fixed eight operands, eight ABI positions,
        // five bindings and fixed identities. Precharge before those bounded reads.
        meter.charge(1024)?;
        let source_call = semantic
            .checked_gfx942_ordered_region_call_v31(span.semantic_function(), span.semantic_block())
            .map_err(|_| InspectionError::InvalidCorrespondence)?;
        let source = source_call.source();
        let physical = source_call.registers();
        if region.source()
            != fe2o3_kernel_ir::AssemblySourceIdentity::new(
                source.frontend_unit(),
                *source.function().as_bytes(),
                source.contract(),
                source.statement(),
            )
            || region.registers().scratch() != physical.scratch()
            || region.registers().output() != physical.output()
            || region.registers().inputs() != physical.inputs()
        {
            return Err(InspectionError::InvalidCorrespondence);
        }
        let source_block = source_function
            .blocks()
            .get(span.semantic_block().index() as usize)
            .ok_or(InspectionError::InvalidCorrespondence)?;
        let end = span
            .first_operation_ordinal()
            .checked_add(span.operation_count())
            .ok_or(InspectionError::InvalidCorrespondence)?;
        let mut regions = 0usize;
        for candidate in &block.operations[span.first_operation_ordinal() as usize..end as usize] {
            meter.charge(1)?;
            regions += usize::from(matches!(
                candidate.kind,
                OperationKind::Gfx942OrderedRegion(_)
            ));
        }
        meter.charge(4)?;
        if regions != 1 {
            return Err(InspectionError::InvalidCorrespondence);
        }
        if requested.is_some_and(|requested| requested != coordinate) {
            return Err(InspectionError::WrongCoordinate);
        }
        Ok(ProductionOrderedRegionInspectionV1 {
            owner,
            coordinate,
            span,
            region,
            provenance: source_block.terminator().source(),
            launch,
        })
    }
}
pub use ordered_region_inspection_v1::*;
