use fe2o3_rustc_front::{
    AuthenticatedOrdinaryRustScalarKernelImportV1, BasicBlockV1, BlockIdV1,
    CanonicalKernelInstIdV1, CanonicalKernelItemIdV1, ConcreteMonomorphizationIdentityV1,
    DirectCallObservationV1, FrontendLaunchBoundsV1, FrontendSourceSpanV1, FrontendUnitV1,
    FrontendUnsafeAssemblyDeclarationV1, FrontendUnsafeAssemblyTargetV1,
    FrontendWorkgroupDimensionsV1, FunctionIdentityV1, FunctionImportRoleV1, FunctionRoleV1,
    KernelFrontendContractV1, MonomorphizedFunctionV1, OrdinaryRustScalarDiagnosticCodeV1,
    OrdinaryRustScalarKernelObservationV1, OrdinaryRustScalarValidationErrorV1,
    ReachableFunctionObservationV1, RustItemDefinitionIdentityV1, RustcAbiPassModeV1,
    RustcAbiValueV1, RustcCallingConventionV1, RustcFnAbiFactsV1, RustcFunctionKindV1,
    RustcMirIdentityV1, RustcSourceIdentityV1, SourceFileIdentityV1, SourceLocationV1,
    StableTypeIdentityV1, TypedSignatureV1, UnsupportedRustBehaviorKindV1,
    UnsupportedRustBehaviorObservationV1, authenticate_ordinary_rust_scalar_kernel_v1,
};

const ITEM_BYTES: usize = 112;
const INSTANCE_BYTES: usize = 224;

fn function_id(seed: u8) -> FunctionIdentityV1 {
    FunctionIdentityV1::new([seed; 32]).unwrap()
}

fn type_id(seed: u8) -> StableTypeIdentityV1 {
    StableTypeIdentityV1::new([seed; 32]).unwrap()
}

fn source_location(line: u32) -> SourceLocationV1 {
    SourceLocationV1::new(SourceFileIdentityV1::new([0x91; 32]).unwrap(), line, 5).unwrap()
}

fn span(file: &str, line: u32) -> FrontendSourceSpanV1 {
    FrontendSourceSpanV1::new(file, line, 3, line, 19).unwrap()
}

fn kernel_item(crate_seed: u8, item_seed: u8, generic_seed: u8) -> CanonicalKernelItemIdV1 {
    let mut bytes = [0_u8; ITEM_BYTES];
    bytes[..8].copy_from_slice(b"F2KITEM1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&96_u32.to_le_bytes());
    bytes[16..48].fill(crate_seed);
    bytes[48..80].fill(item_seed);
    bytes[80..112].fill(generic_seed);
    CanonicalKernelItemIdV1::new(bytes).unwrap()
}

fn kernel_instance(
    item: CanonicalKernelItemIdV1,
    type_seed: u8,
    const_seed: u8,
    cfg_seed: u8,
) -> CanonicalKernelInstIdV1 {
    let mut bytes = [0_u8; INSTANCE_BYTES];
    bytes[..8].copy_from_slice(b"F2KINST1");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&208_u32.to_le_bytes());
    bytes[16..128].copy_from_slice(item.as_bytes());
    bytes[128..160].fill(type_seed);
    bytes[160..192].fill(const_seed);
    bytes[192..224].fill(cfg_seed);
    CanonicalKernelInstIdV1::new(bytes).unwrap()
}

fn abi_value(rust_type: StableTypeIdentityV1, seed: u8) -> RustcAbiValueV1 {
    RustcAbiValueV1::new(rust_type, [seed; 32], 4, 4, RustcAbiPassModeV1::Direct).unwrap()
}

fn fn_abi(
    identity_seed: u8,
    arguments: &[StableTypeIdentityV1],
    result: StableTypeIdentityV1,
) -> RustcFnAbiFactsV1 {
    RustcFnAbiFactsV1::new(
        [identity_seed; 32],
        RustcCallingConventionV1::Rust,
        arguments
            .iter()
            .enumerate()
            .map(|(index, rust_type)| abi_value(*rust_type, 0x60 + index as u8))
            .collect(),
        abi_value(result, 0x70),
        false,
        false,
    )
    .unwrap()
}

fn frontend_function(
    id: FunctionIdentityV1,
    role: FunctionRoleV1,
    name: &str,
    arguments: Vec<StableTypeIdentityV1>,
    result: StableTypeIdentityV1,
    line: u32,
) -> MonomorphizedFunctionV1 {
    MonomorphizedFunctionV1::new(
        id,
        role,
        name,
        source_location(line),
        TypedSignatureV1::new(arguments, result).unwrap(),
        BlockIdV1::new(0),
        vec![BasicBlockV1::new(BlockIdV1::new(0), source_location(line), vec![]).unwrap()],
    )
    .unwrap()
}

fn scalar_contract() -> KernelFrontendContractV1 {
    let one = FrontendWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    KernelFrontendContractV1::new(
        Some(FrontendLaunchBoundsV1::new(Some(one), Some(one), None).unwrap()),
        None,
    )
    .unwrap()
}

#[derive(Clone, Copy)]
struct FixtureMutation {
    type_seed: u8,
    source_seed: u8,
    mir_seed: u8,
    fn_abi_seed: u8,
    helper_item_seed: u8,
    call_line: u32,
}

impl Default for FixtureMutation {
    fn default() -> Self {
        Self {
            type_seed: 0x31,
            source_seed: 0x41,
            mir_seed: 0x51,
            fn_abi_seed: 0x61,
            helper_item_seed: 0x71,
            call_line: 12,
        }
    }
}

fn observation_with(
    mutation: FixtureMutation,
    root_kind: RustcFunctionKindV1,
    root_concrete: bool,
    root_abi_flags: (bool, bool),
    unsupported: Vec<UnsupportedRustBehaviorObservationV1>,
) -> OrdinaryRustScalarKernelObservationV1 {
    let f32_type = type_id(0x11);
    let item = kernel_item(0x21, 0x22, 0x23);
    let instance = kernel_instance(item, mutation.type_seed, 0x32, 0x33);
    let root_instance = ConcreteMonomorphizationIdentityV1::for_kernel_instance(instance);
    let helper_instance = ConcreteMonomorphizationIdentityV1::new([0x42; 32]).unwrap();
    let frontend = FrontendUnitV1::new(vec![
        frontend_function(
            function_id(0x81),
            FunctionRoleV1::Kernel,
            "app::scalar_add::<f32>",
            vec![f32_type, f32_type],
            f32_type,
            7,
        ),
        frontend_function(
            function_id(0x82),
            FunctionRoleV1::Helper,
            "math_support::add::<f32>",
            vec![f32_type, f32_type],
            f32_type,
            30,
        ),
    ])
    .unwrap();
    let root_abi = RustcFnAbiFactsV1::new(
        [mutation.fn_abi_seed; 32],
        RustcCallingConventionV1::Rust,
        vec![abi_value(f32_type, 0x61), abi_value(f32_type, 0x62)],
        abi_value(f32_type, 0x63),
        root_abi_flags.0,
        root_abi_flags.1,
    )
    .unwrap();
    let root = ReachableFunctionObservationV1::new(
        function_id(0x81),
        FunctionImportRoleV1::Kernel,
        item.rust_item_identity(),
        root_instance,
        RustcSourceIdentityV1::new([mutation.source_seed; 32]).unwrap(),
        RustcMirIdentityV1::new([mutation.mir_seed; 32]).unwrap(),
        span("app/src/lib.rs", 7),
        root_kind,
        root_concrete,
        root_abi,
        vec![DirectCallObservationV1::new(
            helper_instance,
            span("app/src/lib.rs", mutation.call_line),
        )],
    )
    .unwrap();
    let helper = ReachableFunctionObservationV1::new(
        function_id(0x82),
        FunctionImportRoleV1::Helper,
        RustItemDefinitionIdentityV1::new([mutation.helper_item_seed; 32]).unwrap(),
        helper_instance,
        RustcSourceIdentityV1::new([0x43; 32]).unwrap(),
        RustcMirIdentityV1::new([0x44; 32]).unwrap(),
        span("math-support/src/lib.rs", 30),
        RustcFunctionKindV1::OrdinaryItem,
        true,
        fn_abi(0x45, &[f32_type, f32_type], f32_type),
        vec![],
    )
    .unwrap();
    OrdinaryRustScalarKernelObservationV1::new(
        frontend,
        item,
        instance,
        scalar_contract(),
        vec![helper, root],
        unsupported,
    )
    .unwrap()
}

fn observation() -> OrdinaryRustScalarKernelObservationV1 {
    observation_with(
        FixtureMutation::default(),
        RustcFunctionKindV1::OrdinaryItem,
        true,
        (false, false),
        vec![],
    )
}

fn authenticate(
    observation: OrdinaryRustScalarKernelObservationV1,
) -> AuthenticatedOrdinaryRustScalarKernelImportV1 {
    authenticate_ordinary_rust_scalar_kernel_v1(observation).unwrap()
}

#[test]
fn authenticates_generic_root_and_cross_crate_reachable_helper() {
    let imported = authenticate(observation());
    assert_eq!(imported.functions().len(), 2);
    assert_eq!(imported.kernel_instance().item(), imported.kernel_item());
    assert_eq!(
        imported.root(),
        ConcreteMonomorphizationIdentityV1::for_kernel_instance(imported.kernel_instance())
    );
    let helper = imported
        .functions()
        .iter()
        .find(|function| function.role() == FunctionImportRoleV1::Helper)
        .unwrap();
    assert_eq!(helper.source_span().file(), "math-support/src/lib.rs");
    let chain = imported.call_chain_to(helper.monomorphization()).unwrap();
    assert_eq!(chain.len(), 2);
    assert_eq!(chain[0].function(), imported.root());
    assert_eq!(chain[1].call_site().unwrap().start(), (12, 3));
    assert!(!imported.grants_compiler_authority());
    assert!(!imported.grants_execution_authority());
}

#[test]
fn every_identity_and_source_axis_changes_the_aggregate_identity() {
    let baseline = authenticate(observation());
    let baseline_identity = *baseline.import_identity();
    let baseline_source = *baseline.source_closure_identity();
    let baseline_mir = *baseline.mir_closure_identity();

    for changed in [
        FixtureMutation {
            type_seed: 0x99,
            ..FixtureMutation::default()
        },
        FixtureMutation {
            fn_abi_seed: 0x99,
            ..FixtureMutation::default()
        },
        FixtureMutation {
            helper_item_seed: 0x99,
            ..FixtureMutation::default()
        },
        FixtureMutation {
            call_line: 13,
            ..FixtureMutation::default()
        },
    ] {
        assert_ne!(
            authenticate(observation_with(
                changed,
                RustcFunctionKindV1::OrdinaryItem,
                true,
                (false, false),
                vec![],
            ))
            .import_identity(),
            &baseline_identity
        );
    }

    let changed_source = authenticate(observation_with(
        FixtureMutation {
            source_seed: 0x99,
            ..FixtureMutation::default()
        },
        RustcFunctionKindV1::OrdinaryItem,
        true,
        (false, false),
        vec![],
    ));
    assert_ne!(changed_source.source_closure_identity(), &baseline_source);
    assert_ne!(changed_source.import_identity(), &baseline_identity);

    let changed_mir = authenticate(observation_with(
        FixtureMutation {
            mir_seed: 0x99,
            ..FixtureMutation::default()
        },
        RustcFunctionKindV1::OrdinaryItem,
        true,
        (false, false),
        vec![],
    ));
    assert_ne!(changed_mir.mir_closure_identity(), &baseline_mir);
    assert_ne!(changed_mir.import_identity(), &baseline_identity);
}

#[test]
fn canonical_identity_envelopes_reject_mutation_and_substitution() {
    let mut item = *kernel_item(0x21, 0x22, 0x23).as_bytes();
    item[0] ^= 1;
    assert_eq!(
        CanonicalKernelItemIdV1::new(item).unwrap_err().code(),
        OrdinaryRustScalarDiagnosticCodeV1::InvalidIdentityEnvelope
    );

    let good_item = kernel_item(0x21, 0x22, 0x23);
    let mut instance = *kernel_instance(good_item, 0x31, 0x32, 0x33).as_bytes();
    instance[10] = 1;
    assert_eq!(
        CanonicalKernelInstIdV1::new(instance).unwrap_err().code(),
        OrdinaryRustScalarDiagnosticCodeV1::InvalidIdentityEnvelope
    );

    let selected = kernel_item(0x21, 0x99, 0x23);
    let substituted = OrdinaryRustScalarKernelObservationV1::new(
        observation().clone_frontend_for_test(),
        selected,
        kernel_instance(good_item, 0x31, 0x32, 0x33),
        scalar_contract(),
        observation().clone_functions_for_test(),
        vec![],
    );
    assert!(substituted.is_ok());
    assert_eq!(
        authenticate_ordinary_rust_scalar_kernel_v1(substituted.unwrap())
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::KernelItemMismatch
    );

    let imported = authenticate(observation());
    let root = imported
        .functions()
        .iter()
        .find(|function| function.role() == FunctionImportRoleV1::Kernel)
        .unwrap();
    let wrong_root = ReachableFunctionObservationV1::new(
        root.frontend_identity(),
        root.role(),
        RustItemDefinitionIdentityV1::new([0xee; 32]).unwrap(),
        root.monomorphization(),
        root.source_identity(),
        root.mir_identity(),
        root.source_span().clone(),
        root.function_kind(),
        root.is_concrete(),
        root.fn_abi().clone(),
        root.calls().to_vec(),
    )
    .unwrap();
    let functions = imported
        .functions()
        .iter()
        .map(|function| {
            if function.role() == FunctionImportRoleV1::Kernel {
                wrong_root.clone()
            } else {
                function.clone()
            }
        })
        .collect();
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation_from_import(
        &imported,
        imported.frontend_unit().clone(),
        functions,
    ))
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::KernelRootIdentityMismatch
    );
}

#[test]
fn canonical_identity_component_constructors_match_frozen_envelopes() {
    let item =
        CanonicalKernelItemIdV1::from_components([0x21; 32], [0x22; 32], [0x23; 32]).unwrap();
    assert_eq!(item, kernel_item(0x21, 0x22, 0x23));

    let instance =
        CanonicalKernelInstIdV1::from_components(item, [0x31; 32], [0x32; 32], [0x33; 32]).unwrap();
    assert_eq!(instance, kernel_instance(item, 0x31, 0x32, 0x33));

    assert_eq!(
        CanonicalKernelItemIdV1::from_components([0x21; 32], [0; 32], [0x23; 32])
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::ZeroIdentity
    );
    assert_eq!(
        CanonicalKernelItemIdV1::from_components([0; 32], [0x22; 32], [0x23; 32])
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::ZeroIdentity
    );
    assert_eq!(
        CanonicalKernelItemIdV1::from_components([0x21; 32], [0x22; 32], [0; 32])
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::ZeroIdentity
    );
    assert_eq!(
        CanonicalKernelInstIdV1::from_components(item, [0x31; 32], [0; 32], [0x33; 32])
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::ZeroIdentity
    );
}

#[test]
fn typed_function_and_abi_admission_fails_with_stable_codes() {
    let nonordinary = observation_with(
        FixtureMutation::default(),
        RustcFunctionKindV1::Closure,
        true,
        (false, false),
        vec![],
    );
    let error = authenticate_ordinary_rust_scalar_kernel_v1(nonordinary).unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::NonOrdinaryFunction
    );
    assert!(error.to_string().starts_with("FE2O3-RUST-SCALAR-1201:"));

    let nonconcrete = observation_with(
        FixtureMutation::default(),
        RustcFunctionKindV1::OrdinaryItem,
        false,
        (false, false),
        vec![],
    );
    assert_eq!(
        authenticate_ordinary_rust_scalar_kernel_v1(nonconcrete)
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::NonConcreteMonomorphization
    );

    for flags in [(true, false), (false, true)] {
        let unsupported = observation_with(
            FixtureMutation::default(),
            RustcFunctionKindV1::OrdinaryItem,
            true,
            flags,
            vec![],
        );
        assert_eq!(
            authenticate_ordinary_rust_scalar_kernel_v1(unsupported)
                .unwrap_err()
                .code(),
            OrdinaryRustScalarDiagnosticCodeV1::UnsupportedFnAbi
        );
    }

    let imported = authenticate(observation());
    let root = imported
        .functions()
        .iter()
        .find(|function| function.role() == FunctionImportRoleV1::Kernel)
        .unwrap();
    let non_rust_abi = RustcFnAbiFactsV1::new(
        [0x61; 32],
        RustcCallingConventionV1::C,
        root.fn_abi().arguments().to_vec(),
        root.fn_abi().return_value().clone(),
        false,
        false,
    )
    .unwrap();
    let changed_root = rebuild_function(
        root,
        root.frontend_identity(),
        root.role(),
        root.function_kind(),
        root.is_concrete(),
        non_rust_abi,
        root.calls().to_vec(),
    );
    let functions = imported
        .functions()
        .iter()
        .map(|function| {
            if function.role() == FunctionImportRoleV1::Kernel {
                changed_root.clone()
            } else {
                function.clone()
            }
        })
        .collect();
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation_from_import(
        &imported,
        imported.frontend_unit().clone(),
        functions,
    ))
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::UnsupportedFnAbi
    );
}

#[test]
fn unsupported_behavior_reports_source_and_reachable_call_chain() {
    let helper = ConcreteMonomorphizationIdentityV1::new([0x42; 32]).unwrap();
    let unsupported = UnsupportedRustBehaviorObservationV1::new(
        helper,
        UnsupportedRustBehaviorKindV1::Allocation,
        span("math-support/src/lib.rs", 33),
    );
    let observation = observation_with(
        FixtureMutation::default(),
        RustcFunctionKindV1::OrdinaryItem,
        true,
        (false, false),
        vec![unsupported],
    );
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation).unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::UnsupportedBehavior
    );
    let OrdinaryRustScalarValidationErrorV1::UnsupportedBehavior {
        kind,
        source_span,
        call_chain,
    } = error
    else {
        panic!("expected typed unsupported behavior diagnostic");
    };
    assert_eq!(kind, UnsupportedRustBehaviorKindV1::Allocation);
    assert_eq!(source_span.start(), (33, 3));
    assert_eq!(call_chain.len(), 2);
    assert_eq!(call_chain[1].call_site().unwrap().start(), (12, 3));
}

#[test]
fn scalar_launch_rejects_wider_workgroups_occupancy_and_assembly() {
    let one = FrontendWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    let two = FrontendWorkgroupDimensionsV1::new([2, 1, 1]).unwrap();
    let contracts = [
        KernelFrontendContractV1::new(
            Some(FrontendLaunchBoundsV1::new(Some(one), Some(two), None).unwrap()),
            None,
        )
        .unwrap(),
        KernelFrontendContractV1::new(
            Some(FrontendLaunchBoundsV1::new(Some(one), Some(one), Some(1)).unwrap()),
            None,
        )
        .unwrap(),
        KernelFrontendContractV1::new(
            Some(FrontendLaunchBoundsV1::new(Some(one), Some(one), None).unwrap()),
            Some(
                FrontendUnsafeAssemblyDeclarationV1::new(
                    FrontendUnsafeAssemblyTargetV1::AmdGpuGfx942,
                    1,
                    1,
                    0,
                )
                .unwrap(),
            ),
        )
        .unwrap(),
    ];
    for contract in contracts {
        let base = observation();
        let rejected = OrdinaryRustScalarKernelObservationV1::new(
            base.clone_frontend_for_test(),
            base.kernel_item_for_test(),
            base.kernel_instance_for_test(),
            contract,
            base.clone_functions_for_test(),
            vec![],
        )
        .unwrap();
        assert_eq!(
            authenticate_ordinary_rust_scalar_kernel_v1(rejected)
                .unwrap_err()
                .code(),
            OrdinaryRustScalarDiagnosticCodeV1::UnsupportedLaunch
        );
    }
}

fn rebuild_function(
    function: &ReachableFunctionObservationV1,
    frontend_identity: FunctionIdentityV1,
    role: FunctionImportRoleV1,
    function_kind: RustcFunctionKindV1,
    is_concrete: bool,
    fn_abi: RustcFnAbiFactsV1,
    calls: Vec<DirectCallObservationV1>,
) -> ReachableFunctionObservationV1 {
    ReachableFunctionObservationV1::new(
        frontend_identity,
        role,
        function.item_identity(),
        function.monomorphization(),
        function.source_identity(),
        function.mir_identity(),
        function.source_span().clone(),
        function_kind,
        is_concrete,
        fn_abi,
        calls,
    )
    .unwrap()
}

fn observation_from_import(
    imported: &AuthenticatedOrdinaryRustScalarKernelImportV1,
    frontend: FrontendUnitV1,
    functions: Vec<ReachableFunctionObservationV1>,
) -> OrdinaryRustScalarKernelObservationV1 {
    OrdinaryRustScalarKernelObservationV1::new(
        frontend,
        imported.kernel_item(),
        imported.kernel_instance(),
        imported.kernel_contract(),
        functions,
        vec![],
    )
    .unwrap()
}

#[test]
fn call_graph_rejects_unknown_disconnected_and_recursive_helpers() {
    let imported = authenticate(observation());
    let frontend = imported.frontend_unit().clone();
    let root = imported
        .functions()
        .iter()
        .find(|function| function.role() == FunctionImportRoleV1::Kernel)
        .unwrap();
    let helper = imported
        .functions()
        .iter()
        .find(|function| function.role() == FunctionImportRoleV1::Helper)
        .unwrap();

    let unknown_root = rebuild_function(
        root,
        root.frontend_identity(),
        root.role(),
        root.function_kind(),
        root.is_concrete(),
        root.fn_abi().clone(),
        vec![DirectCallObservationV1::new(
            ConcreteMonomorphizationIdentityV1::new([0xaa; 32]).unwrap(),
            span("app/src/lib.rs", 12),
        )],
    );
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation_from_import(
        &imported,
        frontend.clone(),
        vec![unknown_root, helper.clone()],
    ))
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::UnknownCallee
    );

    let disconnected_root = rebuild_function(
        root,
        root.frontend_identity(),
        root.role(),
        root.function_kind(),
        root.is_concrete(),
        root.fn_abi().clone(),
        vec![],
    );
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation_from_import(
        &imported,
        frontend.clone(),
        vec![disconnected_root, helper.clone()],
    ))
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::UnreachableHelper
    );

    let recursive_helper = rebuild_function(
        helper,
        helper.frontend_identity(),
        helper.role(),
        helper.function_kind(),
        helper.is_concrete(),
        helper.fn_abi().clone(),
        vec![DirectCallObservationV1::new(
            root.monomorphization(),
            span("math-support/src/lib.rs", 35),
        )],
    );
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation_from_import(
        &imported,
        frontend,
        vec![root.clone(), recursive_helper],
    ))
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::RecursiveCall
    );
}

#[test]
fn frontend_roles_and_fnabi_types_must_reconcile_exactly() {
    let imported = authenticate(observation());
    let f32_type = type_id(0x11);
    let role_drift = FrontendUnitV1::new(vec![
        frontend_function(
            function_id(0x81),
            FunctionRoleV1::Kernel,
            "app::scalar_add::<f32>",
            vec![f32_type, f32_type],
            f32_type,
            7,
        ),
        frontend_function(
            function_id(0x82),
            FunctionRoleV1::Kernel,
            "math_support::add::<f32>",
            vec![f32_type, f32_type],
            f32_type,
            30,
        ),
    ])
    .unwrap();
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation_from_import(
        &imported,
        role_drift,
        imported.functions().to_vec(),
    ))
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::FunctionRoleMismatch
    );

    let root = imported
        .functions()
        .iter()
        .find(|function| function.role() == FunctionImportRoleV1::Kernel)
        .unwrap();
    let wrong_abi = fn_abi(0x61, &[type_id(0x12), f32_type], f32_type);
    let changed_root = rebuild_function(
        root,
        root.frontend_identity(),
        root.role(),
        root.function_kind(),
        root.is_concrete(),
        wrong_abi,
        root.calls().to_vec(),
    );
    let functions = imported
        .functions()
        .iter()
        .map(|function| {
            if function.role() == FunctionImportRoleV1::Kernel {
                changed_root.clone()
            } else {
                function.clone()
            }
        })
        .collect();
    let error = authenticate_ordinary_rust_scalar_kernel_v1(observation_from_import(
        &imported,
        imported.frontend_unit().clone(),
        functions,
    ))
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::SignatureMismatch
    );
}

#[test]
fn constructors_enforce_abi_call_and_observation_bounds() {
    assert_eq!(
        RustcAbiValueV1::new(type_id(1), [1; 32], 4, 3, RustcAbiPassModeV1::Direct,)
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::InvalidAbiLayout
    );
    assert_eq!(
        RustcAbiValueV1::new(type_id(1), [1; 32], 0, 1, RustcAbiPassModeV1::Direct,)
            .unwrap_err()
            .code(),
        OrdinaryRustScalarDiagnosticCodeV1::InvalidAbiLayout
    );

    let imported = authenticate(observation());
    let root = imported
        .functions()
        .iter()
        .find(|function| function.role() == FunctionImportRoleV1::Kernel)
        .unwrap();
    let calls = (1..=513)
        .map(|line| {
            DirectCallObservationV1::new(
                imported
                    .functions()
                    .iter()
                    .find(|function| function.role() == FunctionImportRoleV1::Helper)
                    .unwrap()
                    .monomorphization(),
                span("app/src/lib.rs", line),
            )
        })
        .collect();
    let error = ReachableFunctionObservationV1::new(
        root.frontend_identity(),
        root.role(),
        root.item_identity(),
        root.monomorphization(),
        root.source_identity(),
        root.mir_identity(),
        root.source_span().clone(),
        root.function_kind(),
        root.is_concrete(),
        root.fn_abi().clone(),
        calls,
    )
    .unwrap_err();
    assert_eq!(
        error.code(),
        OrdinaryRustScalarDiagnosticCodeV1::BoundExceeded
    );
}

trait ObservationTestAccess {
    fn clone_frontend_for_test(&self) -> FrontendUnitV1;
    fn kernel_item_for_test(&self) -> CanonicalKernelItemIdV1;
    fn kernel_instance_for_test(&self) -> CanonicalKernelInstIdV1;
    fn clone_functions_for_test(&self) -> Vec<ReachableFunctionObservationV1>;
}

impl ObservationTestAccess for OrdinaryRustScalarKernelObservationV1 {
    fn clone_frontend_for_test(&self) -> FrontendUnitV1 {
        authenticate(self.clone()).frontend_unit().clone()
    }

    fn kernel_item_for_test(&self) -> CanonicalKernelItemIdV1 {
        authenticate(self.clone()).kernel_item()
    }

    fn kernel_instance_for_test(&self) -> CanonicalKernelInstIdV1 {
        authenticate(self.clone()).kernel_instance()
    }

    fn clone_functions_for_test(&self) -> Vec<ReachableFunctionObservationV1> {
        authenticate(self.clone()).functions().to_vec()
    }
}
