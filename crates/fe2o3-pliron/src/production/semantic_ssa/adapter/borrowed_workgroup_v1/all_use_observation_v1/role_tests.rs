//! Diagnostic state tests; no source/borrow acceptance or synthetic issuer.
use super::*;

#[test]
fn subgroup_selection_reuses_caps_without_scanning_matrix_types() {
    let (source, expansion)=source();
    let view=expansion.root(source.roots()[0]).unwrap();
    let candidates=candidates(view);
    let ty=candidates[0].source_type.index();
    let observation=Observation::with_role(Some(view),
        std::iter::from_fn(||panic!("subgroup selector scanned Matrix iterator")),
        [ty,ty].into_iter(),&candidates,Some(OsStr::new("1")),Some(OsStr::new("subgroup")));
    assert_eq!(&observation.owned[..observation.owned_len],&[ty]);
    assert_eq!(observation.len,candidates.iter().filter(|c|c.source_type.index()==ty).count());
    assert!(!observation.truncated);
    assert!(observation.tracked[..observation.len].iter().flatten()
        .all(|t|candidates[t.index].source_type.index()==ty));
    let capped=Observation::with_role(Some(view),std::iter::empty(),
        0..MAX_OWNED as u32+1,&candidates,Some(OsStr::new("1")),Some(OsStr::new("subgroup")));
    assert_eq!(capped.owned_len,MAX_OWNED);
    assert!(capped.truncated);
    assert_eq!(MAX_STEPS,65_536);
    assert_eq!(MAX_OUTPUT,32_768);
}

#[test]
fn role_never_enables_trace_or_scans_an_unselected_iterator() {
    let (source, expansion)=source();
    let view=expansion.root(source.roots()[0]).unwrap();
    let candidates=candidates(view);
    for (flag,role) in [(None,Some("subgroup")),(Some("0"),Some("subgroup")),
        (Some("1"),Some("Subgroup")),(Some("1"),Some("subgroup "))] {
        let observation=Observation::with_role(Some(view),
            std::iter::from_fn(||panic!("inactive Matrix scan")),
            std::iter::from_fn(||panic!("inactive Subgroup scan")),&candidates,
            flag.map(OsStr::new),role.map(OsStr::new));
        assert!(observation.view.is_none());
        assert_eq!(observation.remaining,MAX_STEPS);
        assert_eq!(observation.len,0);
    }
    for role in [None,Some(OsStr::new("matrix"))] {
        let observation=Observation::with_role(Some(view),[candidates[0].source_type.index()].into_iter(),
            std::iter::from_fn(||panic!("default Matrix trace scanned Subgroup iterator")),
            &candidates,Some(OsStr::new("1")),role);
        assert_eq!(observation.owned_len,1);
        assert!(observation.len>0);
    }
}

#[test]
fn first_invalidating_call_retains_actual_source_abi_and_original_records() {
    let (source, expansion)=source();
    let view=expansion.root(source.roots()[0]).unwrap();
    let mut candidates=candidates(view);
    let (block,call)=view.body().blocks().iter().enumerate().find_map(|(i,b)| {
        let SemanticTerminatorKindV1::Call(call)=b.terminator().kind() else {return None};
        let binding=source.callables().get(call.callee().index() as usize)?.binding()?;
        if binding.abi().source_input_types().is_empty() { return None; }
        Some((i,call))
    }).expect("admitted fixture has a non-body terminal");
    let mut observation=Observation::with_role(Some(view),std::iter::empty(),
        candidates.iter().map(|c|c.source_type.index()),&candidates,
        Some(OsStr::new("1")),Some(OsStr::new("subgroup")));
    observation.callables=source.callables();
    candidates[0].valid=false;
    observation.after(Site{block:block as u32,statement:view.body().blocks()[block].statements().len() as u32},
        "call-unaccepted-argument",&candidates);
    let before=(*view.identity(),source.canonical_encoding().to_vec(),candidates[0].valid,observation.remaining);
    let mut out=Vec::new();
    observation.write(&mut out,view,&candidates,&BTreeSet::new(),|_|None).unwrap();
    let text=String::from_utf8(out).unwrap();
    let binding=source.callables()[call.callee().index() as usize].binding().unwrap();
    assert!(text.contains("mode=call-unaccepted-argument"));
    assert!(text.contains("source_block=Some("));
    assert!(text.contains(&format!("callable_abi callee={}",call.callee().index())));
    assert!(text.contains(&format!("source_identity={:?}",binding.identity())));
    assert!(text.contains(&format!("source_count={}",binding.abi().source_input_types().len())));
    assert!(text.contains(&format!("callable_source_arg=0 ty={} ownership={:?}",
        binding.abi().source_input_types()[0].index(),binding.abi().source_argument_ownership().first())));
    assert!(text.contains(&format!("callable_physical_arg=0 role={:?}",binding.abi().arguments()[0].role())));
    assert!(text.contains("work_truncated=false"));
    assert_eq!(before,(*view.identity(),source.canonical_encoding().to_vec(),candidates[0].valid,observation.remaining));
}

#[test]
fn missing_defined_and_budget_limited_callable_do_not_invent_an_abi() {
    let (source,_)=source();
    for (callables,index,expected) in [
        (&[][..],0,"missing=true"),
        (source.callables(),u32::MAX,"missing=true"),
    ] {
        let mut out=Vec::new();let mut remaining=64;
        callable_abi_v1::write(&mut out,index,callables,&mut remaining).unwrap();
        let text=String::from_utf8(out).unwrap();
        assert!(text.contains(expected));assert!(!text.contains("return_mode="));
        assert_eq!(remaining,60);
    }
    let defined=[SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(7))];
    let mut out=Vec::new();let mut remaining=64;
    callable_abi_v1::write(&mut out,0,&defined,&mut remaining).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("defined_function=7 non_body_abi_unavailable=true"));
    let mut out=Vec::new();let mut remaining=3;
    callable_abi_v1::write(&mut out,0,source.callables(),&mut remaining).unwrap();
    let text=String::from_utf8(out).unwrap();
    assert!(text.contains("work_truncated=true"));assert!(!text.contains("source_identity="));
    assert_eq!(remaining,0);
}

#[test]
fn callable_signature_prefix_and_diagnostic_work_are_bounded_independently() {
    let (source,_)=source();
    let (binding,operation,operation_identity)=source.callables().iter().find_map(|c| {
        let SemanticCallableDeclV1::CompilerIntrinsic {binding,operation,operation_identity}=c else {return None};
        (!binding.abi().arguments().is_empty()).then_some((binding,operation,operation_identity))
    }).unwrap();
    let abi=SemanticFunctionAbiV1::new(binding.abi().identity(),binding.abi().layout_identity(),
        binding.abi().canon_abi(),false,false,
        vec![binding.abi().arguments()[0].value().clone();5],binding.abi().return_value().clone()).unwrap();
    let callable=SemanticCallableDeclV1::CompilerIntrinsic {
        binding:SemanticNonBodyCallableBindingV1::new(binding.identity(),
            SemanticItemDefinitionIdentityV1::from_sha256([1;32]),
            SemanticMonomorphizationIdentityV1::from_sha256([2;32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([3;32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([4;32]),
            SemanticSourceProvenanceV1::unavailable(),abi),
        operation:operation.clone(),
        operation_identity:*operation_identity,
    };
    let mut out=Vec::new();let mut remaining=64;
    callable_abi_v1::write(&mut out,0,&[callable.clone()],&mut remaining).unwrap();
    let text=String::from_utf8(out).unwrap();
    assert_eq!(text.matches("callable_source_arg=").count(),4);
    assert_eq!(text.matches("callable_physical_arg=").count(),4);
    assert!(text.contains("prefix_truncated=true work_truncated=false"));
    assert_eq!(remaining,28);
    let mut out=Vec::new();let mut remaining=8;
    callable_abi_v1::write(&mut out,0,&[callable],&mut remaining).unwrap();
    let text=String::from_utf8(out).unwrap();
    assert_eq!(text.matches("callable_source_arg=").count(),1);
    assert_eq!(text.matches("callable_physical_arg=").count(),0);
    assert!(text.contains("work_truncated=true"));
    assert_eq!(remaining,0);
}
