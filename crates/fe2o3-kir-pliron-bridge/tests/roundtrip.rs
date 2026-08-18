use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
    Terminator,
};
use fe2o3_kir_pliron_bridge::{
    BRIDGE_MODULE_SYMBOL, BridgeLimits, CanonicalKirRecord, KirVersion, recover_canonical,
    recover_exact,
};
use pliron::{
    builtin::op_interfaces::{SingleBlockRegionInterface, SymbolOpInterface},
    context::Context,
    linked_list::ContainsLinkedList,
};

fn kernel_module(identity: &str, rank: u8) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new(identity);
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let domain = match rank {
        1 => LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
        2 => LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Static(7),
        },
        3 => LaunchDomain::D3 {
            x: LaunchExtent::Static(3),
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Static(5),
        },
        _ => panic!("test rank must be 1, 2, or 3"),
    };
    module.kernels.push(Kernel::new("kernel", "entry", domain));
    module
}

#[test]
fn every_frozen_version_round_trips_exact_bytes_and_discriminant() {
    let limits = BridgeLimits::default();
    let versions = [
        KirVersion::V1,
        KirVersion::V2,
        KirVersion::V3,
        KirVersion::V4,
        KirVersion::V5,
    ];

    for (index, version) in versions.into_iter().enumerate() {
        let rank = (index % 3 + 1) as u8;
        let identity = format!("canonical KIR identity::version/{}", version.wire_value());
        let record =
            CanonicalKirRecord::from_module(&kernel_module(&identity, rank), version, limits)
                .expect("valid KIR must bridge");
        assert_eq!(
            &record.canonical_bytes()[8..10],
            &version.wire_value().to_le_bytes()
        );

        let mut context = Context::new();
        let shell = record
            .project_to_pliron(&mut context, limits)
            .expect("projection must succeed");
        assert_eq!(
            shell.get_symbol_name(&context).as_ref(),
            BRIDGE_MODULE_SYMBOL
        );
        assert_eq!(
            shell
                .get_body(&context, 0)
                .deref(&context)
                .iter(&context)
                .count(),
            2
        );

        let recovered =
            recover_canonical(&context, &shell, limits).expect("valid projection must recover");
        assert_eq!(recovered.version(), version);
        assert_eq!(recovered.module_identity(), identity);
        assert_eq!(recovered.canonical_bytes(), record.canonical_bytes());
        assert_eq!(recovered.module(), record.module());
    }
}

#[test]
fn zero_kernel_record_retains_non_pliron_identity_without_shell_operations() {
    let limits = BridgeLimits::default();
    let module = Module::new("KIR identity has spaces/slashes::and-is-not-a-symbol");
    let record = CanonicalKirRecord::from_module(&module, KirVersion::V5, limits).unwrap();
    let mut context = Context::new();
    let shell = record.project_to_pliron(&mut context, limits).unwrap();

    assert_eq!(
        shell.get_symbol_name(&context).as_ref(),
        BRIDGE_MODULE_SYMBOL
    );
    assert_eq!(
        shell
            .get_body(&context, 0)
            .deref(&context)
            .iter(&context)
            .count(),
        0
    );
    let recovered = recover_canonical(&context, &shell, limits).unwrap();
    assert_eq!(recovered.module_identity(), module.id.as_str());
    assert_eq!(recovered.canonical_bytes(), record.canonical_bytes());
}

#[test]
fn exact_recovery_binds_the_expected_record_not_only_a_self_consistent_shell() {
    let limits = BridgeLimits::default();
    let first = CanonicalKirRecord::from_module(&kernel_module("first", 1), KirVersion::V2, limits)
        .unwrap();
    let second =
        CanonicalKirRecord::from_module(&kernel_module("second", 1), KirVersion::V2, limits)
            .unwrap();
    let mut context = Context::new();
    let shell = first.project_to_pliron(&mut context, limits).unwrap();

    assert!(recover_exact(&context, &shell, &first, limits).is_ok());
    assert!(matches!(
        recover_exact(&context, &shell, &second, limits),
        Err(fe2o3_kir_pliron_bridge::BridgeError::RecordSubstitution)
    ));
}
