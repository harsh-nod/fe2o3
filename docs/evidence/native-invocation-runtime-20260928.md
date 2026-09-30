# Exact Invocation and Runtime Accounting Checkpoint

Date: 2026-09-28 (Pacific). Continues the
[native proof helper bootstrap checkpoint](native-proof-helper-bootstrap-20260928.md).

Next checkpoint: [native compiler exec and capture](native-compiler-exec-20260928.md).

At this September 28 checkpoint, M0-M7 and the 47/47 production-to-safe-GPU-launch
gate remained incomplete. The September 30 follow-up below records newer status.
No tutorial entry changes classification. This checkpoint adds exact invocation
custody, approved helper configuration and bounded runtime admission, not an
integrated compiler/helper attempt, proof RPC or GPU execution.

## Implemented

- Fixed rustc and ELF-interpreter transfers use the existing retained compiler
  inventory. Each duplicate has a separate full backing charge. Validation
  checks the original object, complete inventory, approval and original budget;
  matching bytes or a caller-selected role/path cannot substitute for custody.
- Exact V3 invocation staging preserves argv order, repetitions, empty arguments,
  argv[0] and the complete sorted environment without inheritance or lossy
  conversion. Capacity measurement, byte/count checks, allocation and overlapping
  owners are prepaid. Cwd remains descriptor text, not an admitted directory.
- Private `CompilerInvocationBacking` consumes the exact descriptor and one
  genuine runtime, retaining both executable transfers and staged inputs.
  `ProofHelperBacking` now consumes that compiler backing, rather than competing
  for the same runtime. Full compiler revalidation surrounds sealed-helper
  validation. Neither owner provides an execution entrypoint or public import.
- Compiler approval V2 authenticates explicit helper UID/GID values and rejects
  zero, invalid or colliding service identities. The sole positive approval
  loader and native authority launcher select fixed `policy-v2`, bound to the
  existing V3 client profile. The V1 codec remains inert; there is no V1 approval
  fallback. Helper startup and root-side staging derive credentials from this
  retained approval, never from caller arguments or the helper's observed IDs.
- Helper-local Verus admission and revalidation now use its existing resource
  budget. Charges include complete retained files, interpreter backing, metadata,
  bounded scans and temporary reopen overlap. The original ledger and Budget
  address must agree. Bounded owners reject legacy unmetered revalidation,
  attempt acquisition and execution dispatch before those operations run.

The root-side components are **not yet called by the real per-compiler attempt**.
A pinned interpreter FD does not establish dynamic-loader or DSO resolution.
Configured role credentials do not establish deployment provenance or an
independent whole-domain custodian. The installed V1 provisioner/systemd graph
does not provision this helper role or publish V2 compiler approval; migration
must update those consumers together. Cross-process original-root-account
metering and bounded proof execution remain unfinished.

## Local Validation

Code commit: `2f234ba0cc06a4e941f6c500cee719ef4050ab63`. It includes the separately
published MIR fix `3991a6d6cc350f856038d813674cd1e63b617adb`, merged without
replacing upstream history. The final guarded runs used the same 9,237-file
snapshot:

```text
c60e865ae181cfe8131e574d2ed4576af8613155e093c12e9202250d05be7e2b
```

Pinned `nightly-2026-04-03`, locked/offline dependencies, one build job, serial
tests, fixed resource limits and unchanged before/after source/tool hashes were
retained. Results are scoped, not workspace-wide or warning-free validation.

| Check | Result | Evidence Label |
| --- | --- | --- |
| Compiler closure / coordinator / rustc invocation unit tests | 263 / 217 / 99 passed | `attempt-final-core-rtwo` |
| Build-authority integration suites, including both policy codecs | 55 passed | `attempt-approval-final-rone` |
| Build authority / closure / coordinator / invocation doctests | 221 passed | `attempt-final-doc-rtwo` |
| Retained Verus runtime and process-control tests | 123 passed; 8 ignored locally | `attempt-runtime-resources-rthree` |
| Helper entry I/O tests | 4 passed | `attempt-helper-entry-rone` |
| Merged nominal V35 MIR tests | 16 passed | `attempt-mir-nominal-rone` |
| Unsafe source inventory | 5 passed; maintenance command ignored | `attempt-unsafe-policy-rone` |
| Cargo driver, codegen, verifier, runtime protocol and coordinator | All-target checks passed | `attempt-integration-check-rtwo` |

Nested subprocess results are not counted twice. The protected-runtime test
below is one of the eight locally ignored tests, executed separately, not eight
additional passes. The authority-launcher shell suite also passed. Scoped
formatting, whitespace, all eight hygiene-policy tests and delta hygiene against
`15dd803a2` passed without a waiver. New credential tests were split into a
focused module to retain the file-size limit.

Tests cover exact invocation bytes and capacity charges, wrong roles/inodes,
policy downgrade and identity collisions, missing/short reservations, changed
budgets, sealing and transfer overlap, and refusal before unmetered dispatch.
They do not manufacture a public positive approval owner or prove a successful
composed production launch. An initially overbroad fixture iterator, an empty
policy test filter and the legacy-access gap were corrected before these runs.

## Real Protected Runtime

On `mi350-2`, the actual ignored test
`retained_functional_refinement_runtime_v1::resources::tests::protected_runtime_retains_complete_backing_on_original_budget`
passed: **1 passed, 0 failed, 0 ignored**. It opened the fixed root-protected
runtime, checked complete retained charges, revalidated the genuine owner,
rejected missing storage and foreign/moved budgets, rejected legacy access,
and dropped custody before releasing storage.

The test container was configured for a non-root process in the pinned read-only
image, no network, all capabilities dropped, no-new-privileges, one CPU, 2 GiB
memory and 32 PIDs. Shared runtime and interpreter directories were mounted read-only.
The first probe's individual loader-file mount correctly failed the retained
parent's `NO_XDEV` check. Mounting its containing protected directory fixed the
test environment; no runtime safety check or manifest was changed.

| Identity | SHA-256 |
| --- | --- |
| Pinned container image | `fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f` |
| Executed verifier test binary | `36aece4ec87bf62b62be59ff165169969993b396c2a238ec7c6ae9fa7061343b` |
| Corrected remote probe driver | `21c572d1f551c332316cb7dcbb239ebf6728d49c3659e30112a7dfc5a937d415` |
| Successful remote JSONL | `8224f8dd0da7db91d635cb36dceb17e91ce2606622fde5d96a1d60f8644d0ec4` |

The final local snapshot's verifier test binary was independently checked to
match the executed bytes. Both probe containers and private scratch directories
were removed, and a separate SSH check confirmed their absence. Shared images,
volumes and installations were preserved. Evidence is retained under
`attempt-runtime-probe-rone`, `attempt-runtime-probe-rtwo` and
`attempt-runtime-probe-cleanup-20260928.json`.

This is **static protected-runtime admission/accounting evidence only**. No
proof child, proof receipt, authenticated helper launch or GPU workload ran.
The optimized static helper image from the preceding checkpoint was not rebuilt
in this batch and is not evidence for the new source snapshot.

## Next Integration Gates

1. Extend the existing native exec stage for actual rustc argv/environment,
   directory-object cwd, stdio, backend/output/input descriptors and validated
   loader mappings. Do not introduce a second compiler launcher.
2. Retain actual compiler/helper siblings through one attempt, with separate
   cleanup domains and one consuming wait owner per child. Integrate tracing
   before releasing the compiler's pre-exec gate; pending cleanup retains all
   transitive backing and spawn obligations.
3. Migrate provisioning and installed entrypoints coherently, establish actual
   deployment provenance/custodian admission, bind the authenticated attempt
   commit and exclude peer writers and backing aliases.
4. Add bounded proof RPC and execution through the helper-local lease, resume
   the same compiler, and finish original-account metering. Existing production
   authority refusals remain until their exact obligations are implemented.
5. Complete generic safe launch, the 47-kernel positive/negative matrix,
   target-matched hardware validation, tutorial updates and release gates.

## September 30 Sealed Invocation Follow-Up

Code commit `f37e4d097` replaces the detached descriptor input to
`CompilerInvocationBacking::prepare` with the received sealed
`RustcInvocationCapabilityV1`. Preparation and revalidation retain and check
that same capability. Staging borrows its decoded descriptor; there is no
second descriptor clone, decode or encode path. The original invocation FD,
decoded allocations, canonical bytes, runtime and output owners stay accounted
for together. This remains private backing preparation, not cargo authorship
or permission to execute a compiler.

Every capability constructor now caches checked decoded allocation capacity.
The native retained-storage check includes the entire owner, canonical vector
capacity, sealed image bytes and result envelope. A legacy-created descriptor
with excessive spare capacity is rejected against the existing native bound;
equal canonical bytes do not make its actual allocations disappear. The backing
requires the full received FD charge plus native admission growth, not just the
decoded descriptor charge. Consuming failure closes the inputs but does not
refund the caller's reservations or erase work/denial history.

Guarded tests ran on `09105af9f65cb6f23b620dea48d5c5f3eab49c29`, pinned
nightly-2026-04-03, locked/offline, one Cargo job and serial tests:

| Run | Result |
| --- | --- |
| r126 closure-capability library | 288 passed, 4 ignored |
| r126 execution-coordinator library | 280 passed, 14 ignored |
| r127 both crates' doctests | 222 passed, none ignored |
| r128 both crates, all-target Clippy | Completed with warnings; not a strict warning-clean gate |

Nested subprocess results are not counted twice. The eight installed-runtime
backing tests remain ignored: their genuine fixed-origin approval/runtime inputs
were not provided by these local runs. Real sealed-file mechanics and synthetic
descriptor text do not substitute for those prerequisites. No compiler, proof
helper or GPU workload was launched, and no remote host was modified.

New passing controls cover every construction path, excessive decoded and
canonical capacity, exact/full-owner and one-byte-over boundaries, original FD
identity, FD-plus-growth accounting, consuming closure, and preserved ledger,
budget address, peaks and denial history. Scoped rustfmt, whitespace, DCO and
the repository hygiene delta policy also passed. No safety guard or production
approval constructor was broadened.

All three guarded runs retained the same 9,498-file source snapshot:
`931d99bbd463dcf8ffbb0420512cd163ed2747b4989138e4bd1991eeaee2ee22`.
Log SHA-256 values:

- r126: `d51832452b4a67982b2dbb2e961982ff1eca490eaf4b41f6084b78c32d30d8e0`
- r127: `cb5cc6061d2bf690cf33a37ab636497e99ba583d2e8117e4a09a735e3167bb96`
- r128: `25db5f102ac72430ea5934e6a1edc6d7e4f292679b435d3bd74ea3a9c04af9a1`

The root intake still does not produce this backing through its real request
path. That consuming integration, continuous code/source/output enforcement,
original compiler completion and protected proof execution remain required.
V4 intake remains refusal-only. For issue #272, M0 is now complete; M1-M7 remain
incomplete and the strict production-to-safe-GPU-launch matrix remains **0/47**.

## September 30 Original Root Request Integration

Code commits `47f393561` and `d67ec80e5` integrate backing preparation with the
real root-request intake. The original authenticated request is retained before
fallible preparation, then consumed using the fixed-origin production approval
and runtime constructors. The received sealed invocation, typed output, original
budget and request rights are not reconstructed through a second authority path.
Only backing growth and the new owner envelope are reserved in addition to the
already retained request charge. Failure consumes the turn; cancellation and
draining retain the original backing and rights until their owners retire.

This supersedes the preceding checkpoint's missing root-intake connection, but
does not activate compiler execution. The next turn still returns the existing
V4 `RuntimeEnforcementUnavailable` refusal. Continuous code/source/output
enforcement, actual compiler completion, proof RPC and safe GPU launch remain
unfinished. No success acknowledgement or production safety check was weakened.

The new ignored root-request process matrix obtains its owners from genuine
fixed-origin V3 provisioning records, installed image measurements, approval and
runtime inputs. It no longer depends on synthetic test keys or a hardcoded test
profile. It covers consuming preparation/refusal, short or foreign accounting,
moved same-ledger budgets, trailing requests, unwind and retained descriptor
identity. These cases are implemented and compiled, not executed in this batch.
Running them requires genuine provisioning in a disposable isolated environment
with an external whole-cgroup custodian, bounded resources and deadline,
descendant termination/reaping, and private-mount cleanup. Environment opt-ins
alone do not establish that isolation. The unwind exit assertion is not evidence
of descendant or mount cleanup.

Guarded local validation used `d67ec80e5a5b84236a2af88bb36c9b96ffcc559c`, the
pinned nightly, locked/offline dependencies, one Cargo job and serial tests:

| Run | Result |
| --- | --- |
| r133 closure-capability library | 288 passed, 4 ignored |
| r133 execution-coordinator library | 282 passed, 16 ignored |
| r134 both crates' doctests | 222 passed, none ignored |
| r135 both crates, all-target Clippy | Completed with warnings; not warning-clean |

Nested subprocess results are not counted twice. The total library result is
570 passed and 20 ignored. The genuine installed matrix remains ignored; no
compiler, proof helper or GPU workload ran. All three runs retained the same
9,504-file source snapshot and unchanged tools. Whitespace, DCO and the existing
delta hygiene policy passed for both code commits.

Source snapshot SHA-256:
`15e4f54db07d5a4d5413a1167b22b27801c8e0ca020483754b60237cfad03852`.
Log SHA-256 values:

- r133: `446b814a8e4750641d64d0f27f71e0778e1ea769ce737c132d6fb06e94010d59`
- r134: `aa4ad7f3d00ce663fd4462eabb6b80e71ea2dbf05836f7ea65b5d2cac1d356ca`
- r135: `dd70cb8670f233f78090ea707e8d2f6c1bacfa3577d425d4b6afae84a8bc0347`

A later bounded read-only SSH probe reached MI350-2's Conductor authorization
step but timed out before returning filesystem or image results. The earlier
host observations above are historical, not reconfirmed by that probe. No remote
job, container or scratch directory was created. M1-M7 remain incomplete and
the strict end-to-end count remains **0/47**.

## Subsequent September 30 Host Observation

A later primary-session read-only check reached MI350-2. `stat` reported these
host paths absent:

- `/etc/fe2o3/compiler-execution`
- `/etc/fe2o3/compiler-execution/policy-v2`
- `/etc/fe2o3/compiler-execution/compiler-runtime-manifest-v1`
- `/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5`

`docker image inspect` returned the retained linux/amd64 image identity
`sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f`.
This establishes image presence only, not provisioned root-request execution or
proof-runtime readiness. No container, remote file, installation change, proof
execution or GPU workload was created by these observations. Runtime integration
and its genuine provisioned tests remain necessary.

## Approval Paths and Retained Volume

A subsequent September 30 read-only SSH check reached MI350-2 and inspected the
actual build-authority paths, distinct from the earlier compiler-execution
provisioning paths. `stat` reported all three absent:

- `/etc/fe2o3/build-authority/policy-v2`
- `/etc/fe2o3/build-authority/compiler-runtime-manifest-v1`
- `/opt/fe2o3/compiler-runtime-v1`

`docker volume inspect` confirmed that
`fe2o3-authoring-proof-native-20260917-r1` exists. This does not establish its
current file contents, pin integrity, compiler approval or readiness for the
original-root request matrix. Earlier proof-runtime observations cannot be
substituted for those checks. These probes created no container, remote file,
proof job or GPU workload.

A follow-up read-only inventory returned the retained `runtime` directory as
root-owned mode 0555, its installed manifest as root-owned mode 0444, and the
separate interpreter directory with a loader and eight DSOs. The installed
manifest SHA-256 matches the repository pin:
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
This observes the manifest only, not every payload hash, immutable flags,
canonical loader resolution, writer exclusion or production admission. No
remote installation or execution was performed.

## Retained Volume Payload Audit

At `2026-09-30T10:40:47Z`, a bounded read-only SSH audit checked the complete
retained input volume against the local pinned manifest and target-file list.
The single SSH invocation finished six seconds later with exit 0; its
connection and overall limits were eight and ninety seconds.

- Both pin-stream hashes matched before the 102 pin rows were interpreted.
- The exact runtime roster contained 81 files and nine directories; the
  interpreter roster contained the loader and eight libraries.
- All 90 files (657,248,772 bytes) matched their pinned SHA-256 values.
- Owners were 0:0, regular-file link counts were one, and modes matched the
  pinned layout. No unexpected entries or group/other-writable objects appeared.

This establishes reusable pinned input bytes, not protected runtime admission.
Immutable flags, xattrs, mounted interpreter links, writer exclusion, compiler
approval and genuine execution remain separate checks. No remote file was
written and no container, mount, installation or proof/GPU job was created.

Log: `native-retained-volume-pin-audit-20260930T104047Z-2176959.log` in the local
evidence directory. SHA-256:
`18fa62c6d07a851ccd2dccf9a73709b950f876f473730cd47672a9d204336817`.

## Native Custody Library Rerun

r157 at `5ef50c9e7c3730250fbd6303a3181c1310a3737b` ran both complete
coordinator and protected-service-spawn library suites, with one Cargo job,
serial tests, locked/offline dependencies and disabled GPU visibility:

| Library | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| Compiler execution coordinator | 287 | 0 | 17 |
| Protected service spawn | 271 | 0 | 16 |

The 33 ignored tests include the genuine original-root request, native compiler
execution, mapped-child confinement and cgroup cleanup matrices. They remain
unexecuted, not qualified by the 558 passing library tests. The systemd source
contract check also passed; it does not execute the service unit.

This candidate retains the original request through helper-backed preparation
and a compiler child behind a closed exec gate. The added channel fault hook is
test-only and observes the authenticated child channel before trace seizure.
Namespace confinement is mandatory for compiler children and mapped helpers;
unmapped service creators retain their separate role. The compiler gate stays
closed and the production response remains `RuntimeEnforcementUnavailable`.

Source and tool inventories remained unchanged. Log SHA-256:
`7eb8bbd02cfc3d6590050309822ed5a22ec9fbdcca3395ec590f628fd648b075`.
The public integration applies the four new signed commits on top of their
already-published signed prerequisites. Its code tree exactly matches this
tested candidate; only evidence documents differ. Provisioned deployment,
protected proof execution and GPU validation remain outstanding.

## Ordinary-Domain Admission

Candidate `57b5ae08d470244cbe829f727f82060eeba27c67` binds the creator's
unit to its exact writable cgroup subtree. Only the final membership component
may cross that mount; prefix and control-file opens retain `NO_XDEV`.
The resolved parent must expose the exact protected `domain\n` type record
before its PID list is used as direct-membership evidence. Threaded domains,
invalid domains and hierarchy-root membership refuse. These are native
admission requirements, not a substituted source configuration assertion.

| Run | Result |
| --- | --- |
| r169 | Compile failure: two ignored mount-fixture helpers lacked scoped unsafe-code lint exceptions; no tests ran |
| r170 | 561 passed, one failed, 35 ignored |
| r171 | 562 passed, none failed, 35 ignored |

The r170 failure was the coordinator's old exact capability-list assertion.
It now includes the existing unit's `CAP_SETFCAP`, needed by the root UID-map
creator. No runtime predicate or service setting was relaxed by this test fix.
r171 ran the complete coordinator (287 passing, 17 ignored) and spawn (275
passing, 18 ignored) library suites. The systemd source-contract and mutation
script also passed. The production cgroup module still denies unsafe code;
only the two isolated mount-fixture functions have explicit lint exceptions.

All runs retained unchanged source/tool inventories. Log SHA-256:

- r169: `49078a9621aee9274a719aeff86f72127dd8abe41c063317547a94d130e2ebc6`
- r170: `02a1e75ef178c9fc99041a0681a8454d2b8329078da94a623795b07c9cd8afe1`
- r171: `91f3fed080f782893dc7afd1c31fc208fdb02fd33fd2a018b20c7c0df8af9607`

Public integration adopts the four signed commits atop their already published
prerequisites. Its crates, lockfiles, scripts and deployment tree exactly match
the tested candidate; evidence documents differ. Native mount controls,
threaded-domain controls, effective-unit startup and protected compiler/proof
execution remain unrun. The separate locked-personality observation candidate
is not included. M1-M7 and strict safe-GPU launch coverage remain **0/47**.
