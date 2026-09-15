// Replayed source-SSA custody for shared Workgroup and epoch transport.

mod borrowed_workgroup_01 {
    use super::*;
    use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticDefinedCapabilityContractV1, SemanticWorkgroupEpochProjectionV1,
    };
    use fe2o3_mir_model::SemanticExpandedTerminatorOriginV1;
    use super::capability_ssa_graph_01::{
        CapabilitySsaGraphV1 as Graph, CapabilityDefinitionSiteV1 as Site,
        CapabilityLoanV1 as Loan,
    };

    type Result<T> = std::result::Result<T, ProductionSemanticKirErrorV1>;

    fn reject(detail: &'static str) -> ProductionSemanticKirErrorV1 {
        unsupported(0, None, None, detail)
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(super) struct Origin {
        pub(super) issuer: SsaValueV1,
        pub(super) contract: SemanticExecutionCapabilityContractV1,
        pub(super) loans: Vec<Loan>,
        pub(super) epoch_projection: bool,
    }

    #[derive(Clone)]
    pub(super) struct EpochOccurrence {
        projection: SemanticWorkgroupEpochProjectionV1,
        binding: SemanticExpandedDefinedCapabilityV1,
    }

    /// Nonserializable replay receipt. Wire source fields alone cannot create it.
    #[derive(Clone)]
    pub(super) struct BorrowedSubgroupLoweringV1 {
        pub(super) source_contract: SemanticExecutionCapabilityContractV1,
        pub(super) source_reference: SemanticTypeIdV1,
        pub(super) owned_type: SemanticTypeIdV1,
        pub(super) owned_ssa: SsaValueV1,
        pub(super) owned_value: ValueId,
        pub(super) result_type: Type,
        pub(super) expansion_identity: [u8; 32],
        pub(super) root_identity: [u8; 32],
        pub(super) caller_instance: fe2o3_mir_model::SemanticCallInstanceIdV1,
        pub(super) source_function: SemanticFunctionIdV1,
        pub(super) source_block: SemanticBlockIdV1,
        pub(super) execution_block: SemanticBlockIdV1,
        loans: Vec<Loan>,
    }

    /// Source/SSA custody is obtained only from the replayed production owner.
    /// No API accepts a caller-manufactured occurrence or issuer roster.
    pub(super) struct BorrowedWorkgroupPlanV1<'a> {
        owner: &'a ProductionSemanticSsaOwnerV1,
        view: &'a SemanticExpandedRootV1,
        context: &'a RootKernelContextLoweringV1,
        graph: Graph<'a>,
        epochs: Vec<EpochOccurrence>,
        epoch_transports: BTreeMap<u32, (SemanticTypeIdV1, SemanticPromotedBindingV1)>,
        subgroups: BTreeMap<ValueId, BorrowedSubgroupLoweringV1>,
        partitions: BTreeMap<ValueId, Option<BorrowedSubgroupLoweringV1>>,
    }

    impl<'a> BorrowedWorkgroupPlanV1<'a> {
        pub(super) fn new(
            owner: &'a ProductionSemanticSsaOwnerV1,
            context: &'a RootKernelContextLoweringV1,
            max_work: usize,
        ) -> Result<Self> {
            owner
                .verify_replay()
                .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let view = owner
                .execution_view_for_root(context.selected_root)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let plan = owner
                .execution_plan_for_root(context.selected_root)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if plan.function_identity() != view.body().identity() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let bindings = owner
                .execution_expansion()
                .defined_capability_bindings(owner.source_semantic())
                .map_err(|_| reject("borrowed Workgroup defined-call replay mismatch"))?;
            let epochs = checked_epoch_occurrences_v1(owner, view, bindings)?;
            let mut result = Self {
                owner, view, context, epochs,
                graph: Graph::new(view.body(), plan.plan(), max_work)?,
                epoch_transports: BTreeMap::new(),
                subgroups: BTreeMap::new(), partitions: BTreeMap::new(),
            };
            result.prepare_epoch_transports()?;
            Ok(result)
        }

        /// Resolve the actual source receiver before materializing an operation.
        /// `lowering` must still be lowering this exact replayed execution body.
        pub(super) fn subgroup(
            &mut self,
            block: SemanticBlockIdV1,
            lowering: &SemanticFunctionLoweringV1<'_>,
        ) -> Result<BorrowedSubgroupLoweringV1> {
            if lowering.function != execution_function_for_root_v1(self.owner, self.view.root())?
                || lowering.kernel_context != Some(self.context)
                || lowering.execution_expansion_identity
                    != self
                        .view
                        .has_expanded_calls()
                        .then(|| *self.view.identity())
                || lowering.execution_source_expansion_identity
                    != execution_expansion_identity_v1(self.owner)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let (call, contract) = self.execution_call(block.index())?;
            let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                workgroup_reference,
                workgroup,
                subgroup,
                width,
            } = contract.operation()
            else {
                return Err(reject(
                    "expected the exact borrowed subgroup source operation",
                ));
            };
            if width != 64
                || contract.epoch_after().is_some()
                || contract.signature().arguments().collect::<Vec<_>>() != [workgroup_reference]
                || contract.signature().output() != subgroup
                || !shared_type(
                    self.owner.source_semantic().types(),
                    workgroup_reference,
                    workgroup,
                )
            {
                return Err(reject(
                    "borrowed subgroup reference, signature or epoch changed",
                ));
            }
            let [receiver] = call.arguments() else {
                return Err(reject(
                    "borrowed subgroup must retain its one shared receiver",
                ));
            };
            let receiver = receiver.clone();
            let origin = self.resolve_operand(block.index(), &receiver, &[], contract, 0)?;
            if origin.loans.is_empty() || origin.epoch_projection {
                return Err(reject("borrowed subgroup has no retained shared loan"));
            }
            self.check_live(&origin, block.index())?;
            let binding = lowering
                .semantic_ssa_bindings
                .get(&origin.issuer)
                .ok_or_else(|| reject("owned Workgroup SSA issuer has not been lowered"))?;
            let values = binding.values().map_err(reject)?;
            let [(owned_value, Type::ExecutionCapability(owned))] = values.as_slice() else {
                return Err(reject(
                    "owned Workgroup must be one existing typed SSA value",
                ));
            };
            if !owned.is_complete()
                || owned.role != ExecutionCapabilityRoleV1::Workgroup
                || owned.source_type != execution_type_identity_v1(lowering.types, workgroup)?
                || !same_provenance(owned, contract, self.context)
            {
                return Err(reject(
                    "owned Workgroup SSA issuer type or provenance changed",
                ));
            }
            // The live receiver binding must name the same SSA operand as the
            // resolved issuer. Never select a value from a type-indexed table.
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = receiver else {
                return Err(reject("borrowed subgroup constant receiver"));
            };
            let live = lowering
                .locals
                .get(place.local().index() as usize)
                .and_then(Option::as_ref)
                .ok_or_else(|| reject("borrowed subgroup receiver is not live"))?
                .values()
                .map_err(reject)?;
            if !place.projections().is_empty() || live != values {
                return Err(reject(
                    "borrowed subgroup receiver changed its exact Workgroup SSA value",
                ));
            }
            let source = &self.view.block_origins()[block.index() as usize];
            let mut result = owned.clone();
            result.source_type = execution_type_identity_v1(lowering.types, subgroup)?;
            result.role = ExecutionCapabilityRoleV1::BorrowedSubgroup {
                workgroup_reference: execution_type_identity_v1(lowering.types, workgroup_reference)?,
                workgroup: execution_type_identity_v1(lowering.types, workgroup)?, width,
            };
            Ok(BorrowedSubgroupLoweringV1 {
                source_contract: contract,
                source_reference: workgroup_reference,
                owned_type: workgroup,
                owned_ssa: origin.issuer,
                owned_value: *owned_value,
                result_type: Type::ExecutionCapability(result),
                expansion_identity: *self.owner.execution_expansion().identity(),
                root_identity: *self.view.identity(),
                caller_instance: source.instance(),
                source_function: source.function(),
                source_block: source.block(),
                execution_block: block,
                loans: origin.loans,
            })
        }

        /// Epoch consumers must join on the exact issuer SSA value before KIR
        /// additionally enforces its existing subgroup/Workgroup ValueId equality.
        pub(super) fn epoch_matches_subgroup(
            &mut self,
            block: SemanticBlockIdV1,
            epoch: &SemanticOperandV1,
            subgroup: &BorrowedSubgroupLoweringV1,
        ) -> Result<()> {
            if subgroup.expansion_identity != *self.owner.execution_expansion().identity()
                || subgroup.root_identity != *self.view.identity()
                || subgroup.loans.is_empty()
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let (call, consumer) = self.execution_call(block.index())?;
            let SemanticExecutionCapabilityOperationV1::SubgroupPartition(
                fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1::Derive {
                    epoch: epoch_type,
                    ..
                },
            ) = consumer.operation()
            else {
                return Err(reject(
                    "epoch owner comparison requires the actual partition derive call",
                ));
            };
            if call.arguments().get(1) != Some(epoch)
                || epoch.ty() != epoch_type
                || consumer.provenance() != subgroup.source_contract.provenance()
                || consumer.workgroup_brand() != subgroup.source_contract.workgroup_brand()
                || consumer.epoch_before() != subgroup.source_contract.epoch_before()
                || consumer.epoch_after().is_some()
            {
                return Err(reject(
                    "partition epoch operand or retained source contract changed",
                ));
            }
            let origin =
                self.resolve_operand(block.index(), epoch, &[], subgroup.source_contract, 0)?;
            self.check_live(&origin, block.index())?;
            for loan in &subgroup.loans {
                self.graph.loan_live(*loan, Site { block: block.index(), statement: None, local: 0 })?;
            }
            if origin.issuer != subgroup.owned_ssa || !origin.epoch_projection {
                return Err(reject(
                    "epoch and subgroup have different owned Workgroup SSA issuers",
                ));
            }
            Ok(())
        }

        fn source(&mut self) -> WorkgroupSourceResolverV1<'a, '_> {
            WorkgroupSourceResolverV1 {
                owner: self.owner, view: self.view, context: self.context,
                graph: &mut self.graph, epochs: &self.epochs,
            }
        }

        fn execution_call(
            &self, block: u32,
        ) -> Result<(&SemanticDirectCallV1, SemanticExecutionCapabilityContractV1)> {
            checked_execution_source_call_v1(self.owner, self.view, self.context, block)
        }

        fn resolve_operand(
            &mut self, block: u32, operand: &SemanticOperandV1,
            tail: &[SemanticProjectionV1], contract: SemanticExecutionCapabilityContractV1,
            depth: usize,
        ) -> Result<Origin> {
            self.source().resolve_operand(block, operand, tail, contract, depth)
        }

        fn resolve(
            &mut self, value: SsaValueV1, projections: &[SemanticProjectionV1],
            contract: SemanticExecutionCapabilityContractV1, depth: usize,
        ) -> Result<Origin> {
            self.source().resolve(value, projections, contract, depth)
        }

        fn check_live(&mut self, origin: &Origin, consumer: u32) -> Result<()> {
            self.source().check_live(origin, consumer)
        }

    }

    fn shared_type(
        types: &[SemanticTypeDeclV1],
        reference: SemanticTypeIdV1,
        owned: SemanticTypeIdV1,
    ) -> bool {
        reference != owned
            && matches!(types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Pointer(p)) if p.kind() == SemanticPointerKindV1::Reference
                && p.mutability() == SemanticMutabilityV1::Immutable && p.pointee() == owned
                && p.address_space() == 0 && p.pointer_width_bits() == 64
                && p.metadata() == SemanticPointerMetadataV1::None)
    }

    fn same_provenance(
        owned: &ExecutionCapabilityTypeV1,
        source: SemanticExecutionCapabilityContractV1,
        root: &RootKernelContextLoweringV1,
    ) -> bool {
        let p = source.provenance();
        global_capability_provenance_matches_v1(root, p)
            && owned.provenance.root == *root.context_type.root()
            && owned.provenance.kernel_binding == *p.kernel_binding().as_bytes()
            && owned.provenance.frontend_unit == *p.frontend_unit().as_bytes()
            && owned.provenance.kernel_marker == *p.kernel_marker().as_bytes()
            && owned.provenance.target_brand == *p.target_brand().as_bytes()
            && owned.provenance.launch_brand == *p.launch_brand().as_bytes()
            && owned.provenance.issuance == *p.issuance().as_bytes()
            && owned.workgroup_brand == source.workgroup_brand().map(|id| *id.as_bytes())
            && owned.epoch == source.epoch_before().map(|id| *id.as_bytes())
    }

    include!("borrowed_workgroup_01/source_custody.rs");
    include!("borrowed_workgroup_01/transpose_publish.rs");
    include!("borrowed_workgroup_01/allocation.rs");
    include!("borrowed_workgroup_01/transport.rs");
    include!("borrowed_workgroup_01/workgroup_index.rs");
    include!("borrowed_workgroup_01/dispatch.rs");

    #[cfg(test)]
    #[path = "tests.rs"]
    mod tests;
}
