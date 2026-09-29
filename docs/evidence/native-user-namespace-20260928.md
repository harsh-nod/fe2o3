# Native User Namespace Checkpoint

Date: 2026-09-28 (Pacific). Continues the
[aggregate cleanup checkpoint](native-domain-cleanup-20260928.md).

Later results: [credential-bound bootstrap and five paired memory diagnostics](native-bootstrap-isolation-20260928.md).

**M0-M7 and the 47/47 production-to-safe-GPU-launch gate remain incomplete.**
This validates namespace creation, profile installation and aggregate cleanup,
not protected proof execution, memory-writer exclusion or a GPU launch.

## Implemented Boundary

The optional unsafe `spawn_retaining_in_fresh_user_namespace` uses the existing
native staging, reserved cleanup slot, artifact-spawn lease and cgroup clone path.
It is not another launcher or a public safe proof-isolation constructor.

1. Before clone, prepare fixed root/helper/peer identity maps and retain the actual
   parent user, PID and time namespace handles. Reject pending PID/time transitions.
2. Clone with atomic pidfd, fresh cgroup placement and `CLONE_NEWUSER`. The child
   arms parent-death protection before blocking on a separate mapping gate.
3. Adopt namespace, domain, child, lease and cleanup reservation together before
   any fallible parent-side map operation. Validate the actual child pidfd, nsfs
   type, distinct user namespace, exact parent ancestry and root namespace owner.
4. Write each initially empty ID map once and verify its complete structured
   readback. Keep `setgroups=allow` so the existing profile can empty groups.
   PID and time namespaces remain shared with the parent.
5. Release the mapping gate; install and validate the existing dropped profile.
   The original profile-ready and final exec gates remain separate.
6. Retain every partially configured namespace handle through actual root reaping
   and aggregate domain retirement. Uncertain custody retains storage/capacity.
   `ECHILD` during namespace validation immediately records lost wait ownership.

Original request and persistent cleanup accounts prepay the additional work,
fixed buffers and 13 retained descriptor obligations, including deferred closure.
Logical work limits do not bound kernel syscall latency or process RSS.

Review also removed libc cancellation-point I/O wrappers from the post-clone
child path. The production mapping-gate sequence is exercised with valid tokens,
invalid tokens and EOF in a deadline-bounded subprocess.

## Verified Candidate

Code commit: `c47c32426cb0c497a62ee49f2f55364cb5bc12c7`.
All final guarded runs used the same 9,202-file source snapshot:

```text
3576fd673f018d4113e19d38bbb26d48227292a69e0a719be7ff8ce1483cc637
```

The pinned nightly is `nightly-2026-04-03`; runs use locked/offline dependencies,
one build job, serial tests, fixed resource limits and before/after source/tool
hashes. Existing warnings remain; no warning-free claim is made.

| Check | Result | Evidence Label |
| --- | --- | --- |
| Spawn unit tests | 163 passed; 2 root diagnostics ignored locally | `namespace-spawn-lib-rthree` |
| Compiler coordinator / issuer / anchor | 180 / 19 / 78 passed; 2 issuer diagnostics ignored | `namespace-callers-lib-rfour` |
| Spawn doctests | 30 passed | `namespace-spawn-doc-rthree` |
| Unsafe source inventory | 5 passed; maintenance command ignored | `namespace-unsafe-policy-rthree` |
| Build authority, closure capability, verifier, cargo driver, codegen | All-target checks passed | `namespace-integration-check-rthree` |
| Static Linux diagnostic binary | Build passed | `namespace-spawn-musl-rthree` |

Total: 475 passing local Rust tests, five locally ignored tests. Delta hygiene
against `a666b88f2`, its eight policy tests, scoped formatting and whitespace
checks passed. An oversized test fixture was split into a focused test module;
no policy waiver was added.

## Actual Linux Run

The root-only namespace diagnostic ran on `mi350-2` using private scratch and
a fresh outer cgroup. The final candidate passed **1 test, 0 failures, 0 ignored**.
It independently checks exact ID maps, dropped IDs/groups/capabilities, shared
PID/time namespaces, a different user namespace shared by the child and its
descendant, live descendant custody after root exit, descendant termination,
domain removal and exactly-once retained-input release.

The first candidate failed with `ESPIPE`: proc ID-map controls reject positioned
writes. Its child was reaped and scratch/cgroup state removed. The fix uses one
ordinary write on each freshly opened map control; preceding `pread` checks do
not advance its offset. A nonseekable-control regression test covers this error.
That failed run remains separate evidence and is not relabeled as success.

Final binary SHA-256 (5,400,400 bytes):
`31a36021b83d45ef88a2bc018c397c0c174163c339a508fdee4472ae32089d8d`.
Final controller JSONL SHA-256:
`cd5abffdd1a1baca0cf542b3338db1428d12214e07fed865a74f66e97987830a`.

The controller consumed test PID 1699918 with status 0 and adopted descendant
1699921 with SIGKILL status 9. It recorded aggregate-empty cleanup and removed
its private directory and outer cgroup. A separate read-only SSH check confirmed
all 13 tracked current/historical temporary paths absent at
`2026-09-29T00:53:10.242Z`. The fast test removed its leaf before the controller
sampled it; no independent controller-observed leaf identity is claimed.
No host security settings or shared protected-runtime installation were changed.

## Remaining Gates

Numeric root, namespace metadata and this diagnostic do not establish trusted
administrator origin or a complete proof-process isolation boundary. Remaining:

- Paired successful baseline/isolated-denial tests for all five external memory
  access paths; mapped peer-credential transport and adversarial identity cases.
- Immutable backing and exclusion of preopened/shared writable aliases.
- Closed authenticated helper/compiler bootstrap, exact compiler invocation,
  separate exec-status/bootstrap channels and one consuming-wait owner.
- Whole-lifetime controller integration, including the actual cloning thread.
  Linux parent-death signaling follows that thread; TGID readback alone does not
  enforce its lifetime. This remains an explicit unsafe caller obligation.
- Production proof/receipt admission, safe launch and all 47 end-to-end GPU runs.

Production activation and the proof-child dumpability exception remain disabled.
