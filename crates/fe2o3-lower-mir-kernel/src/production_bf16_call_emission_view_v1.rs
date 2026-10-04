/// Immutable exact nominal emission rows, borrowed from the SAME pre-ranked
/// owner. These are logical SSA components, not physical registers/FnABI.
/// The only constructor is the owner's checked materialization route.
///
/// Neither source authority nor execution authority can be reconstructed:
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::Bf16CallInstanceEmissionViewV1;
/// fn forge() { let _ = Bf16CallInstanceEmissionViewV1 {}; }
/// ```
pub struct Bf16CallInstanceEmissionViewV1<'a> {
    owner: &'a ProductionPreRankedKirOwnerV1,
    relation: &'a SealedBf16CallRelationV1,
}
impl ProductionPreRankedKirOwnerV1 {
    /// Borrows the exact emitted relation only for this closed typed profile.
    pub fn bf16_call_instance_emission_v1(&self) -> Option<Bf16CallInstanceEmissionViewV1<'_>> {
        self.helper_memory
            .bf16_nominal
            .as_deref()
            .map(|relation| Bf16CallInstanceEmissionViewV1 {
                owner: self,
                relation,
            })
    }
}
impl<'a> Bf16CallInstanceEmissionViewV1<'a> {
    /// Same retained source, graph, correspondence and resource receipts.
    pub const fn owner(&self) -> &'a ProductionPreRankedKirOwnerV1 {
        self.owner
    }
    /// Actual semantic root identity, not an assumed ordinal.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.relation.root
    }
    /// Actual semantic helper identity.
    pub const fn helper(&self) -> SemanticFunctionIdV1 {
        self.relation.helper
    }
    fn function(&self, id: SemanticFunctionIdV1) -> &'a FunctionId {
        &self
            .owner
            .correspondence
            .lowered_functions
            .iter()
            .find(|row| row.semantic_function == id)
            .expect("sealed two-function relation")
            .kernel_ir_function
    }
    /// Actual canonical root in this owner's graph.
    pub fn root_function(&self) -> &'a FunctionId {
        self.function(self.root())
    }
    /// Actual canonical helper in this owner's graph.
    pub fn helper_function(&self) -> &'a FunctionId {
        self.function(self.helper())
    }
    /// Source block containing the actual Defined call.
    pub const fn source_call_block(&self) -> SemanticBlockIdV1 {
        self.relation.source_call_block
    }
    /// Source block containing the actual helper MFMA terminal.
    pub const fn source_matrix_block(&self) -> SemanticBlockIdV1 {
        self.relation.source_matrix_block
    }
    /// Actual root Call block and operation ordinal.
    pub const fn call_site(&self) -> (BlockId, u32) {
        (self.relation.call_block, self.relation.call_ordinal)
    }
    /// Actual helper Matrix block and operation ordinal.
    pub const fn matrix_site(&self) -> (BlockId, u32) {
        (self.relation.matrix_block, self.relation.matrix_ordinal)
    }
    /// Exact live source producer components. Context has no runtime component;
    /// Lane has one; each nominal fragment/accumulator/values has four.
    pub fn producer_components(&self, role: Bf16CallInstanceRoleV1) -> &'a [ValueId] {
        let (index, count) = match role {
            Bf16CallInstanceRoleV1::Context => (0, 0),
            Bf16CallInstanceRoleV1::Lane => (1, 1),
            Bf16CallInstanceRoleV1::Lhs => (2, 4),
            Bf16CallInstanceRoleV1::Rhs => (3, 4),
            Bf16CallInstanceRoleV1::Zero => (4, 4),
            Bf16CallInstanceRoleV1::Result => (5, 4),
            Bf16CallInstanceRoleV1::Values => (6, 4),
        };
        &self.relation.capture.producers[index][..count]
    }
    /// Ordered actual Call components for source arguments context/A/B/acc.
    /// An empty context component list is not an erased physical pointer claim.
    pub fn call_argument_components(&self, index: usize) -> Option<&'a [ValueId]> {
        self.relation
            .capture
            .arguments
            .get(index)
            .map(|row| &row[..if index == 0 { 0 } else { 4 }])
    }
    /// Ordered actual helper entry components for context/A/B/acc.
    pub fn formal_components(&self, index: usize) -> Option<&'a [ValueId]> {
        self.relation
            .capture
            .formals
            .get(index)
            .map(|row| &row[..if index == 0 { 0 } else { 4 }])
    }
    /// Four actual root Call definitions, joined to the captured source result.
    pub const fn call_results(&self) -> &'a [ValueId; 4] {
        &self.relation.capture.call_result
    }
    /// Four actual helper Return operands, possibly identity edge parameters.
    pub const fn helper_return(&self) -> &'a [ValueId; 4] {
        &self.relation.helper_return
    }
    /// Source-proved Identity/Swap01, independently joined to actual Return.
    pub const fn return_permutation(&self) -> [u8; 4] {
        self.relation.permutation
    }
    /// Exact same-owner Call source operation span.
    pub fn call_span(&self) -> &'a SemanticKirTerminatorOperationSpanV1 {
        self.span(self.root(), self.source_call_block())
    }
    /// Exact same-owner MFMA source operation span.
    pub fn matrix_span(&self) -> &'a SemanticKirTerminatorOperationSpanV1 {
        self.span(self.helper(), self.source_matrix_block())
    }
    fn span(
        &self,
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    ) -> &'a SemanticKirTerminatorOperationSpanV1 {
        self.owner
            .correspondence
            .terminator_operation_spans
            .iter()
            .find(|row| {
                row.correspondence_owner == self.root()
                    && row.semantic_function == function
                    && row.semantic_block == block
            })
            .expect("sealed complete source coverage")
    }
}
