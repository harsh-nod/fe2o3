# Early TARGET Account And Native Preparation

Date: 2026-09-26. Source/test snapshot:
`59e233e09b306504dcb296c095ea07188bb13647`.
This continues the [issuer checkpoint](conditional-native-issuer-20260926.md).
All [#272](https://github.com/harsh-nod/fe2o3/issues/272) milestones M0-M7
remain open; this supplies no protected, machine-proof, GPU, or 47/47 credit.

## Implemented

The normal backend now creates its conditional TARGET account before target
and compiler-execution admission and before collection. The owned account moves
with retained admission; one borrowed view reaches normal publication/lowering
and the existing conditional preparation. That preparation no longer creates a
replacement Work/Budget. Its limits remain 2^54 logical work units and 256 MiB
storage. SOURCE retains its separate original account. Extraction callers
explicitly provide their own account. This is not whole-compiler accounting:
V1 execution admission is unchanged and does not charge this TARGET account.

Both native client families share a consuming `prepare` operation. Preparation
borrows the client's original account without exposing its peer. Eight work
units precede the callback; work identity and the inherited storage floor are
checked before returning the client and prepared value. Inner work, storage,
peaks and first denials are not rolled back on failure or unwind. A replaced
account or damaged floor is never repaired or refunded by client drop. The peer
closes on failure; successful preparation preserves the absolute deadline.

This is a preparation API, not a publication callback or native backend
activation. The backend still admits V1 execution custody and conditional
publication still refuses. No ordinary receipt conversion or fallback was added.

The isolated V2/V3 issuer harness now shares 17 admission and six readiness/
Cancel cases using each family's actual policy, key, admission, manifest,
readiness and packet types. V2 test names and markers are preserved. Both
targets retain explicit opt-in, distinct-UID disposable-root-container,
sealed-static-image, bounded-wait and private-child cleanup requirements.
All four tests per target are ignored by default; none was executed here.
See the [harness contract](../../crates/fe2o3-compiler-execution-issuer/README.md#isolated-native-harness).
Type-checking those targets is not protected-service qualification.

## Validation

Pinned nightly-2026-04-03, locked/offline Cargo, one build job, 12 GiB VM ceiling,
20-minute command limits and disabled GPU visibility. Source stayed fixed while
builds ran. The native unit rerun explicitly used eight test threads.

| Check | Result |
| --- | --- |
| Native client admission/preparation units | 16 passed, 0 failed/ignored |
| Backend account, preparation and source-wiring cases | 8 passed, 0 failed/ignored |
| Client doctests | 16 compile-fail and 2 positive passed |
| Integrated all-target check | Passed for client, issuer, broker service, cargo-fe2o3 and rustc-codegen-fe2o3 |
| Public Cancel/expired-deadline transcripts, V2 and V3 | 4 failed before preparation/exchange: receive-timeout socket option returned EPERM |
| Changed-file rustfmt, whitespace and hygiene | Passed |

"All-target" means Cargo target kinds, not GPU architectures. The focused unit
fixtures exercise accounting, not protected admission; three backend cases
inspect source wiring. No socket guard, timeout, isolation rule or refusal was
weakened. This is not an all-tests-green result, and the previous issuer
checkpoint's broader socket failures remain unresolved.

An independent source review found a raw-descriptor-number reuse race in the
test assertions. The fix uses a shared nonblocking pipe-EOF witness, also in
existing admission tests. The eight-thread rerun passed; follow-up review found
no remaining concrete defect. The expiration transcript is implemented but
cannot execute past the local socket prerequisite.

Final log names below live in the private evidence directory
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`.
All have prefix `conditional-early-account-`; repeated runs are not additive.

| Log Suffix | SHA-256 |
| --- | --- |
| client-tests-r2.log | `a06dd76b3bd1b2c620f2ffa04182781ea448c6172009078c525d23aebb717431` |
| backend-tests-r3.log | `b244330037d68722dc225239764b13d7fe223c3a6bc4e56cdc923f38b5800f37` |
| client-docs-r2.log | `e55e9876ac24ff174770919044b84977e097af227a77bc943b4c4d17a3c4c06b` |
| integrated-check-r1.log | `290b5d02548894ca6aa1f1d562859b65455c103ab46b32db2977faeaa02fc5d7` |
| socket-tests-r2.log | `468039143ed61132615b8875db70a0acacf38366413af9257901ed07216f1eae` |

## Remaining Integration

1. Independently pin native family/profile/launch/readiness and admit the
   inherited V3 policy/peer on this original account. Connect the preparation
   API to the normal compiler without propagating authority from decoded data.
2. Consume actual conditional Prepared custody only after all preparation
   postchecks; retain original proof/collector owners through invocation
   finish/revalidation, V5 publication, SubjectV3 acquisition and transport.
   Publication and terminal acquisition still need a deliberate original-budget
   lifetime connection: do not publish inside the new preparation callback.
3. Complete parent-pinned policy/history intake, locked V5 recovery before
   mapping, concrete Worker preflight and consumption of the same occurrence.
   Actual KIR-to-LLVM/ISA machine refinement remains separate required work.
4. Run protected service/source/proof and target-matched GPU validation, then
   update the release manifest/site and qualify all 47 kernels end to end.

All three SSH aliases still failed DNS from this environment. Normal fetches
for both Git remotes also failed DNS. Their public mains were observed through
GitHub metadata at `27ba44e2616945481557f4555f67327aabbab043`; those objects
were not available locally. Integration must preserve that concurrent work,
with no force push or alternative Git transport. No remote job or scratch was
created. Only three inactive test executables in this task's private build cache
were removed (154,653,080 bytes); source worktrees and reports were preserved.
