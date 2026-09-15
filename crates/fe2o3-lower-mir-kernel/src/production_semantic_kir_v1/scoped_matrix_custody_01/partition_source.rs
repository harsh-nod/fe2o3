// Uses the existing reference/aggregate/incoming-edge resolver. This method
// handles only the additional closed source terminal, not a second SSA walk.
include!("partition_geometry.rs");

impl Resolver<'_, '_> {
    fn partition_source(
        &mut self,
        value: SsaValueV1,
        site: Site,
        path: &[SemanticProjectionV1],
        target: SemanticTypeIdV1,
        depth: usize,
    ) -> Result<Origin> {
        let Requirement::Transpose(issue) = self.requirement else {
            return Err(reject(
                "partition source custody requires an actual transpose Issue",
            ));
        };
        let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose) = issue.operation()
        else {
            return Err(mismatch());
        };
        let (call, contract) = self.execution(site.block)?;
        let SemanticExecutionCapabilityOperationV1::SubgroupPartition(
            fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1::Derive {
                subgroup_reference,
                epoch,
                ..
            },
        ) = contract.operation()
        else {
            return Err(reject(
                "transpose partition has no original partition derive call",
            ));
        };
        let SemanticExecutionCapabilityOperationV1::SubgroupPartition(derive) = contract.operation() else {
            return Err(mismatch());
        };
        if !transpose_partition_source_types_match(self.types(), transpose, derive, path, target) {
            return Err(reject(
                "transpose partition changed its exact Wave64/Wave16 source geometry or types",
            ));
        }
        let [subgroup_arg, epoch_arg] = call.arguments() else {
            return Err(mismatch());
        };
        if subgroup_arg.ty() != subgroup_reference || epoch_arg.ty() != epoch {
            return Err(mismatch());
        }
        let mut result = self.operand_path(
            site.block,
            subgroup_arg,
            &[],
            subgroup_reference,
            Role::Subgroup,
            depth + 1,
        )?;
        let subgroup_site = self
            .graph
            .definition(result.subgroup.ok_or_else(mismatch)?)?;
        let (_, subgroup_contract) = self.execution(subgroup_site.block)?;
        let epoch_origin = self.workgroups().resolve_operand(
            site.block,
            epoch_arg,
            &[],
            subgroup_contract,
            depth + 1,
        )?;
        self.workgroups().check_live(&epoch_origin, site.block)?;
        if !epoch_origin.epoch_projection
            || epoch_origin.issuer != result.workgroup.as_ref().ok_or_else(mismatch)?.issuer
        {
            return Err(reject(
                "transpose partition epoch changed its Workgroup SSA issuer",
            ));
        }
        self.graph.charge(epoch_origin.loans.len())?;
        result.loans.extend(epoch_origin.loans);
        self.live(&result, site)?;
        result.issuer = value;
        Ok(result)
    }
}
