# Composed Native XGMI Request Accounting

Development evidence on parent `c042651406c8f04c5d57d3f19015cc69fddeee95`.
This enables composed request/N1/N2 admission for the separate native XGMI
backend. It does not close A1, A2, A3 or issue #182 and makes no HIP/HSA parity,
native-performance or production-refinement claim.

## Implementation

- Public composed-root constructors preserve the original endpoint/budget
  order. Both root admissions and complete device/bundle/request-binding checks
  precede either VM acquisition. Lower native account installation remains
  fallible; second-acquisition error or panic remains process-fatal.
- Immutable Required policy retains both distinct request accounts after native
  admission consumption and clean shutdown. Complete profile discovery rejects
  unhealthy endpoints rather than silently truncating or downgrading the roster.
- The selected endpoint must match the witness's exact leaf, full model, extent
  and retained credit before allocation-table reservation or native effects.
  Witness-free Required allocation and witnessed Legacy allocation reject.
  Unrelated session quarantine does not globally seal a healthy selected leaf.
- Both allocation APIs use the same record transaction. Terminal, zero and
  occupied handle states reject before effects. Storage and handle space are
  checked before lower allocation. Classified capacity rejection preserves the
  existing consumed-handle behavior. Lower ambiguity/panic seals the backend;
  no generic failure becomes a settled-no-owner refund. Successful lease custody
  is immediately inserted into the pre-reserved vacant record.

## Qualification Scope

In-tree tests execute Legacy policy, the public constructor's invalid-ID
preflight, generic binding-before-acquisition and the production record
transaction. Move-only owners check exact identity, custody and cleanup.
Existing subprocess tests check fail-stop on second VM acquisition failure.
Source-wiring assertions check the full-admission bridge, policy lifetime and
native trait call sites; they are structural evidence, not native execution.

The isolated source copy adds test-only accounting/model fixtures, not shipped
authority or test-support APIs. It runs production Context, request policy and
record transaction code through a synthetic backend with Box owners. It does
**not** execute `RequestPolicyV1::from_admissions`, mint checked-device/native
admissions, execute successful public native constructors, or call the actual
native XGMI trait allocation/shutdown implementations. The adapter restricts
memory capabilities but is not a full native-XGMI behavioral emulator.

Seven new typed fixture groups cover:

1. Original-order complete profiles, mixed/swapped/missing/aliased bindings.
2. Wrong endpoint, foreign root, model generation, extent and retained charge
   rejection before table reservation, ID advancement or operation callbacks.
3. Invalid typed rosters, including first-valid/second-invalid bindings, rejected
   before the acquisition callback. Generic in-tree tests separately check
   ordered first/second binding callbacks and cleanup on second-binding failure.
4. Complete-profile rejection on unhealthy sessions with independent healthy
   selected-leaf behavior. This does not demonstrate concurrent native sealing.
5. Context rejection before credit/callbacks for unsupported kind or invalid
   geometry, classified-capacity refund, two-endpoint charges and exact release.
6. Terminal and panic quarantine retaining the selected request/root after all
   ordinary owners leave scope, without debiting the other endpoint.
7. Root lifetime through Context, returned synthetic backend and final clean Drop.

Clean release restores exact pre-allocation root usage, including its existing
bootstrap record; leaf request usage and reservation/retention counters return
to zero. The inherited 14 single-device/generated fixture groups are also run.

`check-overlay.sh` compares every copied Cargo/toolchain/crate/example input,
rejects nonordinary filesystem nodes and checks exact file rosters. Each
production-source difference must match the recorded patch (apart from hunk
line numbers); each fixture must match its source bytes. Inherited fixtures are
pinned by `qualification/inherited.sha256`. This is development source-closure
evidence, not an independently authenticated hardware or proof attestation.

## Results

| Gate | Result |
| --- | --- |
| Production XGMI focused filter | 191 passed |
| Runtime all-feature library suite | 1,505 passed, 3 failed, 28 ignored |
| Runtime doctests | 50 passed, including the public composed constructor signature |
| All-feature/all-target Clippy, `-D warnings` | Passed |
| No-default-feature check, formatting and whitespace | Passed |
| Isolated typed qualification before mutation | 21 passed (14 inherited, 7 new) |
| Witness-authentication mutation | Two intended assertion failures, exit 101 |
| Exact source/fixture restoration checks | Passed |
| Restored-source isolated qualification | 21 passed; executable hash matches pre-mutation build |

Counts overlap and must not be added as distinct-test totals. The three library
failures are `authorized_execution::tests::{cooperative_debug_telemetry_emits_only_bounded_logical_records,
failed_session_end_is_explicit_and_terminal, pre_native_telemetry_failure_is_returned_and_poisoned}`:
each reports `InspectSocket` with `Operation not permitted`, matching the
parent's known environment failures. This is not a fully passing CPU suite.

The mutation removes only the selected witness guard in the isolated production
policy. It compiles successfully and is rejected by the wrong-witness and
unhealthy-selected-session assertions. `qualification/mutation.patch` records
the change; `raw/mutation-observed.patch` includes the existing test-module
overlay. The shipped tree is never mutated. Executable/source hashes, commands,
statuses and restoration checks are in `raw/`.
Large intermediate file lists/hash-check logs are archived with deterministic
gzip compression; `raw/sources.sha256` remains directly checkable. Replay
scripts regenerate the uncompressed intermediate files.

Initial results are retained in `initial/`. Two source-wiring tests initially
expected terminal checks inside `require_live`; they were updated to assert
its call to the shared health helper and the helper's original terminal checks.
The final full suite has only the three known environment failures above.
Initial narrower source closure and earlier fixture versions are not the final
replay evidence. The final fixture review found no remaining blocker within
the explicitly limited CPU scope.

## Remaining Gates

- Actual checked-device constructor replay, ordered partial-admission cleanup,
  both native endpoints' allocation/copy/release, peer mapping, queue bootstrap
  and native shutdown still require MI300X execution.
- Worker request transport, generated multi-device execution, native failure
  campaigns, unified compute and complete memory closure remain open.
- No new Verus proof is executed here. The model, accounting and lower KFD
  production trees are unchanged. Production Context/backend/ledger refinement,
  concurrent sealing exclusion and matched HIP/HSA measurements remain open.
- Fresh SSH fails resolving `sharkmi300x-1`; issue retrieval fails connecting to
  `api.github.com`. No remote artifacts were created or native results inferred.

## Reproduction

```sh
bash docs/evidence/dev-runtime-xgmi-request-2026-09-26/validate.sh /tmp/fe2o3-owned-target
bash docs/evidence/dev-runtime-xgmi-request-2026-09-26/qualify.sh /tmp/fe2o3-unused-copy
```

Use unused owned paths. Remove only those paths after all processes finish.
This run removed the exact owned target (551,032 KiB) and isolated source/target
tree (514,152 KiB) after all build/test handles were terminal. Both paths were
confirmed absent; no unrelated temporary files were removed.
The three known telemetry socket tests require an environment that permits
local authenticated socket setup; their failures are not passing qualification.
