// Typed consumer selection only. All issuer values still come from the shared
// replayed SSA graph, never from this type-indexed transport registration.
pub(super) fn borrowed_allocation_pair(
    types: &[SemanticTypeDeclV1],
    operation: SemanticExecutionCapabilityOperationV1,
) -> Result<Option<(SemanticTypeIdV1, SemanticTypeIdV1)>> {
    let SemanticExecutionCapabilityOperationV1::LdsAllocate { workgroup, .. } = operation else {
        return Ok(None);
    };
    let shape = types
        .get(workgroup.index() as usize)
        .ok_or_else(|| reject("LDS allocation Workgroup source type is missing"))?
        .shape();
    let SemanticTypeShapeV1::Pointer(pointer) = shape else {
        // The existing owned-input KIR allocation contract keeps its old meaning.
        return Ok(None);
    };
    let owned = pointer.pointee();
    if !shared_type(types, workgroup, owned) || types.get(owned.index() as usize).is_none() {
        return Err(reject(
            "LDS allocation requires the exact shared Workgroup reference",
        ));
    }
    Ok(Some((workgroup, owned)))
}

fn workgroup_reference_pair(
    types: &[SemanticTypeDeclV1],
    contract: SemanticExecutionCapabilityContractV1,
) -> Result<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let pair = match contract.operation() {
        SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
            workgroup_reference,
            workgroup,
            ..
        }
        | SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
            workgroup_reference,
            workgroup,
            ..
        } => (workgroup_reference, workgroup),
        SemanticExecutionCapabilityOperationV1::LdsAllocate { .. } => {
            borrowed_allocation_pair(types, contract.operation())?
                .ok_or_else(|| reject("LDS shared receiver cannot use an owned source signature"))?
        }
        _ => return Err(reject("operation has no checked shared Workgroup receiver")),
    };
    if !shared_type(types, pair.0, pair.1) {
        return Err(reject(
            "shared Workgroup consumer changed its exact reference pair",
        ));
    }
    Ok(pair)
}

fn allocation_callable_pair(
    types: &[SemanticTypeDeclV1],
    callable: &SemanticCallableDeclV1,
) -> Result<Option<(SemanticExecutionCapabilityContractV1, SemanticTypeIdV1)>> {
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding,
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        ..
    } = callable
    else {
        return Ok(None);
    };
    let Some((reference, owned)) = borrowed_allocation_pair(types, contract.operation())? else {
        return Ok(None);
    };
    let abi = binding.abi();
    if binding.identity() != contract.source_identity()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.source_input_types() != [reference]
        || abi.source_output_type() != contract.signature().output()
        || abi.source_argument_ownership()
            != [fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1::SharedBorrow]
        || !contract.signature().arguments().eq([reference])
        || contract.workgroup_brand().is_none()
        || contract.epoch_before().is_none()
        || contract.epoch_after().is_some()
    {
        return Err(reject(
            "LDS allocation source callable, ABI, brand or epoch changed",
        ));
    }
    Ok(Some((*contract, owned)))
}

pub(super) fn register_allocation_workgroup_issuers(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    bindings: &mut BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
) -> Result<()> {
    let mut consumers = BTreeSet::new();
    for callable in callables {
        if let Some((contract, owned)) = allocation_callable_pair(types, callable)? {
            consumers.insert((
                owned,
                contract.provenance(),
                contract.workgroup_brand(),
                contract.epoch_before(),
            ));
        }
    }
    if consumers.is_empty() {
        return Ok(());
    }
    // Bounded by the already admitted callable roster. Two linear traversals
    // and ordered-set lookups avoid an additional issuer/consumer cross product.
    for callable in callables {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = callable
        else {
            continue;
        };
        let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { workgroup, .. } =
            contract.operation()
        else {
            continue;
        };
        if !consumers.contains(&(
            workgroup,
            contract.provenance(),
            contract.workgroup_brand(),
            contract.epoch_before(),
        )) {
            continue;
        }
        if binding.identity() != contract.source_identity()
            || contract.signature().output() != workgroup
            || binding.abi().source_output_type() != workgroup
            || binding
                .abi()
                .source_input_types()
                .iter()
                .copied()
                .ne(contract.signature().arguments())
            || contract.epoch_after().is_some()
        {
            return Err(reject(
                "LDS allocation Workgroup issuer lost its source identity or ABI",
            ));
        }
        insert_compiler_issued_ssa_binding_v1(
            bindings,
            workgroup,
            SemanticPromotedBindingV1::SubgroupPartitionAuthority {
                contract: *contract,
                source_type: execution_type_identity_v1(types, workgroup)?,
            },
        )?;
    }
    Ok(())
}
