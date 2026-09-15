//! Passive-lane component regressions, not source issuer or lifetime authority.
use super::*;
use super::super::super::{
    borrow_components_v1, closed_lane_flow, lane_work_v1, math_capture_flow_v1,
    matrix_access_borrow,
};

#[path = "closed_lane_flow_oracle.rs"]
mod oracle;

type Relations = Vec<(u32, u32, u32, u32, u32)>;

fn run(
    body: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    legacy: bool,
    limit: usize,
) -> Result<(Relations, usize), ProductionSemanticSsaErrorV1> {
    let callable = access(true, 64, true);
    let facts = BTreeMap::from([(0, MatrixAccessBorrow::for_callable(types, &callable).unwrap())]);
    let mut budget = Budget { remaining: limit, limit, profile: FlowWorkProfile::default() };
    let relations = if legacy {
        oracle::sites(body, Some(types), &facts, &mut budget)?.into_iter()
            .map(|(s,r)| (s.block, s.statement, r.parent, r.reference.index(), r.owned.index())).collect()
    } else {
        closed_lane_flow::sites(body, Some(types), &facts, &mut budget)?.into_iter()
            .map(|(s,r)| (s.block, s.statement, r.parent, r.reference.index(), r.owned.index())).collect()
    };
    Ok((relations, limit-budget.remaining))
}

fn rebuild(body: &SemanticFunctionDeclV1, blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    function(180, body.abi().clone(), body.locals().to_vec(), blocks)
}

fn padded(mutation: Mutation, count: usize) -> SemanticFunctionDeclV1 {
    let body = fixture(mutation);
    let zero = SemanticOperandV1::Constant(SemanticConstantV1::new(ty(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0,4).unwrap())));
    let mut statements = body.blocks()[2].statements().to_vec();
    statements.extend(std::iter::repeat_n(assign(13,1,SemanticRvalueKindV1::Use(zero)),count));
    let mut blocks=body.blocks().to_vec();
    blocks[2]=block(2,statements,body.blocks()[2].terminator().kind().clone());
    rebuild(&body,blocks)
}

#[test]
fn passive_terminal_first_matches_frozen_full_audit_for_every_existing_mutation() {
    for mutation in [Mutation::None,Mutation::Field,Mutation::Mutable,Mutation::Dereference,
        Mutation::Escape,Mutation::Duplicate,Mutation::TwoLeaves,Mutation::ParentEscape,Mutation::Fork] {
        let mut types=types();
        if matches!(mutation,Mutation::TwoLeaves) { types[9]=aggregate(9,&[7,7],&[0,8],16,8,true); }
        for count in [0,64] {
            let body=padded(mutation,count);
            let expected=run(&body,&types,true,1_048_576).unwrap().0;
            let actual=run(&body,&types,false,1_048_576).unwrap().0;
            assert_eq!(actual,expected,"{mutation:?} padding={count}");
        }
    }
}

#[test]
fn rejecting_one_lane_component_still_audits_every_survivor_statement() {
    let body=fixture(Mutation::Escape);
    let SemanticStatementKindV1::Assign(a)=body.blocks()[1].statements()[0].kind() else {panic!()};
    let SemanticRvalueKindV1::Borrow {place:field,..}=a.value().kind() else {panic!()};
    for kill in [false,true] {
        let mut statements=body.blocks()[1].statements().to_vec();
        statements.push(borrow(11,7,field.clone(),SemanticBorrowKindV1::Shared));
        if kill {
            statements.push(SemanticStatementV1::new(source(),SemanticStatementKindV1::Deinitialize(place(11,7))));
        }
        let mut blocks=body.blocks().to_vec();
        blocks[1]=block(1,statements,body.blocks()[1].terminator().kind().clone());
        let body=rebuild(&body,blocks);
        let old=run(&body,&types(),true,262_144).unwrap().0;
        let new=run(&body,&types(),false,262_144).unwrap().0;
        assert_eq!(new,old);
        assert_eq!(new.len(),usize::from(!kill));
        if !kill { assert_eq!(new[0],(1,4,3,4,3)); }
    }
}

#[test]
fn passive_unknown_call_kinds_and_backedge_keep_original_fail_closed_results() {
    let body=fixture(Mutation::Escape);
    for (name,terminal) in [
        ("direct-reference",call(0,vec![SemanticOperandV1::Copy(place(6,7))],place(0,0),2)),
        ("carrier",body.blocks()[1].terminator().kind().clone()),
        ("assert",SemanticTerminatorKindV1::Assert {
            condition:SemanticOperandV1::Copy(place(6,7)),expected:true,
            message:SemanticAssertMessageV1::NullPointerDereference,
            target:SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::AssertSuccess,SemanticBlockIdV1::from_index(2)),
            unwind:SemanticUnwindActionV1::Unreachable,
        }),
    ] {
        let mut blocks=body.blocks().to_vec();
        blocks[1]=block(1,body.blocks()[1].statements().to_vec(),terminal);
        blocks[2]=block(2,vec![],SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,SemanticBlockIdV1::from_index(1))));
        let body=rebuild(&body,blocks);
        assert_eq!(run(&body,&types(),false,262_144).unwrap().0,
            run(&body,&types(),true,262_144).unwrap().0,"{name}");
        assert!(run(&body,&types(),false,262_144).unwrap().0.is_empty(),"{name}");
    }
}

#[test]
fn terminal_rejected_lane_avoids_whole_body_audit_without_raising_work_limit() {
    let body=padded(Mutation::Escape,32_768);
    let (_,old_work)=run(&body,&types(),true,1_048_576).unwrap();
    let (new,new_work)=run(&body,&types(),false,262_144).unwrap();
    assert!(new.is_empty());
    assert!(old_work>262_144,"old work={old_work}");
    assert!(new_work<old_work/2,"old={old_work} new={new_work}");
    assert!(matches!(run(&body,&types(),true,262_144),Err(ProductionSemanticSsaErrorV1::BorrowFlowWork {..})));
    eprintln!("passive-rejection component work: old={old_work} new={new_work} unchanged-limit=262144");
}

#[test]
fn terminal_first_success_and_rejection_keep_exact_budget_exhaustion() {
    for mutation in [Mutation::None,Mutation::Escape] {
        let body=padded(mutation,32);
        let (expected,needed)=run(&body,&types(),false,262_144).unwrap();
        assert_eq!(run(&body,&types(),false,needed).unwrap(),(expected,needed));
        let error=run(&body,&types(),false,needed-1).unwrap_err();
        let ProductionSemanticSsaErrorV1::BorrowFlowWork {error,..}=error else {panic!("{error:?}")};
        assert!(matches!(*error,ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource:SsaPlannerResourceV1::WorkUnits,required,limit } if required==needed && limit==needed-1));
    }
}
