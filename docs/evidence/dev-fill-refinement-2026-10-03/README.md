# Executed Conditional Fill Refinement

Parent: `e3e5a9c266867877ef298c0bcbed62b7b4fd1015`.
This advances ordinary multi-GPU application admission. It grants no launch
authority and claims no GPU execution or performance improvement.

## Implementation

The closed fill recognizer retains three independent typed expression graphs:
actual semantic MIR, neutral KIR and replayed target KIR. Edges retain real operand
identity, including borrows, moves, sparse SSA IDs, casts, predicates and store
operands. Every semantic step retains its checked correspondence span, including
administrative steps. Graphs are append-only, type-checked and bounded to 512 nodes
and 1,024 semantic steps; the existing 64-operation KIR limit remains unchanged.

The public producer generates three store/byte relations from those graphs and
executes the shared wave and dispatch bodies. It borrows the exact compiler and
authenticated analyzer owners and retains generated source, obligation preimage
and strictly imported signed receipt. Boundary 5 is distinct from unconditional
and conditional MIR/KIR proofs. The obligation binds complete compiler and target
lineage, exact V4/V5 correspondence, all graphs, generated source, entry, entire
HSACO, analyzer request, bundle and execution receipt. Existing MIR subject fields
are preserved rather than reused for machine-code hashes.

The protected runtime has a private ClosedFill profile selected only by boundary
5. The [pinned Verus interpreter](https://github.com/verus-lang/verus/blob/b677dd5a766f25f56e9aa1e32621aa4e53304b47/source/vir/src/interpreter.rs#L2115)
requests a 1 GiB stack for bit-vector simplification; this
profile disables that simplifier while retaining the existing 32 MiB clone guard.
Three proof assertions explicitly spell out their original helper expressions.
Machine bodies, specifications and theorem contracts do not change.

Nine bit-vector sites include two `usize` sites checked for both widths. The
controller authenticates exactly 12 successful pinned solver executions, with at
most two live/pending solver leaders. Auxiliary ordering, Verifier-only parentage,
descriptor and mapping checks, deadlines, resource bounds and cleanup remain
enforced. Configuration identities distinguish this profile; the default Ranked
profile and its one-solver limit are unchanged.

## Qualification

The ignored protected test uses the unchanged genuine `fill.handoff`, the previously
qualified 6,160-byte finalized payload and a freshly built real LLVM analyzer. The
Worker is measured again inside the local protected namespace, not admitted using
its different remote library closure. ROCm 7.2.4 LLVM/LLD build and machine-effect
CTest pass on `mi300x`, using two CPU build jobs and no GPUs.

The real producer verifies and imports the generated 49-obligation proof. Six
well-typed private recipe mutations fail logically: semantic/neutral/target zero
values, reversed neutral predicate, zero target offset, and all three graphs
agreeing on a wrong value. Each requires exit 1, no signal, exactly 48 verified/1
failed obligation, matching diagnostic counts, and only postcondition errors in
the generated store-relation theorem. Parse/type errors, resource failures and
unrelated theorem failures are not accepted as negative evidence.

Equivalent actual-operand aliases prove with different source and binding. A
wrong-value graph passed through the public producer returns no proof owner.
The shared dispatch proof independently retains its original 44/0 result under
the default Verus flags. Receipt tests cover all five boundaries, cross-boundary
retagging, unknown tags, stale binding, failed result and wrong toolchain.

CPU supervisor controls cover exact and missing/excess rosters, wrong executable,
nonzero exit, two concurrent solvers, excess concurrency, auxiliary order/success,
and reaping every observed descendant when another solver fails. Ownership
doctests reject cloning and escaping borrowed refinement custody.

| Campaign | Result |
| --- | --- |
| Verifier library | 163 passed, 8 default ignores |
| Functional-proof legacy/V2 receipt suites | 10 + 8 passed |
| Functional-proof ownership doctests | 2 passed |
| Conditional fill ownership doctests | 3 passed, 35 filtered |
| Host library | 211 passed, 1 default ignore |
| Native Worker machine-effect CTest | 1 passed |
| Shared dispatch proof with default flags | 44 verified, 0 errors |
| Complete protected campaign, first run | 1 passed, 176.89 seconds |
| Complete protected campaign, final repeat | 1 passed, 179.79 seconds |

Strict verifier-library Clippy, targeted rustfmt and whitespace checks pass.
Default ignores outside the explicit protected campaign are not claimed as tested.

Development diagnostics are kept separately: an incomplete Worker transfer failed
startup, the old stack profile rejected the interpreter, disabling simplification
alone exposed unsupported helper calls, and globally inlining the helpers disturbed
existing proof triggers. The final implementation uses explicit expressions only
inside the three assertions. A diagnostic-footer classifier and test-only moved
policy values were corrected before final qualification.

## Remaining Admission

The closed recognizer, intrinsic/ISA interpretation and hash assumptions remain
trusted. The theorem proves a conditional machine projection, not actual native
entry, allocation backing, schedule, visibility or completion. Local signing is
not protected compiler-origin attestation. No conditional-to-unconditional
conversion or caller-supplied proof source is exposed.

Next, consume the original compiler/target/analyzer owners into an owned pending
artifact with protected compiler-service and current-publication custody. Then
discharge exact packed coverage, full64 geometry, selected device and actual KFD
allocation/fixup premises before private invocation authority. Proof production
belongs at artifact admission; per-launch work should discharge the retained
contract, not rerun Verus. Reuse existing staging, settled H2D and PUBLIC XGMI for
admitted fill and guarded readback in both directions.

## Reproduction

The qualification archive contains the code-only patch, build/provision/test
scripts, Worker source and build identity, exact analyzer/refinement captures,
logical mutation diagnostics, CPU logs and read-only review notes. Large compiler
and Worker executables are not vendored. Reproduction requires fresh owned scratch
paths and the pinned runtime prerequisites described by the scripts.

The [qualification archive](qualification.tar.gz) contains 94 entries.
SHA-256: `4d46517ee5ce364cb2873c6bfc550f81b7ba6830dc812a3d1f4437491c97edb8`.
Its extracted code-only patch has SHA-256
`2dee3cc8ab8c4f094c91c2965bef0a6e114c446929d1d6acdd273583d1622e7a`,
matching the qualified working tree. The Worker executable SHA-256 is
`fb020a09969938d7fa849e106a146e81668d9d0b0d7dd40ded0e2145e6836715`;
the finalized payload SHA-256 is
`8b6c2e5b67ba2f76bb57d5f42aedbaf968f8dc12bb121daa5d7853cc55d158c6`.

The owned remote build directory was removed and its absence verified. No unrelated
shared-host resources were deleted; no GPU queues, resets or allocations were used.
