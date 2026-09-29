# Native Aggregate Cleanup Checkpoint

Later progress: [fresh user-namespace mapping and actual Linux validation](native-user-namespace-20260928.md).
The results below remain the earlier aggregate-cleanup checkpoint.

This checkpoint advances [#272](https://github.com/harsh-nod/fe2o3/issues/272)
after the [retained-runtime controller work](retained-runtime-controller-20260928.md).
**M0-M7 and the 47/47 production-to-safe-GPU-launch gate remain incomplete.**
No proof execution, kernel qualification, GPU run or namespace-isolation claim
is added by this checkpoint.

## Implemented Boundary

The existing native protected-service spawn now has an optional unsafe
`spawn_retaining_in_fresh_domain` operation. It creates a fresh root-controlled
cgroup v2 and uses the same `clone3` path with `CLONE_INTO_CGROUP`; there is no
post-spawn migration window or second reaper.

- The original funded slot owns rollback before mkdir. No existing path or
  caller-supplied descriptor can construct a domain owner.
- Successful clone transfers the domain, atomic pidfd, artifact-spawn lease
  and original slot before any fallible parent check.
- Failed clone or interrupted setup can retain a domain-only cleanup record.
  Unknown creation results quarantine rather than adopt or remove a stale path.
- Root terminal status and aggregate completion are distinct. After root reaping,
  cleanup does not signal or wait that root again. Descendants still prevent
  retirement of inputs, capacity and original accounting.
- Complete disposal requires the exact root consuming wait, aggregate emptiness
  and removal of the retained domain. Verified exec independently discharges
  only the inherited artifact-spawn obligation.
- Every cleanup turn is finite and uses original prepaid work and storage.
  No best-effort destructor, PID census, kill result or renewed budget proves
  successful cleanup. Syscall and mutex latency are not wall-clock bounded.

The cgroup `populated` field covers descendants, but excludes zombies; it cannot
replace the direct-child consuming wait. See the
[Linux cgroup v2 lifecycle specification](https://docs.kernel.org/admin-guide/cgroup-v2.html).

Implementation: [spawn](../../crates/fe2o3-protected-service-spawn/src/native_domain_spawn.rs),
[domain](../../crates/fe2o3-protected-service-spawn/src/native_cgroup.rs),
[cleanup](../../crates/fe2o3-protected-service-spawn/src/process_cleanup.rs).
Default spawns remain domain-free. Shared cleanup quota constants now conservatively
fund domain operations; callers must use the current quota queries/constants.

## Verification

Final tested code: `2c0cfc941f9950f610346bc1b9cb35d9416dd90d`.
The following guards used the same 9,196-file snapshot:
`575fa0ef8edc60ba197de0450de340ae8173b99409321438768aed64f322ab4d`.
The pinned nightly was `nightly-2026-04-03`; builds were serial, locked and offline.

| Check | Result | Evidence label |
| --- | --- | --- |
| Spawn library | 144 passed; root diagnostic ignored | `domain-spawn-lib-rfive` |
| Compiler coordinator | 180 passed | `domain-callers-lib-final` |
| Compiler issuer | 19 passed; 2 ignored | `domain-callers-lib-final` |
| Anchor coordinator | 78 passed | `domain-callers-lib-final` |
| Spawn doctests | 29 passed | `domain-final-policy` |
| Unsafe inventory | 5 passed; maintenance command ignored | `domain-unsafe-inventory-final` |
| Authority, closure, verifier, Cargo and backend all-target check | Passed | `domain-integration-check-final` |
| Static musl diagnostic build | Passed | `domain-spawn-musl-rtwo` |

The delta hygiene check against `f89ef45c1`, scoped formatting and whitespace
checks passed. Test-only accessors were moved into the existing test module;
no hygiene waiver was added. Reviews found and corrected a diagnostic gap:
the test now independently checks descendant termination and domain removal.

### Actual Linux Run

The explicitly selected ignored test
`native_spawn::domain_spawn::tests::root_exit_does_not_retire_live_descendant_domain`
passed on MI350-2, Linux `5.15.160+`: **1 passed, 0 failed, 0 ignored**.
The final-candidate run took 0.04 seconds inside the test, with empty test stderr.

The static fixture execed after the normal credential/profile gate, forked a
descendant and exited. The test observed the descendant alive after root exit,
checked that both belonged to a fresh domain distinct from the parent, and then
required descendant pidfd termination, domain removal and input destruction.
An outer diagnostic custodian independently consumed the test's zero exit and
the adopted descendant's SIGKILL exit, observed aggregate emptiness, and removed
its private outer domain and temporary files. No shared installation or global
security setting was changed.
An independent final SSH check confirmed absence of both candidates' private
directories, outer/leaf cgroups and the three previously cleaned scratch roots.

| Final-candidate object | SHA256 |
| --- | --- |
| Static test binary, local and remote pre/post | `7d30b749660195e3586da614fb809a40a79461e735480075698c51e0ca0f50a7` |
| Static fixture source | `e441aedaba1548c7392d0dee38ea16d58fdc4e77d63ea0081e148246f0d38ad9` |
| Reviewed diagnostic driver | `cbe7e55d18ff0cc2008de3a2c34a32408d3708b9c5aa65bdc83c484d8a197ab8` |
| Complete remote JSONL report | `59e0c07297d20001df590f7123463e2ce7f060e20d45358c75578ff21bd5af32` |

An earlier candidate (`4a0b07967`) also passed; its separate report is not
relabeled as final-candidate evidence. The only intervening source change moved
test accessors. Pure failure schedules remain separate from actual Linux evidence;
not every mkdir, clone or cleanup failure was kernel fault-injected.

## Remaining Work

This is mechanical process custody, not protected helper admission. Its unsafe
administrator caller must exclude privileged cgroup/mount writers and prevent
children from accessing controls, migrating or delegating the domain. A cgroup
does not exclude same-UID memory writers or establish trusted namespace origin.

Next: implement root-created user-namespace mapping and admission through this
spawn path; qualify paired memory-access controls; compose the authenticated
proof-helper bootstrap and retained runtime; then validate protected proof and
safe GPU launch through the one production compiler. The child dumpability
exception and Cargo runtime-enforcement activation gates remain disabled.
