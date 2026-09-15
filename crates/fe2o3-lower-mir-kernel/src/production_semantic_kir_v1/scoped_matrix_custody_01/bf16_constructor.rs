//! Exact constructor-receiver to live Bind/lane join. This is not a checked
//! Result, Global extent, or four-read theorem.
use super::resolve::{Requirement, Resolver, Role};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticPolicyMatrixBindTypesV1;

impl<'a> ProductionScopedMatrixSourceSessionV1<'a> {
    /// Resolves a captured constructor's original receiver at its retained use.
    /// Identity/types are inert requirements, never the selected Bind issuer.
    /// The compiler must independently authenticate the constructor provider.
    /// All receiver/issuer checks precede insertion of the checked lane row.
    pub fn checked_bf16_constructor_lane(
        &mut self,
        source: ProductionGlobalBf16SourceRowV1<'_, 'a>,
        identity: SemanticDefinedMatrixIdentityV1,
        types: SemanticPolicyMatrixBindTypesV1,
    ) -> Result<ProductionScopedBf16LaneUseV1> {
        self.require_bf16_scope()?;
        if !std::ptr::eq(source.owner(), self.owner)
            || !std::ptr::eq(source.view(), self.view)
            || source.contract().provenance() != identity.provenance()
            || source.contract().matrix_brand() != identity.matrix_brand()
            || source.contract().global_brand() != identity.kernel_brand()
        {
            return Err(mismatch());
        }
        let receiver = source.receiver();
        let receiver_site = Site {
            block: receiver.site().block().index(),
            statement: receiver.site().statement(),
            local: receiver.destination_local(),
        };
        if resolve::shared_pointee(
            self.owner.source_semantic().types(),
            receiver.operand().ty(),
        ) != Some(types.bound)
            || receiver_site.statement.is_none()
        {
            return Err(reject(
                "BF16 constructor receiver lost its original shared pair",
            ));
        }
        // Select a single structural requirement, not a compatible occurrence.
        // The real issuer below is resolved from the original source use.
        let mut requirement = None;
        for binding in self.occurrences.binds.values() {
            self.graph.charge(1)?;
            if binding.record.identity() == identity && binding.record.types() == types {
                if requirement
                    .replace(binding.record)
                    .is_some_and(|old| old != binding.record)
                {
                    return Err(reject("BF16 constructor has ambiguous Bind requirements"));
                }
            }
        }
        let requirement = requirement
            .ok_or_else(|| reject("BF16 constructor has no matching original Bind contract"))?;
        let query = self
            .owner
            .source_query_for_root(self.view.root(), self.view.body())
            .map_err(|_| mismatch())?;
        let mut work_error = None;
        let receiver_use =
            query.operand_use(receiver.site(), receiver.operand(), &mut || match self
                .graph
                .charge(1)
            {
                Ok(()) => true,
                Err(error) => {
                    work_error = Some(error);
                    false
                }
            });
        if let Some(error) = work_error {
            return Err(error);
        }
        let receiver_use = receiver_use
            .map_err(|_| reject("BF16 constructor receiver has no exact retained source use"))?;
        let consumer = Site {
            block: source.load_block(),
            statement: None,
            local: source
                .call()
                .destination()
                .ok_or_else(mismatch)?
                .place()
                .local()
                .index(),
        };
        let mut resolver = Resolver {
            owner: self.owner,
            view: self.view,
            context: &self.context,
            graph: &mut self.graph,
            occurrences: &self.occurrences,
            epochs: &self.epochs,
            requirement: Requirement::Bound(requirement),
        };
        let bound = resolver.value(
            receiver_use.value(),
            &[],
            receiver.operand().ty(),
            Role::Bound,
            0,
        )?;
        if bound.narrow.is_some() {
            return Err(reject(
                "BF16 constructor cannot substitute a narrowed Matrix owner",
            ));
        }
        resolver.live(&bound, receiver_site)?;
        resolver.live(&bound, consumer)?;
        let bind = bound.bind.ok_or_else(mismatch)?;
        let binding = self.occurrences.binds.get(&bind).ok_or_else(mismatch)?;
        if binding.record != requirement {
            return Err(mismatch());
        }
        let returned = self.graph.use_value(
            binding.returned.block,
            binding.binding.callee_return().index(),
        )?;
        if returned != bound.issuer || self.graph.definition(returned)? != binding.site {
            return Err(reject(
                "BF16 constructor receiver differs from its actual Bind return",
            ));
        }
        // No candidate/row rollback: all constructor comparisons are complete.
        // The existing lane query keeps all Context/Workgroup/epoch checks and
        // inserts only after its own complete checks succeed.
        self.checked_bf16_lane_at(
            source.view().body(),
            source.load_block(),
            source.call(),
            binding.binding.expanded_call_block().index(),
        )
    }
}
