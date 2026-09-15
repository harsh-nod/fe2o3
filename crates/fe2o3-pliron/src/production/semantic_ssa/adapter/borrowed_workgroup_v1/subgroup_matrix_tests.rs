use super::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaResolvedEventV1, SsaVariableIdV1};
use super::super::matrix_access_borrow::MatrixAccessBorrow;

// Synthetic address-transparency components, not authenticated source issuers.
fn decl(index: u8, layout: SemanticTypeLayoutV1, shape: SemanticTypeShapeV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(SemanticTypeIdentityV1::from_sha256([index + 20; 32]),
        SemanticLayoutIdentityV1::from_sha256([index + 40; 32]), layout, shape)
}
fn aggregate(index: u8, fields: &[u32], offsets: &[u64], size: u64, align: u64, tuple: bool) -> SemanticTypeDeclV1 {
    let fields = fields.iter().copied().map(ty).collect();
    decl(index, SemanticTypeLayoutV1::aggregate(Some(size), align,
        SemanticAggregateLayoutV1::new(offsets.to_vec(), vec![]).unwrap()).unwrap(),
        if tuple { SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(fields).unwrap()) }
        else { SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()) })
}
fn pointer(index: u8, owned: u32, kind: SemanticPointerKindV1, mutability: SemanticMutabilityV1) -> SemanticTypeDeclV1 {
    decl(index, SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8), SemanticScalarValidityRangeV1::new(1, u64::MAX.into()))), false).unwrap(),
        SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1::new_with_kind(
            ty(owned), kind, mutability, 0, 64, SemanticPointerMetadataV1::None).unwrap()))
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let shared = |index, owned| pointer(index, owned, SemanticPointerKindV1::Reference, SemanticMutabilityV1::Immutable);
    vec![
        aggregate(0, &[], &[], 0, 1, false),
        decl(1, SemanticTypeLayoutV1::new_with_backend_repr(Some(4), 4,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 32, 4), SemanticScalarValidityRangeV1::new(0, u32::MAX.into()))), false).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits: 32 })),
        aggregate(2, &[1,0,0,0], &[0,4,4,4], 4, 4, false), // lane
        aggregate(3, &[2,0,0], &[0,4,4], 4, 4, false), // subgroup
        shared(4,3), shared(5,0), aggregate(6, &[], &[], 0, 1, false), // Matrix
        shared(7,2), shared(8,6),
        aggregate(9, &[8,7], &[0,8], 16, 8, true),
    ]
}
fn abi(inputs: &[u32], output: u32, shared: bool) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([180; 32]), SemanticLayoutIdentityV1::from_sha256([181; 32]),
        SemanticCanonAbiV1::Rust, SemanticExternAbiV1::Rust, false, false, inputs.len() as u32,
        inputs.iter().copied().map(ty).collect(), ty(output),
        inputs.iter().map(|&id| SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(ty(id),
            if id == 0 { SemanticAbiPassModeV1::Ignore } else { SemanticAbiPassModeV1::Direct(attributes(shared, false)) }))).collect(),
        SemanticAbiValueV1::new(ty(output), SemanticAbiPassModeV1::Ignore)).unwrap()
        .with_source_argument_ownership(vec![if shared { SemanticSourceArgumentOwnershipV1::SharedBorrow }
            else { SemanticSourceArgumentOwnershipV1::ByValue }; inputs.len()]).unwrap()
}
fn access(bound: bool, width: u32, shared: bool) -> SemanticCallableDeclV1 {
    let contract = SemanticExecutionCapabilityContractV1::new(E::MatrixAccess {
        subgroup: ty(4), epoch: ty(5), matrix: ty(6), width,
        subgroup_brand: SemanticTypeIdentityV1::from_sha256([166;32]),
    }, SemanticExecutionCapabilitySignatureV1::new(&[ty(4),ty(5)], ty(6)).unwrap(),
        provenance(), SemanticTypeIdentityV1::from_sha256([167;32]),
        SemanticTypeIdentityV1::from_sha256([168;32]), None,
        SemanticFunctionIdentityV1::from_sha256([170;32])).unwrap();
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            if bound { contract.source_identity() } else { SemanticFunctionIdentityV1::from_sha256([171;32]) },
            SemanticItemDefinitionIdentityV1::from_sha256([170;32]), SemanticMonomorphizationIdentityV1::from_sha256([170;32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([170;32]), SemanticConstGenericArgumentsIdentityV1::from_sha256([170;32]),
            source(), abi(&[4,5],6,shared)),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([170;32]),
    }
}
#[derive(Clone,Copy,Debug)]
enum Mutation { None, Field, Mutable, Dereference, Escape, Duplicate, TwoLeaves, ParentEscape, Fork }
fn fixture(mutation: Mutation) -> SemanticFunctionDeclV1 {
    let mut before = vec![borrow(3,4,place(1,3),SemanticBorrowKindV1::Shared), borrow(4,5,place(2,0),SemanticBorrowKindV1::Shared)];
    let parent = if matches!(mutation,Mutation::Fork) { before.push(alias(12,3,4)); 12 } else { 3 };
    let field = SemanticPlaceV1::new(SemanticLocalIdV1::from_index(parent), vec![
        SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference,ty(3)).unwrap(),
        SemanticProjectionV1::new(SemanticProjectionKindV1::Field(if matches!(mutation,Mutation::Field) {1} else {0}),ty(2)).unwrap(),
    ],ty(2)).unwrap();
    let mut after = vec![
        borrow(6,7,field,if matches!(mutation,Mutation::Mutable) {SemanticBorrowKindV1::Mutable} else {SemanticBorrowKindV1::Shared}),
        assign(8,9,SemanticRvalueKindV1::Aggregate(SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Tuple,vec![
            SemanticOperandV1::Copy(if matches!(mutation,Mutation::TwoLeaves) {place(6,7)} else {place(7,8)}),
            SemanticOperandV1::Copy(place(6,7)),
        ]).unwrap())),
        assign(9,9,SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(8,9)))),
        assign(10,8,SemanticRvalueKindV1::Use(SemanticOperandV1::Move(SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(9), vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0),ty(8)).unwrap()],ty(8)).unwrap()))),
    ];
    if matches!(mutation,Mutation::Duplicate) { after.push(alias(9,8,9)); }
    if matches!(mutation,Mutation::Dereference) {
        after.push(assign(13,1,SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(6),vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference,ty(2)).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0),ty(1)).unwrap(),
            ],ty(1)).unwrap()))));
    }
    let tail = match mutation {
        Mutation::Escape => call(0,vec![SemanticOperandV1::Copy(place(9,9))],place(0,0),2),
        Mutation::ParentEscape => call(0,vec![SemanticOperandV1::Copy(place(3,4))],place(0,0),2),
        _ => SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::Goto,SemanticBlockIdV1::from_index(2))),
    };
    function(180,abi(&[3,0,8],0,false), vec![
        local(0,0,SemanticLocalRoleV1::Return),local(1,3,SemanticLocalRoleV1::Argument(0)),
        local(2,0,SemanticLocalRoleV1::Argument(1)),local(3,4,SemanticLocalRoleV1::Temporary),
        local(4,5,SemanticLocalRoleV1::Temporary),local(5,6,SemanticLocalRoleV1::Temporary),
        local(6,7,SemanticLocalRoleV1::Temporary),local(7,8,SemanticLocalRoleV1::Argument(2)),
        local(8,9,SemanticLocalRoleV1::Temporary),local(9,9,SemanticLocalRoleV1::Temporary),
        local(10,8,SemanticLocalRoleV1::Temporary),local(11,7,SemanticLocalRoleV1::Temporary),
        local(12,4,SemanticLocalRoleV1::Temporary),local(13,1,SemanticLocalRoleV1::Temporary),
    ],vec![block(0,before,call(0,vec![SemanticOperandV1::Copy(place(3,4)),SemanticOperandV1::Copy(place(4,5))],place(5,6),1)),
        block(1,after,tail),block(2,vec![],SemanticTerminatorKindV1::Return)])
}
fn classify(body: &SemanticFunctionDeclV1, types: &[SemanticTypeDeclV1], callables: &[SemanticCallableDeclV1], limit: usize)
    -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    sites_with_math(body,callables,&[],limit,Some(types),&MathBorrowSitesV1::default())
}
fn root() -> SemanticTransparentBorrowSiteV1 { SemanticTransparentBorrowSiteV1 {block:0,statement:0} }

#[test]
fn matrix_access_and_passive_lane_tuple_emit_real_subgroup_ssa_use() {
    for mutation in [Mutation::None,Mutation::Fork] {
        let body=fixture(mutation); let types=types(); let callables=vec![access(true,64,true)];
        let sites=classify(&body,&types,&callables,MAX_FLOW_WORK).unwrap();
        assert!(sites.contains(&root()),"{mutation:?} {sites:?}");
        assert!(sites.contains(&SemanticTransparentBorrowSiteV1 {block:1,statement:0}));
        let mut at_borrow=None;
        let (input,_,_)=semantic_function_ssa_input_with_event_origins_v1(&body,Some(&types),&callables,&sites,
            |block,statement,events| if block==0 && statement==Some(0) { at_borrow=Some(events) });
        assert!(input.promotable()[1]);
        let plan=plan_ssa_with_limits_v1(&input,SsaPlannerLimitsV1::default()).unwrap();
        assert!(plan.promoted_variables().contains(&SsaVariableIdV1::new(1)));
        let range=at_borrow.unwrap();
        assert_eq!(plan.resolved_events(SsaBlockIdV1::new(0)).unwrap().iter().filter(|(event,resolved)|
            range.contains(&(*event as usize)) && matches!(resolved,SsaResolvedEventV1::Use {variable,..} if variable.get()==1)).count(),1);
    }
}

#[test]
fn every_lane_escape_projection_and_duplicate_poisons_subgroup_component() {
    for mutation in [Mutation::Field,Mutation::Mutable,Mutation::Dereference,Mutation::Escape,
        Mutation::Duplicate,Mutation::TwoLeaves,Mutation::ParentEscape] {
        let mut types=types();
        if matches!(mutation,Mutation::TwoLeaves) { types[9]=aggregate(9,&[7,7],&[0,8],16,8,true); }
        let body=fixture(mutation); let callables=vec![access(true,64,true)];
        let sites=classify(&body,&types,&callables,MAX_FLOW_WORK).unwrap();
        assert!(!sites.contains(&root()),"{mutation:?} {sites:?}");
        let (input,_,_)=semantic_function_ssa_input_v1(&body,Some(&types),&callables,&sites);
        assert!(!input.promotable()[1],"{mutation:?}");
    }
}

#[test]
fn matrix_source_binding_shared_abi_geometry_and_pointer_edges_are_exact() {
    let body=fixture(Mutation::None);
    for callable in [access(false,64,true),access(true,32,true),access(true,64,false)] {
        assert!(MatrixAccessBorrow::for_callable(&types(),&callable).is_none());
        assert!(!classify(&body,&types(),&[callable],MAX_FLOW_WORK).unwrap().contains(&root()));
    }
    for (index,kind,mutability,owned) in [
        (4,SemanticPointerKindV1::Raw,SemanticMutabilityV1::Immutable,3),
        (4,SemanticPointerKindV1::Reference,SemanticMutabilityV1::Mutable,3),
        (4,SemanticPointerKindV1::Reference,SemanticMutabilityV1::Immutable,2),
        (5,SemanticPointerKindV1::Reference,SemanticMutabilityV1::Immutable,3),
        (7,SemanticPointerKindV1::Raw,SemanticMutabilityV1::Immutable,2),
    ] {
        let mut types=types(); types[index]=pointer(index as u8,owned,kind,mutability);
        assert!(!classify(&body,&types,&[access(true,64,true)],MAX_FLOW_WORK).unwrap().contains(&root()));
    }
    assert!(sites_with_math(&body,&[access(true,64,true)],&[],MAX_FLOW_WORK,None,&MathBorrowSitesV1::default()).unwrap().is_empty());
}

#[path = "subgroup_matrix_tests/facts_cost_tests.rs"]
mod facts_cost_tests;

#[path = "subgroup_matrix_tests/used_lane_tests.rs"]
mod used_lane_tests;

#[path = "subgroup_matrix_tests/non_assignment_uses_tests.rs"]
mod non_assignment_uses_tests;

#[path = "subgroup_matrix_tests/carrier_tests.rs"]
mod carrier_tests;

#[test]
fn complete_subgroup_flow_has_exact_charged_work_boundary() {
    let body=fixture(Mutation::None); let types=types(); let callables=vec![access(true,64,true)];
    let (mut low,mut high)=(0,MAX_FLOW_WORK);
    while low<high { let mid=low+(high-low)/2; if classify(&body,&types,&callables,mid).is_ok() { high=mid } else { low=mid+1 } }
    let at=classify(&body,&types,&callables,low).unwrap();
    assert!(at.contains(&root()));
    assert_eq!(at,classify(&body,&types,&callables,MAX_FLOW_WORK).unwrap());
    let error=classify(&body,&types,&callables,low-1).unwrap_err();
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {error,..}=error else { panic!("{error:?}") };
    assert!(matches!(*error,ProductionSemanticSsaErrorV1::AggregateResourceLimit {
        resource:SsaPlannerResourceV1::WorkUnits,required,limit } if required==low && limit==low-1));
}
