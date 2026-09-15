use super::*;

fn policies(source: &AdmittedInertSemanticMirV1) -> Vec<u32> {
    source.callables().iter().filter_map(|callable| match callable {
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
        } => match contract.operation() {
            SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { capability, .. } => Some(capability.index()),
            _ => None,
        },
        _ => None,
    }).collect()
}

#[test]
fn policy_scope_selects_only_declared_issue_types_and_preserves_source_state() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let mut candidates = candidates(view);
    let policy = policies(&source);
    assert_eq!(policy.len(), 1);
    let selected = candidates.iter().position(|c| policy.contains(&c.source_type.index())).unwrap();
    let mut observed = Observation::with_policy_role(Some(view), source.callables(), &candidates,
        Some(OsStr::new("1")), Some(OsStr::new("policy")));
    assert_eq!(&observed.owned[..observed.owned_len], &policy);
    assert_eq!(observed.len, candidates.iter().filter(|c| policy.contains(&c.source_type.index())).count());
    assert_eq!(observed.remaining, MAX_STEPS - source.callables().len()
        - (policy.len() + candidates.len()) * (MAX_OWNED + 1));
    candidates[selected].valid = false;
    let site = candidates[selected].site;
    observed.after(site, "policy-ordinary-statement", &candidates);
    observed.after(candidates[0].site, "later", &candidates);
    let before = (source.canonical_encoding().to_vec(), *view.identity(),
        candidates.iter().map(|c| (c.valid,c.consumers,c.intrinsic_consumer)).collect::<Vec<_>>(),observed.remaining);
    let mut output=Vec::new();
    observed.write(&mut output,view,&candidates,&BTreeSet::new(),|_|None).unwrap();
    let output=String::from_utf8(output).unwrap();
    assert!(output.contains("scope=policy"));
    assert!(output.contains("mode=policy-ordinary-statement"));
    assert!(output.contains("source_statement=Some("));
    assert!(!output.contains("mode=later"));
    assert_eq!(before,(source.canonical_encoding().to_vec(),*view.identity(),
        candidates.iter().map(|c| (c.valid,c.consumers,c.intrinsic_consumer)).collect::<Vec<_>>(),observed.remaining));
}

#[test]
fn policy_scope_never_scans_disabled_or_unselected_roles() {
    let (source, expansion) = source();
    let view = expansion.root(source.roots()[0]).unwrap();
    let candidates = candidates(view);
    for (setting,role) in [(None,Some("policy")),(Some("0"),Some("policy")),
        (Some("true"),Some("policy")),(Some("1"),None),(Some("1"),Some("matrix")),
        (Some("1"),Some("subgroup")),(Some("1"),Some("lane")),(Some("1"),Some("Policy"))]
    {
        let observed=Observation::with_policy_role(Some(view),source.callables(),&candidates,
            setting.map(OsStr::new),role.map(OsStr::new));
        assert!(observed.view.is_none());
        assert_eq!(observed.remaining,MAX_STEPS);
        assert_eq!(observed.owned_len,0);
        assert_eq!(observed.len,0);
    }
    let missing=Observation::with_policy_role(None,source.callables(),&candidates,
        Some(OsStr::new("1")),Some(OsStr::new("policy")));
    assert!(missing.view.is_none());
    assert_eq!(missing.remaining,MAX_STEPS);
    let lane=Observation::with_lane_role(Some(view),std::iter::from_fn(||panic!("wrong lane scope")),
        &candidates,Some(OsStr::new("1")),Some(OsStr::new("policy")));
    assert!(lane.view.is_none());
}

#[test]
fn policy_roster_scan_spends_existing_budget_on_irrelevant_entries_and_stops() {
    let (source,expansion)=source();
    let view=expansion.root(source.roots()[0]).unwrap();
    let candidates=candidates(view);
    let defined=SemanticCallableDeclV1::defined(source.roots()[0]);
    let issue=source.callables().iter().find(|c| matches!(c,
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
        } if matches!(contract.operation(),SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue { .. })
    )).unwrap().clone();
    // The valid issuer exists just beyond the diagnostic allowance. It cannot
    // be discovered via an uncharged filter or grant extra candidate work.
    let mut hidden=vec![defined.clone();MAX_STEPS];hidden.push(issue.clone());
    let observed=Observation::with_policy_role(Some(view),&hidden,&candidates,
        Some(OsStr::new("1")),Some(OsStr::new("policy")));
    assert!(observed.view.is_none());
    assert!(observed.truncated);
    assert_eq!(observed.remaining,0);
    assert_eq!(observed.len,0);
    // Finding an issuer earlier still does not reset work before tracking.
    let mut prefix=vec![issue];prefix.extend(vec![defined;MAX_STEPS]);
    let observed=Observation::with_policy_role(Some(view),&prefix,&candidates,
        Some(OsStr::new("1")),Some(OsStr::new("policy")));
    assert!(observed.view.is_some());
    assert!(observed.truncated);
    assert_eq!(observed.remaining,0);
    assert_eq!(observed.len,0);
    assert!(candidates.iter().all(|c|c.valid&&c.consumers==0));
}

#[test]
fn selected_policy_duplicates_do_not_expand_selection_or_hide_their_scan_cost() {
    let (source,expansion)=source();
    let view=expansion.root(source.roots()[0]).unwrap();
    let mut rows=source.callables().to_vec();rows.extend_from_slice(source.callables());
    let candidates=candidates(view);
    let observed=Observation::with_policy_role(Some(view),&rows,&candidates,
        Some(OsStr::new("1")),Some(OsStr::new("policy")));
    assert_eq!(observed.owned_len,1);
    assert_eq!(observed.remaining,MAX_STEPS-rows.len()-(2+candidates.len())*(MAX_OWNED+1));
    let no_issuers=[SemanticCallableDeclV1::defined(source.roots()[0])];
    let absent=Observation::with_policy_role(Some(view),&no_issuers,&candidates,
        Some(OsStr::new("1")),Some(OsStr::new("policy")));
    assert!(absent.view.is_none());
    assert_eq!(absent.remaining,MAX_STEPS-1);
    assert_eq!(absent.len,0);
}
