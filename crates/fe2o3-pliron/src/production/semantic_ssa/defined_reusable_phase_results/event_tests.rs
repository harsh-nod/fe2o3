//! Real planner checks over emitted events, not live source/loan authority.
use super::*;
use fe2o3_mir_model::{plan_ssa_v1,SsaPlannerErrorV1};
fn v(n:u32)->SsaVariableIdV1 {SsaVariableIdV1::new(n)}
fn l(n:u32)->SemanticLocalIdV1 {SemanticLocalIdV1::from_index(n)}
fn plan(events:Vec<SsaEventV1>,entry:Vec<SsaVariableIdV1>)->std::result::Result<SsaConstructionPlanV1,SsaPlannerErrorV1> {
    plan_ssa_v1(&SsaConstructionInputV1::new(SsaBlockIdV1::new(0),12,vec![true;12],entry,
        vec![SsaBlockInputV1::new(events,vec![])]))
}
#[test]
fn phase_bind_retains_both_shared_phase_and_unique_storage_uses() {
    let mut events=vec![];
    emit_result(ProductionSemanticPhaseResultInputsV1::Bind {phase_reference:l(0),storage_reference:l(1)},l(2),&mut events);
    assert_eq!(events,[SsaEventV1::Use(v(0)),SsaEventV1::Use(v(1)),SsaEventV1::Define(v(2))]);
    plan(events,vec![v(0),v(1)]).unwrap();
}
#[test]
fn phase_bind_missing_either_reference_is_exact_undefined_use() {
    for missing in [0,1] {
        let mut events=vec![];
        emit_result(ProductionSemanticPhaseResultInputsV1::Bind {phase_reference:l(0),storage_reference:l(1)},l(2),&mut events);
        assert!(matches!(plan(events,vec![v(1-missing)]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(missing)));
    }
}
#[test]
fn phase_finish_consumes_real_barrier_output_before_return_definition() {
    let mut events=vec![];
    emit_result(ProductionSemanticPhaseResultInputsV1::Finish {barrier_output:l(3)},l(4),&mut events);
    assert_eq!(events,[SsaEventV1::Use(v(3)),SsaEventV1::Kill(v(3)),SsaEventV1::Define(v(4))]);
    plan(events.clone(),vec![v(3)]).unwrap();
    assert!(matches!(plan(events,vec![]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(3)));
}
#[test]
fn phase_same_body_two_completion_chains_use_distinct_linear_relays() {
    let mut events=vec![];
    for (barrier,returned,relay) in [(0,2,10),(1,3,11)] {
        emit_result(ProductionSemanticPhaseResultInputsV1::Finish {barrier_output:l(barrier)},l(returned),&mut events);
        emit_pack(l(returned),true,v(relay),&mut events);
        emit_drop(v(relay),&mut events);
    }
    plan(events,vec![v(0),v(1)]).unwrap();
}
#[test]
fn phase_erased_pack_cannot_replace_missing_finish_or_reuse_consumed_finish() {
    let mut events=vec![];
    emit_pack(l(4),true,v(10),&mut events);
    assert!(matches!(plan(events.clone(),vec![]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(4)));
    emit_pack(l(4),true,v(11),&mut events);
    assert!(matches!(plan(events,vec![v(4)]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(4)));
}
#[test]
fn phase_completion_drop_rejects_missing_wrong_or_consumed_relay() {
    let mut missing=vec![];emit_drop(v(10),&mut missing);
    assert!(matches!(plan(missing,vec![]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(10)));
    let mut wrong=vec![];emit_pack(l(4),true,v(10),&mut wrong);emit_drop(v(11),&mut wrong);
    assert!(matches!(plan(wrong,vec![v(4)]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(11)));
    let mut twice=vec![];emit_pack(l(4),true,v(10),&mut twice);emit_drop(v(10),&mut twice);emit_drop(v(10),&mut twice);
    assert!(matches!(plan(twice,vec![v(4)]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(10)));
}
#[test]
fn phase_retained_pack_keeps_the_original_source_move_responsible() {
    let mut events=vec![];emit_pack(l(4),false,v(10),&mut events);
    assert_eq!(events,[SsaEventV1::Define(v(10))]);
    // The retained aggregate operand's normal adapter events are not removed.
    consume(l(4),&mut events);emit_drop(v(10),&mut events);
    plan(events.clone(),vec![v(4)]).unwrap();
    assert!(matches!(plan(events,vec![]),Err(SsaPlannerErrorV1::UndefinedAtUse {variable,..}) if variable==v(4)));
}
