use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1;

#[cfg(test)]
mod workgroup_index_transport_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;
    include!("workgroup_index_transport_01/tests.rs");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WorkgroupIndexTransportV1 {
    semantic: SemanticTypeIdV1,
    source: ExecutionTypeIdentityV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    brand: SemanticTypeIdentityV1,
    epoch: SemanticTypeIdentityV1,
}

impl WorkgroupIndexTransportV1 {
    fn register(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        bindings: &mut BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        for callable in callables {
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } = callable
            else {
                continue;
            };
            let witness = match contract.operation() {
                SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
                    witness, ..
                } => witness,
                SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint {
                    output_witness,
                    ..
                } => output_witness,
                _ => continue,
            };
            if binding.identity() != contract.source_identity() || contract.epoch_after().is_some()
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let transport = Self {
                semantic: witness,
                source: execution_type_identity_v1(types, witness)?,
                provenance: contract.provenance(),
                brand: contract
                    .workgroup_brand()
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                epoch: contract
                    .epoch_before()
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
            };
            insert_compiler_issued_ssa_binding_v1(
                bindings,
                witness,
                SemanticPromotedBindingV1::WorkgroupIndex(transport),
            )?;
        }
        Ok(())
    }

    fn types(
        self,
        types: &[SemanticTypeDeclV1],
        semantic: SemanticTypeIdV1,
        context: Option<&KernelContextTypeV1>,
    ) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
        let context = context.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let p = self.provenance;
        if semantic != self.semantic
            || execution_type_identity_v1(types, semantic)? != self.source
            || context.kernel_marker() != p.kernel_marker().as_bytes()
            || context.target() != p.target_brand().as_bytes()
            || context.launch() != p.launch_brand().as_bytes()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(vec![Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
            source_type: self.source,
            provenance: ExecutionCapabilityProvenanceV1 {
                root: context.root().clone(),
                kernel_binding: *p.kernel_binding().as_bytes(),
                frontend_unit: *p.frontend_unit().as_bytes(),
                kernel_marker: *p.kernel_marker().as_bytes(),
                target_brand: *p.target_brand().as_bytes(),
                launch_brand: *p.launch_brand().as_bytes(),
                issuance: *p.issuance().as_bytes(),
            },
            workgroup_brand: Some(*self.brand.as_bytes()),
            epoch: Some(*self.epoch.as_bytes()),
            role: ExecutionCapabilityRoleV1::WorkgroupMemoryIndex,
        })])
    }

    fn values(
        self,
        binding: &SemanticValueBindingV1,
    ) -> Result<Vec<(ValueId, Type)>, &'static str> {
        let SemanticValueBindingV1::Value {
            id,
            ty: Type::ExecutionCapability(capability),
        } = binding
        else {
            return Err("scoped index transport lost its logical witness");
        };
        let p = self.provenance;
        if !capability.is_complete()
            || capability.source_type != self.source
            || capability.role != ExecutionCapabilityRoleV1::WorkgroupMemoryIndex
            || capability.workgroup_brand != Some(*self.brand.as_bytes())
            || capability.epoch != Some(*self.epoch.as_bytes())
            || capability.provenance.kernel_binding != *p.kernel_binding().as_bytes()
            || capability.provenance.frontend_unit != *p.frontend_unit().as_bytes()
            || capability.provenance.kernel_marker != *p.kernel_marker().as_bytes()
            || capability.provenance.target_brand != *p.target_brand().as_bytes()
            || capability.provenance.launch_brand != *p.launch_brand().as_bytes()
            || capability.provenance.issuance != *p.issuance().as_bytes()
        {
            return Err("scoped index transport changed witness, root provenance, brand or epoch");
        }
        Ok(vec![(*id, Type::ExecutionCapability(capability.clone()))])
    }

    fn restore(
        self,
        types: &[SemanticTypeDeclV1],
        semantic: SemanticTypeIdV1,
        values: &[ValueDef],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let [value] = values else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let Type::ExecutionCapability(capability) = &value.ty else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let p = &capability.provenance;
        let context = KernelContextTypeV1::new(
            p.root.clone(),
            p.kernel_marker,
            p.target_brand,
            p.launch_brand,
        );
        if self.types(types, semantic, Some(&context))? != [value.ty.clone()] {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let binding = SemanticValueBindingV1::Value {
            id: value.id,
            ty: value.ty.clone(),
        };
        self.values(&binding)
            .map_err(|detail| unsupported(0, None, None, detail))?;
        Ok(binding)
    }
}

fn contains_workgroup_index_payload_v1(
    types: &[SemanticTypeDeclV1],
    root: SemanticTypeIdV1,
    bindings: &BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    // This is a storage classification, never an issuer lookup. Custody must
    // still be retained from one dominating source definition by the caller.
    let mut pending = vec![(root, 0usize)];
    let mut visited = BTreeSet::new();
    let mut work = 0usize;
    while let Some((ty, depth)) = pending.pop() {
        work = work
            .checked_add(1)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if depth > 64 || work > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(unsupported(
                0,
                None,
                None,
                "scoped index payload classification exceeds its structural limit",
            ));
        }
        if !visited.insert(ty) {
            continue;
        }
        if matches!(
            bindings.get(&ty),
            Some(SemanticPromotedBindingV1::WorkgroupIndex(_))
        ) {
            return Ok(true);
        }
        let declaration = types
            .get(ty.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let mut push = |field| -> Result<(), ProductionSemanticKirErrorV1> {
            if pending.len() >= MAX_SSA_VALUE_COMPONENTS_V1 {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "scoped index payload frontier exceeds its structural limit",
                ));
            }
            pending.push((field, depth + 1));
            Ok(())
        };
        match declaration.shape() {
            SemanticTypeShapeV1::Aggregate(fields) => {
                for field in fields.fields() {
                    push(*field)?;
                }
            }
            SemanticTypeShapeV1::Enum { variants, .. } => {
                for variant in variants {
                    for field in variant.fields().fields() {
                        push(*field)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(false)
}

fn exact_workgroup_index_enum_custody_v1(binding: &SemanticValueBindingV1) -> bool {
    fn visit(binding: &SemanticValueBindingV1, depth: usize, work: &mut usize) -> Option<bool> {
        *work = work.checked_add(1)?;
        if depth > 64 || *work > MAX_SSA_VALUE_COMPONENTS_V1 {
            return None;
        }
        match binding {
            SemanticValueBindingV1::Value {
                ty: Type::ExecutionCapability(capability),
                ..
            } if capability.role == ExecutionCapabilityRoleV1::WorkgroupMemoryIndex => {
                Some(capability.is_complete())
            }
            SemanticValueBindingV1::Enum {
                variant: Some(variant),
                payloads,
                ..
            } => {
                let mut found = false;
                for field in payloads.get(variant)? {
                    found |= visit(field, depth + 1, work)?;
                }
                Some(found)
            }
            SemanticValueBindingV1::Aggregate(fields) => {
                let mut found = false;
                for field in fields {
                    found |= visit(field, depth + 1, work)?;
                }
                Some(found)
            }
            SemanticValueBindingV1::Unit
            | SemanticValueBindingV1::Value {
                ty: Type::Scalar(_),
                ..
            } => Some(false),
            _ => None,
        }
    }
    matches!(
        binding,
        SemanticValueBindingV1::Enum {
            variant: Some(_),
            ..
        }
    ) && visit(binding, 0, &mut 0) == Some(true)
}
