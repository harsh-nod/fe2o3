use super::*;
#[path = "fixture.rs"]
mod fixture;

pub(super) fn source(annotated: bool) -> AdmittedInertSemanticMirV1 {
    let mut f = fixture::fixture();
    let unit = SemanticTypeIdV1::from_index(9);
    f.declarations.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([10; 32]),
        SemanticLayoutIdentityV1::from_sha256([10; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    ));
    let original = &f.functions[0];
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        original.abi().identity(),
        SemanticLayoutIdentityV1::from_sha256([120; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticTypeIdV1::from_index(8)],
        unit,
        original.abi().arguments().to_vec(),
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let mut locals = original.locals().to_vec();
    let old_return = &locals[0];
    locals[0] = SemanticLocalDeclV1::new(
        old_return.identity(),
        unit,
        SemanticLocalRoleV1::Return,
        old_return.source(),
    );
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([4; 32]),
        f.types.output,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let mut blocks = original.blocks().to_vec();
    blocks[1] = fixture::block(
        2,
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                    f.types.input,
                    SemanticConstantValueV1::ZeroSized,
                ))],
                Some(SemanticCallDestinationV1::new(
                    fixture::place(3, f.types.output),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(2),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    f.functions[0] = SemanticFunctionDeclV1::new(
        original.identity(),
        SemanticFunctionRoleV1::KernelRoot,
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi,
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"reusable_lds_component".to_vec()).unwrap(),
        f.provenance.kernel_binding(),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let record = f.observe().unwrap();
    if annotated {
        f.functions[1] = f.functions[1]
            .clone()
            .with_defined_capability_contract(
                SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record),
            )
            .unwrap();
    }
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([120; 32])),
        f.declarations,
        vec![],
        vec![],
        vec![],
        f.functions,
        f.callables,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .expect("inert canonical component fixture")
}
