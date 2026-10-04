//! Synthetic codec data for filesystem safety tests, never source authentication.

use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;
use fe2o3_source_isa_observation::multilevel_authoring_v1::{
    AuthoringOperationCoordinateV1, AuthoringRegionSelectorV1, AuthoringSnapshotV1,
};
use sha2::{Digest, Sha256};

pub const ORIGINAL: &[u8] = b"fn baseline(a:u32,b:u32)->u32 {a^b}\n";

pub fn fixture() -> (Vec<u8>, String) {
    let mut module = Module::new("candidate-synthetic-no-source-authentication");
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "v_xor_b32".into(),
            operands: vec![
                AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
            ],
            options: BTreeSet::from([AssemblyOption::NoMemory]),
            declared_effects: BTreeSet::new(),
        }),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 2],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    let canonical = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
    let identity = *canonical.identity();
    let prepared = PreparedSimulationBundleV6::new(
        SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
        SimulationProductionKirIdentityV6::new(11, *identity.digest(), identity.canonical_length())
            .unwrap(),
        "gfx942:xnack-",
        canonical,
    )
    .unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([5; 32], 128, "inert/not-opened.rs".into()).unwrap()],
        vec![
            DebugSourceMapSiteV1::new(
                DebugSourceMapKirSiteV1::operation(1, 0, 0),
                vec![DebugSourceMapSpanV1::new([5; 32], 3, 11, 1, 4).unwrap()],
            )
            .unwrap(),
        ],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let semantic = b"synthetic-candidate-fixture-no-source-authentication".to_vec();
    let storage = SemanticStorageMapV6::new(
        *prepared.subject_identity(),
        1,
        Sha256::digest(&semantic).into(),
        semantic.len() as u64,
        [9; 32],
        *identity.digest(),
        identity.canonical_length(),
        vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV6::new(
        *prepared.subject_identity(),
        *identity.digest(),
        identity.canonical_length(),
        vec![SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![])],
    )
    .unwrap();
    let bundle = prepared
        .finalize(source_map, semantic, storage, aggregate)
        .unwrap();
    let bytes = bundle.canonical_bytes().to_vec();
    let summary = AuthoringSnapshotV1::from_bundle_v6(bundle)
        .unwrap()
        .summary();
    let selector = AuthoringRegionSelectorV1 {
        bundle_identity: summary.bundle_identity,
        canonical_kir_digest: summary.canonical_kir_digest,
        target: summary.target,
        operations: vec![AuthoringOperationCoordinateV1 {
            function: 1,
            block: 0,
            operation: 0,
        }],
    };
    (bytes, serde_json::to_string(&selector).unwrap())
}
