use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, DiagnosticCode, Function, Kernel, LaunchDomain, LaunchExtent, Operation,
    ResourceCapabilityRequirementV1, Signature, Terminator,
};

fn requirement(bytes: u64) -> ExecutionCapabilityRequirementV1 {
    ExecutionCapabilityRequirementV1::Resource(
        ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(bytes),
    )
}

fn call(callee: impl Into<FunctionId>) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: callee.into(),
            arguments: vec![],
        },
    )
}

fn call_graph(edges: &[Vec<usize>]) -> Module {
    let mut module = Module::new("closure-worklist");
    for (index, callees) in edges.iter().enumerate() {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = callees
            .iter()
            .map(|callee| call(format!("f{callee}")))
            .collect();
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::internal_helper(
            format!("f{index}"),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
    }
    module
}

fn add_kernel(module: &mut Module, name: &str, callees: &[&str]) {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = callees.iter().map(|callee| call(*callee)).collect();
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        name,
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        name,
        name,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
}

#[test]
fn reverse_worklist_preserves_recursive_external_and_declaration_scopes() {
    let mut module = call_graph(&[vec![1, 1], vec![2], vec![0, 2], vec![]]);
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::Execution(requirement(16)));
    module.functions[2]
        .required_capabilities
        .insert(TargetCapability::Execution(requirement(32)));
    let mut external = Function::external_import("external", Signature::new(vec![], vec![]));
    external
        .required_capabilities
        .insert(TargetCapability::Execution(requirement(64)));
    module.functions[2].body.as_mut().unwrap().blocks[0]
        .operations
        .push(call("external"));
    module.functions.push(external);
    module
        .required_capabilities
        .insert(TargetCapability::Execution(requirement(128)));
    add_kernel(&mut module, "entry", &["f0"]);
    add_kernel(&mut module, "isolated", &["f3"]);
    module.kernels[0]
        .required_capabilities
        .insert(TargetCapability::Execution(requirement(256)));
    let (canonical, facts) = capture(&module).unwrap();

    let scoped = facts.effective_scoped_requirements.iter().fold(
        BTreeMap::<_, BTreeSet<_>>::new(),
        |mut result, fact| {
            result
                .entry(fact.scope.clone())
                .or_default()
                .insert(fact.requirement.clone());
            result
        },
    );
    let recursive = BTreeSet::from([requirement(16), requirement(32), requirement(64)]);
    for function in ["f0", "f1", "f2", "entry"] {
        assert_eq!(
            scoped[&ExecutionRequirementScopeV1::Function(FunctionId::new(function))],
            recursive
        );
    }
    assert_eq!(
        scoped[&ExecutionRequirementScopeV1::Function(FunctionId::new("external"))],
        BTreeSet::from([requirement(64)])
    );
    assert!(
        !scoped.contains_key(&ExecutionRequirementScopeV1::Function(FunctionId::new(
            "f3"
        )))
    );
    assert!(
        !scoped.contains_key(&ExecutionRequirementScopeV1::Kernel(KernelId::new(
            "isolated"
        )))
    );
    assert_eq!(
        scoped[&ExecutionRequirementScopeV1::Kernel(KernelId::new("entry"))],
        BTreeSet::from([
            requirement(16),
            requirement(32),
            requirement(64),
            requirement(256)
        ])
    );
    assert_eq!(
        facts.effective_requirements,
        vec![
            requirement(16),
            requirement(32),
            requirement(64),
            requirement(128)
        ]
    );
    assert_eq!(
        scoped[&ExecutionRequirementScopeV1::Module],
        facts
            .effective_requirements
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(facts.scoped_requirements.len(), 5);
    assert_eq!(
        canonical.canonical_bytes(),
        fe2o3_kernel_ir::encode_module_v13(&module).unwrap()
    );

    // The public vectors retain module/function/kernel declaration order.
    let mut reversed = module.clone();
    reversed.functions.reverse();
    let (_, reversed_facts) = capture(&reversed).unwrap();
    assert_eq!(
        facts.effective_requirements,
        reversed_facts.effective_requirements
    );
    let mut expected = Vec::new();
    for scope in std::iter::once(ExecutionRequirementScopeV1::Module)
        .chain(
            reversed
                .functions
                .iter()
                .map(|f| ExecutionRequirementScopeV1::Function(f.id.clone())),
        )
        .chain(
            reversed
                .kernels
                .iter()
                .map(|k| ExecutionRequirementScopeV1::Kernel(k.id.clone())),
        )
    {
        for requirement in scoped.get(&scope).into_iter().flatten() {
            expected.push(ScopedExecutionRequirementV1 {
                scope: scope.clone(),
                requirement: requirement.clone(),
            });
        }
    }
    assert_eq!(reversed_facts.effective_scoped_requirements, expected);
}

fn chain(count: usize) -> Module {
    let edges = (0..count)
        .map(|index| {
            if index + 1 == count {
                vec![]
            } else {
                vec![index + 1]
            }
        })
        .collect::<Vec<_>>();
    let mut module = call_graph(&edges);
    module
        .functions
        .last_mut()
        .unwrap()
        .required_capabilities
        .insert(TargetCapability::Execution(requirement(16)));
    module
}

#[test]
fn long_call_chains_have_linear_propagation_work() {
    let mut used = Vec::new();
    for count in [64, 256] {
        let mut budget = RequirementClosureBudget::new(10_000);
        let (_, facts) = capture_with_budget(&chain(count), &mut budget).unwrap();
        assert_eq!(facts.effective_scoped_requirements.len(), count + 1);
        used.push(budget.limit - budget.remaining);
    }
    assert!(
        used[1] <= 5 * used[0],
        "work grew faster than the chain: {used:?}"
    );
}

fn assert_exhausted(
    result: Result<
        (VerifiedCanonicalKernelIrV13, ProtectedCapabilityFactsV1),
        VerifiedCanonicalKernelIrErrorV13,
    >,
) {
    let Err(VerifiedCanonicalKernelIrErrorV13::Verification(error)) = result else {
        panic!("expected deterministic closure exhaustion, not partial facts");
    };
    assert!(error.contains(DiagnosticCode::ResourceLimit));
    assert!(
        error.diagnostics()[0]
            .message
            .contains("requirement closure work")
    );
}

#[test]
fn closure_budget_is_shared_through_materialization_and_capture_errors() {
    let module = chain(8);
    let mut budget = RequirementClosureBudget::new(MAX_REQUIREMENT_CLOSURE_WORK_V1);
    let (_, expected) = capture_with_budget(&module, &mut budget).unwrap();
    let used = budget.limit - budget.remaining;
    for limit in [0, 1, 8, used / 2, used - 1] {
        assert_exhausted(capture_with_budget(
            &module,
            &mut RequirementClosureBudget::new(limit),
        ));
    }
    let mut exact = RequirementClosureBudget::new(used);
    let (_, actual) = capture_with_budget(&module, &mut exact).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(exact.remaining, 0);
    assert_exhausted(capture_with_budget(&module, &mut exact));
    assert_eq!(
        RequirementClosureBudget::new(usize::MAX).limit,
        MAX_REQUIREMENT_CLOSURE_WORK_V1
    );
}

#[test]
fn recursive_fanout_charges_duplicate_edge_attempts_and_new_facts() {
    let mut module = call_graph(&[vec![1, 1, 2], vec![0, 2], vec![0, 1]]);
    for (index, function) in module.functions.iter_mut().enumerate() {
        function
            .required_capabilities
            .insert(TargetCapability::Execution(requirement(
                16 * (index as u64 + 1),
            )));
    }
    let mut budget = RequirementClosureBudget::new(1_000);
    let (_, expected) = capture_with_budget(&module, &mut budget).unwrap();
    assert_eq!(expected.effective_scoped_requirements.len(), 12);
    let used = budget.limit - budget.remaining;
    assert_exhausted(capture_with_budget(
        &module,
        &mut RequirementClosureBudget::new(used - 1),
    ));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(0);
    let mut deduplicated = RequirementClosureBudget::new(1_000);
    let (_, actual) = capture_with_budget(&module, &mut deduplicated).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(deduplicated.remaining, budget.remaining + 2);
}

#[test]
fn scoped_output_charges_copied_identity_bytes_before_materialization() {
    let mut module = call_graph(&[vec![]]);
    module.functions[0].id = FunctionId::new("x".repeat(128));
    module.functions[0]
        .required_capabilities
        .insert(TargetCapability::Execution(requirement(16)));
    // The graph is tiny; the shared budget must still pay for each scoped name.
    assert_exhausted(capture_with_budget(
        &module,
        &mut RequirementClosureBudget::new(128),
    ));
    let (_, facts) =
        capture_with_budget(&module, &mut RequirementClosureBudget::new(1_000)).unwrap();
    assert_eq!(facts.effective_requirements, vec![requirement(16)]);
    assert_eq!(facts.effective_scoped_requirements.len(), 2);
}
