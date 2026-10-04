# Native Proof Helper Bootstrap Checkpoint

Date: 2026-09-28 (Pacific). Continues the
[bootstrap and isolation checkpoint](native-bootstrap-isolation-20260928.md).

Follow-up: [exact invocation custody, V2 helper approval and real protected-runtime accounting](native-invocation-runtime-20260928.md).

**M0-M7 and the 47/47 production-to-safe-GPU-launch gate remain incomplete.**
No tutorial kernel changes classification. This checkpoint implements helper
startup, shutdown and private root-side custody, not proof RPC or an integrated
production compiler attempt. Native enforcement refusals remain in place;
executor replies remain inert.

## Implemented

- A dedicated static `fe2o3-proof-executor-helper` enters through
  `fe2o3_secure_start_v1`. Its sole inherited channel is FD 3: an unnamed,
  nonblocking Unix SEQPACKET with credentials enabled before clone. Raw cleanup
  custody precedes fallible validation; unrelated descriptors are closed. There
  is no command-line/environment configuration, key or publication authority.
- The helper independently opens fixed production approval and compiler
  inventory, checks its sealed running image against the unique helper entry,
  and revalidates process and namespace observations. It opens a fresh,
  process-affine Verus runtime lease after exec at the fixed protected path and
  compares its identity with the inventory. This path has not been exercised in
  an admitted positive protected deployment.
- Initial/Ready and Finish/Finished use a fixed 88-byte inert record. Actual
  parent/child PIDs, session and runtime identity must agree. The helper opens
  its actual parent's pidfd itself; socket and per-message credentials must
  identify that root parent. Records convey no proof or launch authority.
- A private root-side backing owner retains the genuine approved runtime and
  a freshly sealed helper image. Source duplication and sealed-image transfer
  retain separate overlapping charges and validate the actual source/owner,
  not just equal bytes or a digest.
- The private launch adapter uses existing native clone/pidfd, fresh user
  namespace, cgroup and original cleanup-pool ownership. Full backing is
  retained before clone. Independent exec-status EOF precedes readiness;
  inherited writer aliases close, and actual child credentials are checked.
  Finish consumes the handshake and bootstrap EOF, then performs one prepaid
  cancellation step. Only `Reaped` means aggregate cleanup; it does not prove
  graceful exit or status zero. Pending/quarantined cleanup retains backing.
- Shared `launch_io::send_ready` bounds attempts and deadlines, meters retries
  and liveness checks, and uses `MSG_DONTWAIT | MSG_NOSIGNAL`. It sends exactly
  one bounded packet without descriptor transfers. Existing receive quotas are
  unchanged; send quotas are separate.
- The existing static-ELF parser and its tests moved into the authority-free
  `fe2o3-static-executable-format` crate. Existing runtime protocol reexports
  remain. This removes the verifier/static-image/runtime/finalizer dependency
  cycle without copying the parser or introducing another authority route.

These components are **not yet called by the real per-compiler production path**.
The current deployment profile does not approve a distinct helper UID/GID role.
Observed credentials configure confinement; they do not establish role admission,
host-root creator provenance or the outside whole-domain custodian. Those remain
explicit unsafe caller obligations, not properties proved by readiness or root IDs.

The root adapter uses its original resource account. The helper has one separate
dedicated-process ledger that is not reset between exchanges. The older retained
Verus runtime lease still uses its own finite inventory bounds, not the original
kernel-IR ledger. Complete cross-process original-account metering is unfinished.

## Validation

Code commit: `a35a27f2741e802ba1563aef26c619f06ed4c391`.
All final guarded runs used the same 9,224-file snapshot:

```text
2c0c849538aff1910d36fd521f816065666c66ca5221f51e14c62069d6fed3df
```

Pinned `nightly-2026-04-03`, locked/offline dependencies, one build job, serial
tests, fixed resource limits, and before/after source/tool hashes were retained.
These are scoped results, not a warning-free or workspace-wide validation.

| Check | Result | Evidence Label |
| --- | --- | --- |
| Coordinator / protocol / spawn / static image / runtime protocol / static format | 204 / 95 / 177 / 15 / 22 / 13 passed; 2 privileged diagnostics ignored | `helper-all-lib-rthree` |
| Same six crates' doctests | 287 passed | `helper-doc-rone` |
| Dedicated helper entry I/O tests | 4 passed | `helper-verifier-entry-rone` |
| Static musl spawn tests, executed locally | 177 passed; 2 privileged diagnostics ignored | `helper-spawn-musl-rone` |
| Unsafe source inventory | 5 passed; maintenance command ignored | `helper-unsafe-policy-rone` |
| Cargo driver, codegen, verifier, runtime protocol, coordinator | All-target checks passed | `helper-integration-check-rone` |
| Optimized static musl helper | Build passed | `helper-static-release-rone` |

The six-crate GNU unit total is 526. Musl repeats spawn coverage under that ABI;
it is not 177 additional unique test definitions. Nested subprocess results are
not counted twice. An earlier snapshot additionally passed 124 selected verifier
tests with one export test ignored; that is not a full final-snapshot verifier run.
The shell build/inspection contract passed 27 mock cases, separately from the
actual binary checks below. Scoped formatting, whitespace and delta hygiene
against `30947e280` passed without a waiver.

Tests cover exact/one-short quotas, original account association, sealed source
and backing substitution, FD disposal, malformed/authentication-mismatched records,
independent exec EOF, bounded saturated sends, real subprocess SIGPIPE suppression,
and missing/wrong startup descriptor properties. They do not manufacture a public
positive approval owner or establish an admitted root helper launch.

Recorded failed iterations include the dependency cycle, a borrowed inventory
iterator preventing ownership transfer, an incorrect SIGPIPE fixture using an
unread saturated socket (which returned ECONNRESET), and a test-file hygiene limit.
The final SIGPIPE test uses a fresh empty closed-peer socket to exercise EPIPE;
it does not weaken the expected error or suppress the signal globally.

## Actual Static Image

The optimized helper is 7,004,064 bytes. Its ELF64 ET_EXEC entry and
`fe2o3_secure_start_v1` both resolve to `0x6be200`. It has no interpreter,
dynamic section, dynamic dependencies, RPATH/RUNPATH or undefined symbols, and
its GNU stack is writable but non-executable. The runtime's extracted static
format parser accepted these exact bytes.

| Identity | Value |
| --- | --- |
| Raw image SHA-256 | `724003a26b153df448dc8ff69f0fa3970f406564ea6e24d9fb0d54a692ef711b` |
| Canonical static format identity | `afafc04fb3ad0c008d2b1a9d807b5e25242728a81f1b082c93747460b8b548c4` |
| Image-check driver SHA-256 | `2597b5ac83d566ded152bf129ba0571d2a5514fd4b3667684149af2a1fbd1a4a` |

The actual helper ran locally with an empty environment and absent FD 3: exit 1,
no signal, zero stdout/stderr bytes. Image identity was unchanged afterward.
This is a **negative startup test**, not successful protected initialization,
proof execution or GPU execution. The retained report is
`helper-static-image-check-20260928.json`.

The only new remote action was a read-only `mi350-2` uname probe. It reached the
Conductor authorization banner but did not return a uname result; its owned local
SSH process was canceled. No new remote build, upload, installation or GPU job
was started, and no new remote scratch objects require cleanup. Prior paired
Linux diagnostics remain separately reported in the preceding checkpoint.

## Remaining Production Work

1. Admit actual deployment provenance, helper role credentials and independent
   whole-domain cleanup custody through the existing production profile.
2. Stage the exact dynamic-rustc invocation, including argv, cwd, loader and
   approved inputs. The real per-compiler attempt must own actual compiler/helper
   siblings and one traced-child/wait lifecycle, not a standalone launch fixture.
3. Bind authenticated attempt/source/runtime commit and exclude every peer writer
   and backing alias before changing proof-child dumpability. The prior Linux
   mechanism diagnostics are not complete production isolation evidence.
4. Connect proof RPC and actual execution through the fresh helper-local lease,
   then resume the same owned compiler. Finish original-account metering; retain
   existing authority refusals until their exact obligations are implemented.
5. Complete safe GPU launch, the 47-kernel positive/negative production matrix,
   target-matched hardware runs, tutorial updates and release gates. Infrastructure
   tests above do not close any milestone or qualify a tutorial kernel.
