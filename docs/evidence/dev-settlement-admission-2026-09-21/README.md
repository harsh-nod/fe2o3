# Settlement Admission And Unknown Execution

This development packet verifies exact logical settlement preflight, executable
Unknown marking and conditional settlement/issuance composition. It does not
verify the Success/NoEffect staging/commit loop or enable native pending-producer
admission. `SOURCE` binds implementation, packet tools and CPU inputs to Git
objects. Runs began before that commit, not from a recorded clean/signed HEAD.

## Established Scope

- Production settlement now calls a journal-private immutable preflight and then
  stages/commits under the original exclusive borrow. Error order, touched-member
  traversal and indexed-access bounds are unchanged. No public preflight token or
  separately callable commit API was introduced.
- The logical header/member/chain/return-capacity executors and combined preflight
  match exact decisions without a valid-state precondition. They preserve header
  and evidence precedence, full-key chain ordering, exact allocation checks,
  backlinks, epochs and prior lineage, checked return arithmetic, observed physical
  headroom, logical capacities, and ordered scratch validation.
- Capacity arguments are observations. They are not proved equal to live Rust
  `Vec::capacity()` values. The logical model does not count indexed accesses.
- Raw Unknown execution revalidates the complete retained chain, changes only a
  Pending writer's tag, exactly frames all errors, and is object-identical on
  repeated Unknown marking. Its issued wrapper preserves custody, writer history,
  exact Reserved count, stable leases and producer reservations.
- General Success/NoEffect settlement preservation now composes existing exact
  settlement relations with issuance/history and producer custody. It has no
  global-idle premise and retains historical writer identities after their slots
  are freed. This is a conditional relation theorem, not a settlement executor.
- Constructor/enroll/register/Begin witnesses reach empty and two-member Pending
  states, exercise successful/rejected settlement preflight, then execute Unknown
  and repeated Unknown. They do not execute Success/NoEffect, acquire all four
  producer statuses, or derive the settlement relation from a commit loop.

## Qualification

`proof/` retains two whole-crate positives at **338 verified, 0 errors**, including
321 inherited Begin-custody obligations. Counts overlap importing campaigns and
must not be summed. Ten executable-body controls require exactly **337 verified,
1 intended postcondition error** and a normal failure exit:

| Control | Deliberate defect |
| --- | --- |
| `header_context` | Omit retained-writer context validation |
| `member_lineage` | Admit prior lineage at or beyond attempt epoch |
| `trailing_chain` | Admit a remaining chain link |
| `physical_headroom` | Admit an empty writer with insufficient writer-return storage |
| `scratch_busy` | Admit occupied settlement scratch |
| `evidence_identity` | Substitute the evidence-mismatch error |
| `return_precedence` | Check return storage before writer/evidence admission |
| `unknown_error_frame` | Mutate Reserved count on Unknown rejection |
| `unknown_result` | Return failure after Unknown mutation |
| `issued_result` | Substitute the issued Unknown wrapper's result |

The pinned Verus release is `0.2026.08.09.92f466f`, with unchanged default resource
limits, four solver threads, a 180-second deadline and `--no-cheating`. Complete
tool closure checks bracket the campaign: 190 files, 129019839 bytes. Recursive
source and checker dependencies are authenticated, including the inherited exact
lifecycle guard projection. Generated executable bytes are compared with captured
inputs, hashed before/after each solver, and re-created independently by the
mandatory portable audit. Completed cases are frozen. No assumption, axiom or
external body was added. Partial checks, timeouts and unrelated diagnostics do
not count as qualification.

An earlier campaign was deliberately stopped to correct one negative control
that would have failed a loop invariant instead of its target postcondition.
Its owned process group was confirmed absent; that partial campaign is not part
of this packet. All retained runs use the final checker and fresh output directory.

`cargo/` records **823 unit tests passed, 2 ignored; 27 doctests passed**, formatting
and Clippy with warnings denied. The independent 1,080-case fault matrix checks
preflight and complete settlement results, exact state/storage frames and access
counts across simultaneous header/evidence/chain/capacity/scratch faults, empty
and nonempty writers, and both outcomes. The mixed-reader regression now covers
target Success/NoEffect/Unknown with all four unrelated producer statuses and a
stable lease, target producer-reader reservations for nonempty chains, resolved
producer-slot reuse, rejection snapshots and reader arena identity. These are
production tests, not formal constructor-derived mixed-state reachability.

`selftest.py` rejects 28 adverse actual diagnostic changes and four unauthenticated
source inputs. The audit re-creates all input rosters/mutations from Git objects,
checks exact process-custody receipts and final CPU summaries, and requires the
complete manifest. It establishes source binding, not a clean-worktree campaign
or a compiler-checked production-to-machine correspondence proof.

## Remaining Work

Connect successful settlement preflight to exact prestate-derived scratch plans,
sequential Success/NoEffect commit and `settle_chain_relation_v1`; prove complete
unchanged-on-error contents for that whole executor. Bind physical capacities and
fallible allocation to Rust and establish source correspondence/panic semantics.
Context event/producer/result custody, success-gated backend contracts, bounded
producer-first progress and native XGMI qualification remain open. Only then can
matched HIP/HSA runs support a scoped performance claim.

No shared proof runner, musl campaign, GPU test, MI300X cleanup or performance
comparison is included. This checker is standalone, not registered in the shared
runner. No MI300X processes or files were created.

## Audit And Reproduce

```sh
python3 -B docs/evidence/dev-settlement-admission-2026-09-21/audit.py --repo "$REPO"
python3 -B docs/evidence/dev-settlement-admission-2026-09-21/selftest.py \
  --repo "$REPO" --proof docs/evidence/dev-settlement-admission-2026-09-21/proof
```

The offline audit needs the `SOURCE` Git objects, not scratch paths or the Verus
installation. To rerun, use `check.py --repo "$REPO" --verus "$VERUS" --output
"$NEW_OUTPUT"`. CPU receipts use the unchanged recorder at
`docs/evidence/dev-enrollment-transaction-2026-09-21/cargo-checks.py`.
`SHA256SUMS` covers every packet file except itself. Retained evidence does not
complete the broader runtime parity objective.
