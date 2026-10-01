use super::*;
use fe2o3_kernel_ir::{
    CompilerOrderingEffectSummaryV12, InterproceduralEffectDecisionV1 as Decision,
    InterproceduralEffectIncompleteReasonV1 as Incomplete, MemoryEffectSummary,
    OperationEffectSummaryV12,
};

fn complete(effects: impl IntoIterator<Item = MemoryEffect>) -> Decision {
    Decision::Complete(OperationEffectSummaryV12::new(
        MemoryEffectSummary::new(effects),
        CompilerOrderingEffectSummaryV12::empty(),
    ))
}

#[test]
fn helper_effect_diagnostic_distinguishes_missing_incomplete_and_complete() {
    let missing = ProductionHelperEffectDiagnosticV1::from_decision(None).to_string();
    assert_eq!(
        missing,
        "effect decision=missing; physical=unavailable; compiler_order=unavailable; contributing operation=unavailable",
    );
    let incomplete = Decision::Incomplete {
        partial: OperationEffectSummaryV12::pure(),
        reasons: vec![Incomplete::FunctionDeclaration {
            function: FunctionId::new("external"),
        }],
    };
    let rendered = ProductionHelperEffectDiagnosticV1::from_decision(Some(&incomplete)).to_string();
    assert_eq!(
        rendered,
        "effect decision=incomplete; partial_physical=[]; partial_compiler_order=[]; contributing operation=unavailable",
    );
    let pure = complete([]);
    let rendered = ProductionHelperEffectDiagnosticV1::from_decision(Some(&pure)).to_string();
    assert_eq!(
        rendered,
        "effect decision=complete-and-pure; physical=[]; compiler_order=[]; contributing operation=unavailable",
    );
    let effects = complete([
        MemoryEffect::Allocate(AddressSpace::Private),
        MemoryEffect::Read(AddressSpace::Private),
        MemoryEffect::Write(AddressSpace::Private),
    ]);
    let rendered = ProductionHelperEffectDiagnosticV1::from_decision(Some(&effects)).to_string();
    assert_eq!(
        rendered,
        "effect decision=complete-with-effects; physical=[allocate(private),read(private),write(private)]; compiler_order=[]; contributing operation=unavailable",
    );
    assert!(!effects.is_complete_and_pure());
}

#[test]
fn helper_effect_diagnostic_covers_all_physical_categories_and_spaces() {
    let spaces = [
        AddressSpace::Private,
        AddressSpace::Workgroup,
        AddressSpace::Global,
        AddressSpace::Constant,
        AddressSpace::Generic,
    ];
    for (space, name) in
        spaces
            .into_iter()
            .zip(["private", "workgroup", "global", "constant", "generic"])
    {
        let effects = complete([
            MemoryEffect::Allocate(space),
            MemoryEffect::Read(space),
            MemoryEffect::Write(space),
            MemoryEffect::VolatileRead(space),
            MemoryEffect::VolatileWrite(space),
            MemoryEffect::Atomic {
                address_space: space,
                scope: SynchronizationScope::Workgroup,
                ordering: MemoryOrdering::AcquireRelease,
            },
            MemoryEffect::Synchronize {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Workgroup,
                address_spaces: BTreeSet::from([space]),
            },
            MemoryEffect::Fence {
                memory_scope: SynchronizationScope::Workgroup,
                ordering: MemoryOrdering::AcquireRelease,
                address_spaces: BTreeSet::from([space]),
            },
        ]);
        let summary = ProductionHelperEffectDiagnosticV1::from_decision(Some(&effects));
        for mask in summary.physical {
            assert_eq!(mask, 0x80 | helper_effect_space_bit_v1(space));
        }
        let rendered = summary.to_string();
        assert_eq!(
            rendered,
            format!(
                "effect decision=complete-with-effects; physical=[allocate({name}),read({name}),write({name}),volatile-read({name}),volatile-write({name}),atomic({name}),synchronize({name}),fence({name})]; compiler_order=[]; contributing operation=unavailable",
            ),
        );
    }
    let empty_spaces = complete([
        MemoryEffect::Synchronize {
            execution_scope: SynchronizationScope::Workgroup,
            memory_scope: SynchronizationScope::Workgroup,
            address_spaces: BTreeSet::new(),
        },
        MemoryEffect::Fence {
            memory_scope: SynchronizationScope::Workgroup,
            ordering: MemoryOrdering::Acquire,
            address_spaces: BTreeSet::new(),
        },
    ]);
    let rendered =
        ProductionHelperEffectDiagnosticV1::from_decision(Some(&empty_spaces)).to_string();
    assert!(rendered.contains("physical=[synchronize(),fence()]"));
    assert!(rendered.starts_with("effect decision=complete-with-effects;"));
}

#[test]
fn helper_effect_diagnostic_preserves_partial_effects_and_ordering() {
    let ordering = CompilerOrderingEffectSummaryV12::ordered_verification_contract()
        .union(CompilerOrderingEffectSummaryV12::ordered_execution())
        .union(CompilerOrderingEffectSummaryV12::ordered_region());
    let decision = Decision::Incomplete {
        partial: OperationEffectSummaryV12::new(
            MemoryEffectSummary::new([MemoryEffect::Write(AddressSpace::Global)]),
            ordering,
        ),
        reasons: vec![Incomplete::RecursiveCallCycle {
            function: FunctionId::new("recursive"),
        }],
    };
    let before = decision.clone();
    let summary = ProductionHelperEffectDiagnosticV1::from_decision(Some(&decision));
    assert_eq!(
        summary.to_string(),
        "effect decision=incomplete; partial_physical=[write(global)]; partial_compiler_order=[verification-contract,execution,region]; contributing operation=unavailable",
    );
    assert_eq!(decision, before);
    for (ordering, label) in [
        (
            CompilerOrderingEffectSummaryV12::ordered_verification_contract(),
            "verification-contract",
        ),
        (
            CompilerOrderingEffectSummaryV12::ordered_execution(),
            "execution",
        ),
        (CompilerOrderingEffectSummaryV12::ordered_region(), "region"),
    ] {
        let decision = Decision::Complete(OperationEffectSummaryV12::new(
            MemoryEffectSummary::pure(),
            ordering,
        ));
        let rendered =
            ProductionHelperEffectDiagnosticV1::from_decision(Some(&decision)).to_string();
        assert!(rendered.starts_with("effect decision=complete-with-effects; physical=[];"));
        assert!(rendered.contains(&format!("compiler_order=[{label}]")));
    }
}

#[test]
fn helper_effect_diagnostic_is_fixed_size_and_format_bounded() {
    assert_eq!(
        std::mem::size_of::<ProductionHelperEffectDiagnosticV1>(),
        10
    );
    assert!(!std::mem::needs_drop::<ProductionHelperEffectDiagnosticV1>());
    assert!(std::mem::size_of::<ProductionSemanticKirErrorV1>() <= 128);
    let maximal = ProductionHelperEffectDiagnosticV1 {
        decision: HelperEffectDecisionKindV1::Incomplete,
        physical: [0x9f; 8],
        compiler_order: 7,
    };
    assert!(maximal.to_string().len() < 640);
    assert!(
        maximal
            .to_string()
            .ends_with("contributing operation=unavailable")
    );
}
