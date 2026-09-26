// Exact original evaluation sites supply schema demands only. A fixed-site row
// is not a dynamic snapshot, a storage cell, or an initialization/lifetime fact.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SourceReferenceBoundaryRoleV29 {
    Argument(u32),
    Return,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceBoundaryValueV29 {
    site: SourceReferenceSiteV29,
    role: SourceReferenceBoundaryRoleV29,
    node: usize,
}

type SourceReferenceBoundaryKeyV29 = (usize, u32, Option<usize>, SourceReferenceBoundaryRoleV29);

fn source_reference_boundary_key_v29(site: SourceReferenceSiteV29, role: SourceReferenceBoundaryRoleV29) -> SourceReferenceBoundaryKeyV29 {
    (site.instance.index(), site.block.index(), site.statement, role)
}

fn source_reference_boundary_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("source boundary differs from its original evaluated value")
}

impl SourceReferencePlanV29<'_, '_> {
    fn boundary_source_type(
        &self,
        site: SourceReferenceSiteV29,
        role: SourceReferenceBoundaryRoleV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SemanticTypeIdV1, ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        self.charge(9, budget)?;
        if site.statement.is_some() || self.instances.instance_reachable(site.instance) != Some(true)
            || self.instances.block_reachable(site.instance, site.block) != Some(true)
        { return Err(source_reference_boundary_error_v29()); }
        let declaration = self.instances.instance(site.instance)
            .ok_or_else(source_reference_boundary_error_v29)?.declaration();
        let block = declaration.blocks().get(site.block.index() as usize)
            .ok_or_else(source_reference_boundary_error_v29)?;
        match (block.terminator().kind(), role) {
            (SemanticTerminatorKindV1::Call(call), SourceReferenceBoundaryRoleV29::Argument(ordinal)) => {
                call.arguments().get(ordinal as usize).map(|operand| operand.ty())
                    .ok_or_else(source_reference_boundary_error_v29)
            }
            (SemanticTerminatorKindV1::Return, SourceReferenceBoundaryRoleV29::Return) => Ok(declaration.abi().source_output_type()),
            _ => Err(source_reference_boundary_error_v29()),
        }
    }

    fn check_boundary_row(
        &self,
        row: SourceReferenceBoundaryValueV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let ty = self.boundary_source_type(row.site, row.role, budget)?;
        self.charge(3, budget)?;
        let node = self.nodes.get(row.node).ok_or_else(source_reference_boundary_error_v29)?;
        if node.ty != ty || matches!(node.kind, SourceReferenceNodeKindV29::Absent) {
            return Err(source_reference_boundary_error_v29());
        }
        Ok(())
    }

    fn boundary_value(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceBoundaryValueV29, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.check_owner(self.instances, budget)?;
            self.charge(2, budget)?;
            let row = *self.boundary_values.get(ordinal).ok_or_else(source_reference_boundary_error_v29)?;
            self.check_boundary_row(row, budget)?;
            charge_execution_cfg_lookup_v29(self.boundary_sites.len(), budget)?;
            if self.boundary_sites.get(&source_reference_boundary_key_v29(row.site, row.role)) != Some(&ordinal) {
                return Err(source_reference_boundary_error_v29());
            }
            Ok(row)
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }

    fn boundary_at(
        &self,
        site: SourceReferenceSiteV29,
        role: SourceReferenceBoundaryRoleV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<(usize, SourceReferenceBoundaryValueV29)>, ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.boundary_source_type(site, role, budget)?;
            charge_execution_cfg_lookup_v29(self.boundary_sites.len(), budget)?;
            let Some(&ordinal) = self.boundary_sites.get(&source_reference_boundary_key_v29(site, role)) else { return Ok(None); };
            let row = self.boundary_value(ordinal, budget)?;
            if row.site != site || row.role != role { return Err(source_reference_boundary_error_v29()); }
            Ok(Some((ordinal, row)))
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(self, error))
    }
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn retain_boundary_value(
        &mut self,
        site: SourceReferenceSiteV29,
        role: SourceReferenceBoundaryRoleV29,
        node: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.plan.check_owner(self.plan.instances, budget)?;
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<SourceReferenceBoundaryValueV29>(),
                std::mem::size_of::<SourceReferenceBoundaryKeyV29>(),
                argument_product_v1(2, std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>())?,
            ])?)?;
            self.plan.charge(2, budget)?;
            if self.effect_site != Some(site) { return Err(source_reference_boundary_error_v29()); }
            let row = SourceReferenceBoundaryValueV29 { site, role, node };
            self.plan.check_boundary_row(row, budget)?;
            let key = source_reference_boundary_key_v29(site, role);
            charge_execution_cfg_lookup_v29(self.plan.boundary_sites.len(), budget)?;
            if let Some(&ordinal) = self.plan.boundary_sites.get(&key) {
                let original = self.plan.boundary_value(ordinal, budget)?;
                self.plan.charge(2, budget)?;
                if original.site != site || original.role != role { return Err(source_reference_boundary_error_v29()); }
                if original.node == node { return Ok(()); }
                let joined = self.merge_node(original.node, node, budget)?;
                self.plan.check_boundary_row(SourceReferenceBoundaryValueV29 { node: joined, ..row }, budget)?;
                self.plan.charge(1, budget)?;
                self.plan.boundary_values[ordinal].node = joined;
                return Ok(());
            }
            reserve_execution_cfg_map_entry_v29::<SourceReferenceBoundaryKeyV29, usize>(self.plan.boundary_sites.len(), budget)?;
            let ordinal = self.plan.boundary_values.len();
            // emission_push pays/reserves before publication. The index was
            // already prepaid; no fallible action follows the row append.
            emission_push_v1(&mut self.plan.boundary_values, row, budget)?;
            self.plan.boundary_sites.insert(key, ordinal);
            Ok(())
        })();
        result.inspect_err(|error| source_reference_record_failure_v29(&self.plan, error))
    }
}
