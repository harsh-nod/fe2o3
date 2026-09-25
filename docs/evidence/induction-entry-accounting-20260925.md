# Induction Entry Accounting

Compiler candidate: `3ab46198dd6409d2ede4f6bd9cc96a28d8f4fb03`.
This fixes the shared source-accounting defect identified during the
[conditional J integration](conditional-j-source-join-20260925.md). It changes
neither proof requirements nor native/artifact/launch authority.

## Cause And Invariants

The induction scope outlives nested slice-extent scratch. Previously its first
query sampled a storage floor while that temporary scratch was reserved. After
the extent scope released its own storage, induction replay incorrectly treated
the release as lost caller storage and returned `Resource(Accounting)`.

The scope now snapshots the original budget address, work-ledger identity and
storage floor together at outer entry. Every induction query and active cleanup
requires that same account and at least the caller floor plus induction-owned
storage. Shorter-lived scopes may release their own reservations independently.

Entry-custody errors are deferred until actual induction use, preserving
historical adapters that do not provide this capability. A failed entry cannot
be replaced by a newly sampled account. Header reservation remains lazy and
marks initialization complete only after successful reservation; a caught
reservation failure cannot make a later retry omit its header charge.

Cleanup releases only induction-owned storage, preserves accepted work and
denial history, and rejects missing caller storage or account substitution.
No limit is raised and no replacement production account is introduced.

## Validation

These final-candidate guards used one unchanged 8,178-file source inventory,
SHA256 `bec018b04babba1c083ae289c1e20447c566d72bd238f233f0185c8cb0481578`.
The pinned nightly was `2026-04-03`, with locked offline dependencies, one
Cargo job/test thread, disabled HIP, hidden GPUs, a 12-GiB virtual-memory
ceiling and a 1,200-second deadline. This report was added afterward.

| Guard | Result | Log SHA256 |
| --- | --- | --- |
| `induction-entry-account-tests-r3` | 22 passed | `3832dd1e1b72b8d0a992c655d585e32919c71ca5bf93afb27edf9d1771182f64` |
| `induction-entry-production-regressions-r1` | 62 passed, 3 ignored | `873348f30059605386b45f0dec5cc503f39f6fc4cc98651205bfa6738d5fafb8` |
| `induction-entry-normal-check-r1` | library check passed | `7948c111d8ed7ef0d87773b3e65ee726ce0cbd14a549c11565fe68b6099e715b` |

The component tests cover actual nested extent/induction scopes, exact and
one-short work/storage limits, first-header retry, original account identity,
missing custody, historical no-induction behavior, and cleanup on return and
unwind. New lifecycle tests have their own module; the existing test file stays
below the repository's size limit.

The production regression selection was the union of these filters:

```text
canonical_assertion_multi_entry_source_projection_precedes_target_binding
source_launch_
refined_forwarding_native_both_actual_rewrites_share_one_middle_owner_and_final_text
conditional_prefix
policy6_
```

The exact final-graph test that failed in the archived pre-change e333 binary
now passes. The source-only fixture also passes for ordinary and looped input
before target binding. These results are local compiler tests, not GPU runs.
Existing unrelated warnings remain.

## Main Integration

Both main branches advanced during validation to independent partial-move
discriminant fix `09b54c16636ee0e8eab9dbe71c491a936931e144`. Initial pushes
were rejected, not forced. The incoming change was preserved in conflict-free
merge `b706f58ead0c534ea35814b5992dd002fdd836ae`.

Guard `induction-entry-merged-regressions-r1` rebuilt that merged tree and ran
the union of both test selections above: **84 passed, zero failed, three
ignored**. Its unchanged 8,182-file source inventory SHA256 is
`6ad3fa3f0483ba9164998d7da9fb1415de991e4d34eeea403dd3894c1168f1bd`;
log SHA256 is `e24ee2fa06d66f5cf76e810d9f6d90de0d304f35b392176488f61d2b4c1069ea`.
This section was added afterward. The separate normal-library check in the
earlier table remains evidence for pre-merge candidate `3ab46198d`.

## Historical Runs And Remaining Validation

The earlier broad 98-test final-entry run on `534d1d7f` was intentionally stopped
after five completed passes to integrate the reservation-retry review fix.
Its stable-source log SHA256 is
`2054ca40f089395cd409deb044cfe376c438b340974a9183b4dce83f847baa65`.
It is partial evidence, not a passing full suite. A subsequent build was stopped
to split the oversized test file before final-candidate validation above.
Both owned process groups were confirmed empty before source changes.

The subsequent [conditional F report](conditional-final-source-join-20260925.md)
records the completed 98-test inventory on baseline `a3efc57a9`: 94 passed, zero
failed and four ignored, with every name accounted for. It separately records
new conditional source-through-F implementation and focused candidate tests;
the baseline results must not be attributed to that newer source revision.
The genuine-proof tests selected above remain ignored. Fresh protected F
execution, conditional native recovery, safe launch and the full tutorial target
matrix remain incomplete. No #272 milestone closes and no kernel gains 47/47
end-to-end credit from this change.
