use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    CanonicalKernelIrWorkBudgetV1, OperationKind, ValueId,
};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::rc::Rc;

#[derive(Clone)]
struct CountingBTreeKey {
    value: usize,
    comparisons: Rc<Cell<usize>>,
}

impl PartialEq for CountingBTreeKey {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for CountingBTreeKey {}

impl PartialOrd for CountingBTreeKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CountingBTreeKey {
    fn cmp(&self, other: &Self) -> Ordering {
        self.comparisons
            .set(self.comparisons.get().saturating_add(1));
        self.value.cmp(&other.value)
    }
}

#[test]
fn borrowed_and_lower_hex_extension_views_match_owned_capabilities() {
    let text = TargetCapability::Extension {
        namespace: "example.namespace".to_owned(),
        name: "plain-name".to_owned(),
    };
    let text_view = TargetCapabilityRefV1::from_owned(&text);
    assert!(text_view.matches(&text));
    assert_eq!(text_view.into_owned(), text);

    let digest = [0xab, 0x01, 0xf0];
    let encoded = TargetCapability::Extension {
        namespace: "digest.namespace".to_owned(),
        name: "ab01f0".to_owned(),
    };
    let digest_view = TargetCapabilityRefV1::extension_lower_hex("digest.namespace", &digest);
    assert!(digest_view.matches(&encoded));
    assert_eq!(digest_view.into_owned(), encoded);
    assert!(!digest_view.matches(&TargetCapability::Extension {
        namespace: "digest.namespace".to_owned(),
        name: "ab01F0".to_owned(),
    }));
}

#[test]
fn support_comparison_work_covers_dynamic_alias_and_terminal_decisions() {
    let view = TargetCapabilityRefV1::extension("namespace-a", "name-a");
    let candidate = TargetCapability::Extension {
        namespace: "namespace-long".to_owned(),
        name: "name-long".to_owned(),
    };
    let namespace_width = "namespace-a"
        .len()
        .max("namespace-long".len())
        .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
        .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len());
    let name_width = "name-a"
        .len()
        .max("name-long".len())
        .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len())
        .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len());
    assert_eq!(
        view.support_comparison_work_v1(&candidate),
        Some(4 + 4 * (namespace_width + name_width + 2))
    );
    assert_eq!(
        TargetCapabilityRefV1::Float16.support_comparison_work_v1(&candidate),
        Some(4)
    );

    let diagnostic = TargetCapabilityRefV1::extension(
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
    );
    let legacy = TargetCapability::Extension {
        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
    };
    assert!(diagnostic.is_satisfied_by_v1(&legacy));
    assert!(diagnostic.support_comparison_work_v1(&legacy).unwrap() > legacy_name_work(&legacy));
}

#[test]
fn supported_query_has_exact_empty_fixed_and_alias_boundaries() {
    let empty = BTreeSet::new();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(
        !target_capability_is_supported_with_budget_v1(
            TargetCapabilityRefV1::Float16,
            &empty,
            &mut budget,
        )
        .unwrap()
    );
    assert_eq!(budget.work(), 1);

    let required = TargetCapabilityRefV1::extension(
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
    );
    let legacy = TargetCapability::Extension {
        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
    };
    let supported = BTreeSet::from([legacy.clone()]);
    let required_width = AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len()
        + AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len()
        + 3;
    let alias_width = AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len()
        + AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len()
        + 3;
    let classification = 2
        * (AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
            .len()
            .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
            + AMDGPU_DIAGNOSTICS_CAPABILITY_NAME
                .len()
                .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len())
            + 2);
    let exact = 1 + required_width + classification + 1 + alias_width;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(exact - 1);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(matches!(
        target_capability_is_supported_with_budget_v1(required, &supported, &mut budget),
        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
            if error.actual() == exact && error.limit() == exact - 1
    ));
    assert_eq!(budget.work(), 1 + required_width + classification + 1);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(exact);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(
        target_capability_is_supported_with_budget_v1(required, &supported, &mut budget,).unwrap()
    );
    assert_eq!(budget.work(), exact);
    assert_eq!(budget.peak_storage(), 0);
}

#[test]
fn owned_exact_and_atomic_queries_preserve_their_independent_boundaries() {
    let supported = BTreeSet::from([
        TargetCapability::Float16,
        TargetCapability::Float64,
        TargetCapability::Int64,
    ]);
    // One terminal action plus three conservative fixed-width BTree rows.
    const EXACT_WORK: usize = 1 + (1 + 3 * 4);
    let run = |work_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let result = target_capability_is_supported_owned_with_budget_v1(
            &TargetCapability::Float64,
            &supported,
            &mut budget,
        );
        (result, budget.work(), budget.peak_storage())
    };
    assert_eq!(run(EXACT_WORK), (Ok(true), EXACT_WORK, 0));
    assert!(matches!(
        run(EXACT_WORK - 1),
        (
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error)),
            1,
            0,
        ) if error.actual() == EXACT_WORK && error.limit() == EXACT_WORK - 1
    ));

    let required = TargetCapability::Atomic {
        width_bits: 32,
        address_space: AddressSpace::Global,
        max_scope: SynchronizationScope::Subgroup,
    };
    let broader = TargetCapability::Atomic {
        width_bits: 32,
        address_space: AddressSpace::Global,
        max_scope: SynchronizationScope::Workgroup,
    };
    let supported = BTreeSet::from([broader]);
    // Atomic support is a partial order, so the borrowed candidate scan keeps
    // its one action plus one four-unit structural comparison.
    const ATOMIC_WORK: usize = 1 + (1 + 4);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(ATOMIC_WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(
        target_capability_is_supported_owned_with_budget_v1(&required, &supported, &mut budget,)
            .unwrap()
    );
    assert_eq!(budget.work(), ATOMIC_WORK);
    assert_eq!(budget.peak_storage(), 0);
}

#[test]
#[allow(
    clippy::mutable_key_type,
    reason = "Only the comparison counter mutates; key ordering depends on the immutable value"
)]
fn owned_btree_query_bound_covers_pinned_node_searches() {
    const POPULATIONS: &[usize] = &[0, 1, 5, 10, 11, 12, 22, 23, 70, 71, 72, 255, 1024];
    for &population in POPULATIONS {
        let comparison_bound = owned_btree_query_comparison_bound_v1(population).unwrap();
        for reverse in [false, true] {
            let comparisons = Rc::new(Cell::new(0));
            let values: Vec<_> = if reverse {
                (0..population).rev().collect()
            } else {
                (0..population).collect()
            };
            let set: BTreeSet<_> = values
                .into_iter()
                .map(|value| CountingBTreeKey {
                    value,
                    comparisons: comparisons.clone(),
                })
                .collect();
            for value in [0, population / 2, population, usize::MAX] {
                comparisons.set(0);
                let needle = CountingBTreeKey {
                    value,
                    comparisons: comparisons.clone(),
                };
                let _ = set.contains(&needle);
                assert!(
                    comparisons.get() <= comparison_bound,
                    "population={population}, reverse={reverse}, value={value}, actual={}, bound={comparison_bound}",
                    comparisons.get(),
                );
            }
        }
    }
}

#[test]
fn owned_large_roster_query_has_a_logarithmic_pinned_boundary() {
    let mut supported = BTreeSet::new();
    for ordinal in 0..128 {
        supported.insert(TargetCapability::Extension {
            namespace: "test.capability".to_owned(),
            name: format!("capability-{ordinal:04}"),
        });
    }
    supported.insert(TargetCapability::Float64);
    let comparisons = owned_btree_query_comparison_bound_v1(supported.len()).unwrap();
    assert_eq!(comparisons, 33);
    assert!(comparisons < supported.len());
    let exact_work = 1 + (1 + comparisons * 4);

    let run = |work_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let result = target_capability_is_supported_owned_with_budget_v1(
            &TargetCapability::Float64,
            &supported,
            &mut budget,
        );
        (result, budget.work(), budget.peak_storage())
    };
    assert_eq!(run(exact_work), (Ok(true), exact_work, 0));
    assert!(matches!(
        run(exact_work - 1),
        (
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error)),
            1,
            0,
        ) if error.actual() == exact_work && error.limit() == exact_work - 1
    ));
}

#[test]
fn owned_empty_query_prepays_long_nonalias_classification() {
    let required = TargetCapability::Extension {
        namespace: AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: format!("{}{}", AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, "x".repeat(4096)),
    };
    let supported = BTreeSet::new();
    let classification =
        diagnostic_alias_classification_work_v1(TargetCapabilityRefV1::from_owned(&required))
            .unwrap();
    let exact_work = 1 + 1 + classification;
    let run = |work_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let result =
            target_capability_is_supported_owned_with_budget_v1(&required, &supported, &mut budget);
        (result, budget.work(), budget.peak_storage())
    };
    assert_eq!(run(exact_work), (Ok(false), exact_work, 0));
    assert!(matches!(
        run(exact_work - 1),
        (
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error)),
            2,
            0,
        ) if error.actual() == exact_work && error.limit() == exact_work - 1
    ));
}

#[test]
fn owned_diagnostic_alias_query_has_exact_work_without_scratch_storage() {
    let required = TargetCapability::Extension {
        namespace: AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
    };
    let alias = TargetCapability::Extension {
        namespace: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.to_owned(),
        name: AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.to_owned(),
    };
    let supported = BTreeSet::from([alias]);
    let required_width = AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len()
        + AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len()
        + 3;
    let alias_width = AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len()
        + AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len()
        + 3;
    let exact_query = 1 + required_width;
    let alias_classification = 2
        * (AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE
            .len()
            .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
            + AMDGPU_DIAGNOSTICS_CAPABILITY_NAME
                .len()
                .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len())
            + 2);
    let alias_query = 1 + alias_width;
    let exact_work = 1 + exact_query + alias_classification + alias_query;
    let run = |work_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let result =
            target_capability_is_supported_owned_with_budget_v1(&required, &supported, &mut budget);
        (
            result,
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
        )
    };
    assert_eq!(run(exact_work), (Ok(true), exact_work, 0, 0));
    assert!(matches!(
        run(exact_work - 1),
        (
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error)),
            accepted,
            0,
            0,
        ) if error.actual() == exact_work
            && error.limit() == exact_work - 1
            && accepted == 1 + exact_query + alias_classification + 1
    ));

    // Exact and alternate spellings both remain allocation-free.
    let exact_supported = BTreeSet::from([required.clone()]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1 + exact_query);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(
        target_capability_is_supported_owned_with_budget_v1(
            &required,
            &exact_supported,
            &mut budget,
        )
        .unwrap()
    );
    assert_eq!((budget.work(), budget.peak_storage()), (1 + exact_query, 0));

    let reverse_required = supported.iter().next().unwrap();
    let reverse_supported = BTreeSet::from([required.clone()]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(
        target_capability_is_supported_owned_with_budget_v1(
            reverse_required,
            &reverse_supported,
            &mut budget,
        )
        .unwrap()
    );
    assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
}

fn legacy_name_work(capability: &TargetCapability) -> usize {
    let TargetCapability::Extension { namespace, name } = capability else {
        return 0;
    };
    namespace.len() + name.len()
}

fn call(name: &str, arity: usize) -> Operation {
    Operation::new(
        Vec::new(),
        OperationKind::Call {
            callee: name.into(),
            arguments: vec![ValueId(0); arity],
        },
    )
}

fn visited(operation: &Operation) -> Vec<TargetCapability> {
    let mut capabilities = Vec::new();
    operation
        .try_visit_required_capabilities_v1(|capability| {
            capabilities.push(capability.into_owned());
            Ok::<_, std::convert::Infallible>(())
        })
        .unwrap();
    capabilities
}

#[test]
fn every_reserved_call_descriptor_preserves_capability_and_arity() {
    for (name, descriptor) in AmdGpuDiagnosticOperation::intrinsic_descriptor_roster_v1() {
        assert_eq!(
            AmdGpuDiagnosticOperation::from_intrinsic_id(&name.into())
                .unwrap()
                .intrinsic_function_id()
                .as_str(),
            name
        );
        let correct = call(name, descriptor.arity());
        let OperationKind::Call { callee, arguments } = &correct.kind else {
            unreachable!()
        };
        let expected = AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments)
            .unwrap()
            .required_capabilities()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(visited(&correct), expected, "{name}");
        let wrong = call(name, descriptor.arity() + 1);
        let OperationKind::Call { callee, arguments } = &wrong.kind else {
            unreachable!()
        };
        assert!(AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments).is_none());
        assert!(visited(&wrong).is_empty(), "{name}");
    }
    for (name, descriptor) in FloatOperation::intrinsic_descriptor_roster_v1() {
        assert_eq!(
            FloatOperation::from_intrinsic_id(&name.into())
                .unwrap()
                .intrinsic_function_id()
                .as_str(),
            name
        );
        let correct = call(name, descriptor.arity());
        let OperationKind::Call { callee, arguments } = &correct.kind else {
            unreachable!()
        };
        let expected = FloatOperation::from_intrinsic_call(callee, arguments)
            .unwrap()
            .required_capabilities()
            .into_iter()
            .collect::<Vec<_>>();
        assert_eq!(visited(&correct), expected, "{name}");
        let wrong = call(name, descriptor.arity() + 1);
        let OperationKind::Call { callee, arguments } = &wrong.kind else {
            unreachable!()
        };
        assert!(FloatOperation::from_intrinsic_call(callee, arguments).is_none());
        assert!(visited(&wrong).is_empty(), "{name}");
    }
}

#[test]
fn unknown_long_prefix_call_has_exact_portable_classification_bound() {
    let name = format!(
        "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_profiling_marker{}",
        "-unknown-suffix".repeat(256)
    );
    let operation = call(&name, 3);
    let exact_work = 1
        + AmdGpuDiagnosticOperation::INTRINSIC_DESCRIPTOR_COUNT_V1 * (name.len() + 2)
        + FloatOperation::INTRINSIC_DESCRIPTOR_COUNT_V1 * (name.len() + 2);
    assert_eq!(
        operation.required_capability_visitation_work_v1(),
        Some(exact_work)
    );

    let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work - 1);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(matches!(
        budget.charge_work(exact_work),
        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
            if error.actual() == exact_work && error.limit() == exact_work - 1
    ));
    assert_eq!(budget.work(), 0);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    budget.charge_work(exact_work).unwrap();
    assert!(visited(&operation).is_empty());
    assert_eq!(budget.work(), exact_work);
}
