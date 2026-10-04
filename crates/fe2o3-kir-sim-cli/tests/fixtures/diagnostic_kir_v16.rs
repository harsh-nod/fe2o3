//! Shared test-only canonical-owner fixture, based on the existing CPU V16
//! fixture. Synthetic source IDs are declarations, NOT source-produced evidence.
//! Actual source qualification must use the release-active exporter separately.
#![allow(dead_code)]

use fe2o3_kernel_ir::*;

pub fn module(used: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let region = Gfx942OrderedRegionV1::new(
        AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
        Gfx942OrderedRegionRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
    )
    .unwrap();
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = vec![
        Operation::new(
            vec![ValueDef::new(ValueId(4), scalar.clone())],
            OperationKind::Gfx942OrderedRegion(region),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), pointer.clone()),
            OperationKind::GetElementPointer {
                base: ValueId(3),
                offset: ValueId(5),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(6),
                value: ValueId(if used { 4 } else { 0 }),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "region_entry",
        Signature::new(
            vec![scalar.clone(), scalar.clone(), scalar, pointer],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    );
    function.required_capabilities = function.derived_capabilities();
    let mut kernel = Kernel::new(
        "region",
        "region_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("diagnostic-cpu-ordered-region-v16");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

pub fn owner(module: &Module) -> VerifiedCanonicalKernelIrModuleV16 {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV16::from_module_ref_with_verification_budget_v16(
            module,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    owner
}

pub fn request(inputs: [u32; 3]) -> Vec<u8> {
    let mut arguments: Vec<_> = inputs
        .into_iter()
        .map(|value| {
            serde_json::json!({
                "kind": "scalar", "type": "u32", "bits": format!("0x{value:08x}"),
            })
        })
        .collect();
    arguments.push(serde_json::json!({
        "kind": "buffer", "element": "u32", "access": "read_write", "alignment": 4,
        "bytes": format!("0x{}", "5a".repeat(64 * 4 + 8)),
    }));
    serde_json::to_vec(&serde_json::json!({
        "schema": "fe2o3-simulation-request-v1", "kernel": "region",
        "grid": [64, 1, 1], "workgroup": [64, 1, 1], "arguments": arguments,
    }))
    .unwrap()
}

pub fn expected_output(inputs: [u32; 3], used: bool) -> Vec<u8> {
    let value = if used {
        (inputs[0] ^ inputs[1]).wrapping_add(inputs[2])
    } else {
        inputs[0]
    };
    let mut output = value.to_le_bytes().repeat(64);
    output.extend_from_slice(&[0x5a; 8]);
    output
}
