// Inert KIR test data only, never a source, machine, artifact or launch receipt.
#[allow(dead_code)]
mod base {
    use super::*;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-kernel-ir/src/execution_capability_v1/subgroup_partition/fixture.rs"
    ));
    pub(super) fn build() -> Module {
        module()
    }
}

pub(super) fn module() -> Module {
    use ExecutionCapabilityOperationV1 as E;
    use ExecutionCapabilityRoleV1 as R;
    use SubgroupPartitionOperationV1 as P;
    let mut module = base::build();
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let OperationKind::ExecutionCapability(subgroup) = &mut operations[2].kind else {
        unreachable!()
    };
    let E::SubgroupDerive {
        workgroup,
        subgroup: output,
        width,
    } = subgroup.operation
    else {
        unreachable!()
    };
    let reference = ExecutionTypeIdentityV1::new([71; 32]);
    subgroup.operation = E::SubgroupDeriveBorrowed {
        workgroup_reference: reference,
        workgroup,
        subgroup: output,
        width,
    };
    subgroup.signature = ExecutionCapabilitySignatureV1::new(&[reference], output).unwrap();
    subgroup.obligations = ExecutionSafetyObligationsV1::from_bits(
        required_execution_obligations_v1(&subgroup.operation),
    );
    let Type::ExecutionCapability(ty) = &mut operations[2].results[0].ty else {
        unreachable!()
    };
    ty.role = R::BorrowedSubgroup {
        workgroup_reference: reference,
        workgroup,
        width,
    };
    let OperationKind::ExecutionCapability(reduction) = &mut operations[6].kind else {
        unreachable!()
    };
    let E::SubgroupPartition(P::ReduceSumF32 {
        partition_reference,
        partition,
        element,
        width,
        partition_width,
    }) = reduction.operation
    else {
        unreachable!()
    };
    reduction.operation = E::SubgroupPartition(P::ReduceMaxF32 {
        partition_reference,
        partition,
        element,
        width,
        partition_width,
    });
    reduction.obligations = ExecutionSafetyObligationsV1::from_bits(
        required_execution_obligations_v1(&reduction.operation),
    );
    module
}

pub(super) fn legacy_module() -> Module {
    base::build()
}

pub(super) fn reduction_mut(module: &mut Module) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[6].kind
    else {
        unreachable!()
    };
    contract
}
