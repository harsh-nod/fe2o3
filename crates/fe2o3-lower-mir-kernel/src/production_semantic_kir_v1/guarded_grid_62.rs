mod guarded_grid_kir_62 {
    use super::capability_ssa_graph_01::CapabilitySsaGraphV1;
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticBasicBlockV1, SemanticCapabilityMemoryAliasingV1,
    };
    use fe2o3_pliron::{
        ProductionGuardedGridActionKindV1 as ActionKind, ProductionGuardedGridActionV1 as Action,
        ProductionGuardedGridIndexV1 as Index,
        ProductionSemanticSsaSourceQueryErrorV1 as QueryError,
    };

    type Site = (SemanticBlockIdV1, u32);
    pub(super) struct GuardedGridKirPlanV1<'a> {
        graph: CapabilitySsaGraphV1<'a>,
        actions: Vec<(Action<'a>, bool)>,
        stores: Vec<(SemanticBlockIdV1, Index<'a>, bool)>,
    }
    fn mismatch() -> ProductionSemanticKirErrorV1 {
        ProductionSemanticKirErrorV1::CorrespondenceMismatch
    }
    fn reserve<T>(
        count: usize,
        graph: &mut CapabilitySsaGraphV1<'_>,
    ) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
        let words = std::mem::size_of::<T>()
            .div_ceil(std::mem::size_of::<usize>())
            .max(1);
        graph.charge(count.checked_mul(words).ok_or_else(mismatch)?)?;
        let mut result = Vec::new();
        result.try_reserve_exact(count).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            }
        })?;
        graph.charge(
            result
                .capacity()
                .saturating_sub(count)
                .checked_mul(words)
                .ok_or_else(mismatch)?,
        )?;
        Ok(result)
    }
    fn query<T>(
        graph: &mut CapabilitySsaGraphV1<'_>,
        operation: impl FnOnce(&mut dyn FnMut(usize) -> bool) -> Result<T, QueryError>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let mut failure = None;
        let result = operation(&mut |amount| match graph.charge(amount) {
            Ok(()) => true,
            Err(error) => {
                failure = Some(error);
                false
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        result.map_err(|_| mismatch())
    }

    impl<'a> GuardedGridKirPlanV1<'a> {
        pub(super) fn new(
            owner: &'a ProductionSemanticSsaOwnerV1,
            function: &'a SemanticFunctionDeclV1,
            context: &RootKernelContextLoweringV1,
            max_work: usize,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            // The caller has already crossed the existing owner replay boundary.
            let source = owner
                .guarded_grid_source_for_root(context.selected_root, function)
                .map_err(|_| mismatch())?;
            let ssa = owner
                .execution_plan_for_root(context.selected_root)
                .ok_or_else(mismatch)?;
            let mut graph = CapabilitySsaGraphV1::new(function, ssa.plan(), max_work)?;
            let mut actions = reserve(source.event_count(), &mut graph)?;
            for i in 0..source.event_count() {
                let site = source.event_site(i).ok_or_else(mismatch)?;
                let action = query(&mut graph, |charge| {
                    source.action_at(site, &mut |n| charge(n))
                })?
                .ok_or_else(mismatch)?;
                if !action.belongs_to(&source)
                    || !global_capability_provenance_matches_v1(
                        context,
                        action.receipt().contract().provenance(),
                    )
                    || !matches!(owner.source_semantic().types().get(action.receipt().contract().types().context_reference.index() as usize)
                        .map(SemanticTypeDeclV1::shape),Some(SemanticTypeShapeV1::Pointer(p)) if p.pointee()==context.semantic_type)
                {
                    return Err(mismatch());
                }
                actions.push((action, false));
            }
            graph.charge(function.blocks().len())?;
            let is_store = |block: &SemanticBasicBlockV1| {
                matches!(block.terminator().kind(),SemanticTerminatorKindV1::Call(call)
                    if matches!(owner.source_semantic().callables().get(call.callee().index() as usize),
                        Some(SemanticCallableDeclV1::CompilerIntrinsic {operation:SemanticCompilerIntrinsicOperationV1::CapabilityGlobalStore {contract,..},..})
                            if contract.aliasing()==SemanticCapabilityMemoryAliasingV1::Disjoint(SemanticDisjointIndexSpaceV1::GridExclusive)))
            };
            let count = function.blocks().iter().filter(|b| is_store(b)).count();
            let mut stores = reserve(count, &mut graph)?;
            graph.charge(function.blocks().len())?;
            for (i, block) in function.blocks().iter().enumerate() {
                if !is_store(block) {
                    continue;
                }
                let block =
                    SemanticBlockIdV1::from_index(u32::try_from(i).map_err(|_| mismatch())?);
                let index = query(&mut graph, |charge| {
                    source.index_for_store(block, &mut |n| charge(n))
                })?
                .ok_or_else(mismatch)?;
                if !index.belongs_to(&source)
                    || !global_capability_provenance_matches_v1(
                        context,
                        index.receipt().contract().provenance(),
                    )
                {
                    return Err(mismatch());
                }
                stores.push((block, index, false));
            }
            Ok(Self {
                graph,
                actions,
                stores,
            })
        }
        fn action(&mut self, site: Site) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
            self.graph.charge(self.actions.len())?;
            let mut found = None;
            for (i, (a, _)) in self.actions.iter().enumerate() {
                if (a.site().block(), a.site().statement()) == (site.0, Some(site.1))
                    && found.replace(i).is_some()
                {
                    return Err(mismatch());
                }
            }
            Ok(found)
        }
        pub(super) fn finish(&mut self) -> Result<(), ProductionSemanticKirErrorV1> {
            self.graph.charge(
                self.actions
                    .len()
                    .checked_add(self.stores.len())
                    .ok_or_else(mismatch)?,
            )?;
            if self.actions.iter().any(|(_, done)| !done)
                || self.stores.iter().any(|(_, _, done)| !done)
            {
                return Err(mismatch());
            }
            Ok(())
        }
    }

    // Equality of the ordinary Grid/Option carrier representation only. This
    // neither matches capability variants nor converts them into source proofs.
    fn same_plain(
        a: &SemanticValueBindingV1,
        b: &SemanticValueBindingV1,
        graph: &mut CapabilitySsaGraphV1<'_>,
        depth: usize,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        graph.charge(1)?;
        if depth > 32 {
            return Err(mismatch());
        }
        Ok(match (a, b) {
            (SemanticValueBindingV1::Unit, SemanticValueBindingV1::Unit) => true,
            (
                SemanticValueBindingV1::Value { id: a, ty: at },
                SemanticValueBindingV1::Value { id: b, ty: bt },
            ) => a == b && at == bt,
            (SemanticValueBindingV1::Aggregate(a), SemanticValueBindingV1::Aggregate(b))
                if a.len() == b.len() =>
            {
                for (a, b) in a.iter().zip(b) {
                    if !same_plain(a, b, graph, depth + 1)? {
                        return Ok(false);
                    }
                }
                true
            }
            (
                SemanticValueBindingV1::Enum {
                    discriminant: a,
                    discriminant_ty: at,
                    semantic_type: ast,
                    variant: av,
                    payloads: ap,
                },
                SemanticValueBindingV1::Enum {
                    discriminant: b,
                    discriminant_ty: bt,
                    semantic_type: bst,
                    variant: bv,
                    payloads: bp,
                },
            ) if a == b && at == bt && ast == bst && av == bv && ap.len() == bp.len() => {
                for ((ak, a), (bk, b)) in ap.iter().zip(bp) {
                    graph.charge(1)?;
                    if ak != bk || a.len() != b.len() {
                        return Ok(false);
                    }
                    for (a, b) in a.iter().zip(b) {
                        if !same_plain(a, b, graph, depth + 1)? {
                            return Ok(false);
                        }
                    }
                }
                true
            }
            _ => false,
        })
    }
    fn zero_plain(
        value: &SemanticValueBindingV1,
        graph: &mut CapabilitySsaGraphV1<'_>,
        depth: usize,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        graph.charge(1)?;
        if depth > 32 {
            return Err(mismatch());
        }
        match value {
            SemanticValueBindingV1::Unit => Ok(true),
            SemanticValueBindingV1::Aggregate(fields) => {
                for field in fields {
                    if !zero_plain(field, graph, depth + 1)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn precharge_leader_representation(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        graph: &mut CapabilitySsaGraphV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        // A representation check under an exact action, never an issuer proof.
        // Check lengths before visiting fields or cloning layout/type storage.
        graph.charge(16)?;
        let root = types.get(ty.index() as usize).ok_or_else(mismatch)?;
        let flat_zst = |decl: &SemanticTypeDeclV1| {
            decl.layout().size_bytes() == Some(0)
                && decl.layout().alignment_bytes() == 1
                && !decl.layout().is_uninhabited()
        };
        let SemanticTypeShapeV1::Aggregate(fields) = root.shape() else {
            return Err(mismatch());
        };
        let SemanticTypeLayoutDetailsV1::Aggregate(layout) = root.layout().details() else {
            return Err(mismatch());
        };
        if !flat_zst(root)
            || fields.fields().len() != 3
            || layout.field_offsets() != [0, 0, 0]
            || !layout.padding().is_empty()
        {
            return Err(mismatch());
        }
        for field in fields.fields() {
            let decl = types.get(field.index() as usize).ok_or_else(mismatch)?;
            if !flat_zst(decl) {
                return Err(mismatch());
            }
            match (decl.shape(), decl.layout().details()) {
                (SemanticTypeShapeV1::Unit, SemanticTypeLayoutDetailsV1::None) => {}
                (
                    SemanticTypeShapeV1::Aggregate(fields),
                    SemanticTypeLayoutDetailsV1::Aggregate(layout),
                ) if fields.fields().is_empty()
                    && layout.field_offsets().is_empty()
                    && layout.padding().is_empty() => {}
                _ => return Err(mismatch()),
            }
        }
        // Four constant-lowering visits, bounded declaration/layout clones, and
        // three simultaneous copies of the three-marker binding payload.
        let binding_words =
            std::mem::size_of::<SemanticValueBindingV1>().div_ceil(std::mem::size_of::<usize>());
        graph.charge(4 + 64 + 9 * binding_words)
    }

    impl SemanticFunctionLoweringV1<'_> {
        pub(super) fn prepare_guarded_grid_statement(
            &mut self,
            block: SemanticBlockIdV1,
            statement: Option<u32>,
            operations: &mut Vec<Operation>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            let Some(statement) = statement else {
                return Ok(());
            };
            let Some(mut plan) = self.guarded_grid.take() else {
                return Ok(());
            };
            let result = self.prepare_guarded_grid_statement_inner(
                &mut plan,
                (block, statement),
                operations,
            );
            self.guarded_grid = Some(plan);
            result
        }
        fn prepare_guarded_grid_statement_inner(
            &mut self,
            plan: &mut GuardedGridKirPlanV1<'_>,
            site: Site,
            operations: &mut Vec<Operation>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            let Some(index) = plan.action(site)? else {
                return Ok(());
            };
            let (action, done) = &plan.actions[index];
            if *done || !self.is_kernel_entry {
                return Err(mismatch());
            }
            let (source_local, source_value) = action.source();
            let actual = self
                .locals
                .get(source_local.index() as usize)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            let recorded = self
                .semantic_ssa_bindings
                .get(&source_value)
                .ok_or_else(mismatch)?;
            if !same_plain(actual, recorded, &mut plan.graph, 0)? {
                return Err(mismatch());
            }
            if action.kind() == ActionKind::Borrow
                && !self.enum_variant_is_available_v1(action.receipt().option(), 1, site.0)
            {
                return Err(mismatch());
            }
            let output = action.output();
            if let Some((local, value)) = action.created() {
                let ty = action.receipt().contract().types().leader;
                if self
                    .function
                    .locals()
                    .get(local.index() as usize)
                    .is_none_or(|l| l.ty() != ty)
                    || !self
                        .control_flow_ssa
                        .ssa_value_locals
                        .contains(&local.index())
                    || self
                        .pending_semantic_ssa_definitions
                        .get(&(site.0.index(), local.index()))
                        .and_then(VecDeque::front)
                        != Some(&value)
                    || self.semantic_ssa_bindings.contains_key(&value)
                {
                    return Err(mismatch());
                }
                // Only this exact retained Define may receive the closed
                // leader representation. It is not a general ZST initializer.
                precharge_leader_representation(self.types, ty, &mut plan.graph)?;
                let mut structural_nodes = 0;
                let binding = self.lower_constant_bytes(
                    site.0,
                    Some(site.1),
                    ty,
                    &[],
                    0,
                    &mut structural_nodes,
                    operations,
                )?;
                if structural_nodes != 4 || !zero_plain(&binding, &mut plan.graph, 0)? {
                    return Err(mismatch());
                }
                if self
                    .pending_semantic_ssa_definitions
                    .get_mut(&(site.0.index(), local.index()))
                    .and_then(VecDeque::pop_front)
                    != Some(value)
                {
                    return Err(mismatch());
                }
                self.locals[local.index() as usize] = Some(binding.clone());
                insert_semantic_ssa_binding_v1(
                    &mut self.semantic_ssa_bindings,
                    self.semantic_function.index(),
                    Some(site.0.index()),
                    Some(site.1),
                    value,
                    binding,
                )?;
            }
            if self
                .pending_semantic_ssa_definitions
                .get(&(site.0.index(), output.0.index()))
                .and_then(VecDeque::front)
                != Some(&output.1)
            {
                return Err(mismatch());
            }
            if action.kind() == ActionKind::Some {
                if !zero_plain(
                    self.semantic_ssa_bindings
                        .get(&source_value)
                        .ok_or_else(mismatch)?,
                    &mut plan.graph,
                    0,
                )? {
                    return Err(mismatch());
                }
                self.locals[source_local.index() as usize] = None;
                self.retained_local_initialized
                    .remove(&source_local.index());
            }
            plan.actions[index].1 = true;
            // Original assignment is still lowered by the ordinary dispatch.
            Ok(())
        }

        pub(super) fn guarded_grid_store_witness(
            &mut self,
            block: SemanticBlockIdV1,
            binding: &SemanticValueBindingV1,
            witness: SemanticTypeIdV1,
            operations: &mut Vec<Operation>,
        ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
            let Some(mut plan) = self.guarded_grid.take() else {
                return Ok(None);
            };
            let result = self
                .guarded_grid_store_witness_inner(&mut plan, block, binding, witness, operations);
            self.guarded_grid = Some(plan);
            result
        }
        fn guarded_grid_store_witness_inner(
            &mut self,
            plan: &mut GuardedGridKirPlanV1<'_>,
            block: SemanticBlockIdV1,
            binding: &SemanticValueBindingV1,
            witness: SemanticTypeIdV1,
            operations: &mut Vec<Operation>,
        ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
            plan.graph.charge(plan.stores.len())?;
            let mut matches = plan
                .stores
                .iter()
                .enumerate()
                .filter(|(_, (site, _, _))| *site == block);
            let Some((index, (_, proof, done))) = matches.next() else {
                return Ok(None);
            };
            if *done
                || matches.next().is_some()
                || proof.witness_type() != witness
                || !self.enum_variant_is_available_v1(proof.receipt().option(), 1, block)
            {
                return Err(mismatch());
            }
            let consumed = self
                .semantic_ssa_bindings
                .get(&proof.consumed().1)
                .ok_or_else(mismatch)?;
            if !same_plain(binding, consumed, &mut plan.graph, 0)? {
                return Err(mismatch());
            }
            let SemanticValueBindingV1::Aggregate(fields) = binding else {
                return Err(mismatch());
            };
            let [raw, markers @ ..] = fields.as_slice() else {
                return Err(mismatch());
            };
            if markers.len() != 3 {
                return Err(mismatch());
            }
            for marker in markers {
                if !zero_plain(marker, &mut plan.graph, 0)? {
                    return Err(mismatch());
                }
            }
            let original = self
                .semantic_ssa_bindings
                .get(&proof.raw_value())
                .ok_or_else(mismatch)?;
            if !same_plain(raw, original, &mut plan.graph, 0)? {
                return Err(mismatch());
            }
            let availability = SemanticCapabilityAvailabilityV1::EnumPayload {
                local: proof.receipt().option(),
                variant: 1,
            };
            let raw = raw.clone();
            let (id, ty) = self
                .coerce_index(block, operations, raw)?
                .value()
                .map_err(|_| mismatch())?;
            if ty != Type::INDEX {
                return Err(mismatch());
            }
            plan.stores[index].2 = true;
            Ok(Some(SemanticValueBindingV1::IndexWitness {
                id,
                index_space: SemanticDisjointIndexSpaceV1::GridExclusive,
                disjoint: true,
                availability: Some(availability),
            }))
        }
        pub(super) fn finish_guarded_grid(&mut self) -> Result<(), ProductionSemanticKirErrorV1> {
            if let Some(plan) = self.guarded_grid.as_mut() {
                plan.finish()?;
            }
            Ok(())
        }
    }

    #[cfg(test)]
    include!("guarded_grid_62_tests.rs");
}
use guarded_grid_kir_62::GuardedGridKirPlanV1;
