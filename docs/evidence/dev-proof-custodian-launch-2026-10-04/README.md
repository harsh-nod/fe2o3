# Fixed Proof Custodian Launch Qualification

Local CPU/root qualification for the next multi-GPU prerequisite: independently
installed, keyless proof-controller execution with retained original proof
custody and per-controller process-tree containment. This is not an application
handoff, a GPU run, complete deployment qualification, or HIP/HSA parity evidence.

## Qualified Source

- Signed source commit: `8cbd34c22c7dda60983a6a6af3d2d6ae32d8592c`.
- Source parent: `3bac2bfbd0a05b630729ab8dd345962ee057e23e`.
- Source patch SHA-256:
  `5544aa2e2904ac5382b8e79b00c445b5656b6aff35a557ecbc1062f2b9f8cd71`.
- Archive SHA-256:
  `399feec1172a557129916defcf22be4082727569b6f329c1eb07754097907076`.

`evidence.tar.gz` contains a per-file manifest, exact source and binary hashes,
the signed commit verification, build commands, isolated-root runner, captured
inputs and subjects, and complete final command logs. Source and binary hashes
were rechecked after the campaign. Binary build caches are not in the archive.

## Implementation Boundary

The new `fe2o3-proof-custodian` crate separates canonical deployment measurements,
fixed-path admission, root launch ownership, cgroup containment, private control
transport, and the fixed static child. It reuses the existing authenticated
analyzer, protected Verus runtime, retained proof producer, process profile,
secure-entry spawn, and sealed static-executable APIs.

The root opener admits only `/etc/fe2o3/proof-custodian/deployment-v1` and the
fixed `/usr/libexec/fe2o3/fe2o3-conditional-fill-proof-controller`. It retains
original installed objects, a sealed executable, current-thread namespaces and
procfs identity. The approved analyzer and Verus resources open after secure
exec in the dedicated keyless child, not inside the filtered compiler service.

A fresh root-owned cgroup receives the original gated child before gate release.
No stale cgroup name is recovered or killed. The root owner contains the tree
and reaps its direct child before releasing deployment custody. Empty-scope
polling is bounded; direct-child reap is not. Unprovable cleanup is fail-stop,
requiring an independently deployed manager-level death-containment backstop.

The private root channel authenticates packet credentials and a fresh nonce,
retains exact autobound endpoint identities, and transfers no application,
compiler-currentness, signing, or GPU authority. Proof execution has a startup
deadline; retained Probe/Release operations have fresh bounds. Once a genuine
proof exists, post-proof errors quarantine its original owners rather than
unwinding them. Only a valid Release drops the proof on the normal path.

## Results

All final commands completed with exit 0:

- 13 unit tests: seven custodian and six shared process-profile tests.
- Four compile-fail doctests, including non-cloneable custody owners.
- Strict all-target custodian Clippy, formatting and whitespace checks.
- Static musl build: no interpreter, dynamic segment, needed library or undefined
  symbol; ELF entry equals `fe2o3_secure_start_v1`.
- Independent non-root measurement of the fixed installed worker, controller and
  pinned Verus runtime inside the same library overlay used by the child.
- Eleven isolated-root cases listed below.

| Case | Observed Result |
| --- | --- |
| `good` | Actual analyzer and Verus proof, repeated Probe after deliberately expiring the root startup deadline, Release, reaped child and absent scope |
| `scope-drop` | Injected owner error removes its fresh empty scope |
| `cancel-ready` | Cancellation before proof and repeated cancellation both succeed |
| `cancel` | Live Verus command with `--no-cheating` observed before whole-tree cancellation |
| `worker` | Incorrect approved executable identity rejected as `WorkerIdentityMismatch` |
| `runtime` | Incorrect protected Verus runtime identity rejected |
| `payload` | Modified HSACO rejected as `Closure(FinalizedLengthMismatch)` |
| `controller` | Incorrect installed controller measurement rejected before launch |
| `replace` | Replacing installed configuration with identical bytes fails retained-object revalidation |
| `fifo` | A FIFO at the configuration path rejects without blocking |
| `poison` | Invalid post-proof Start rejected; controller remains live until explicit containment |

Every root case ended with its fresh outer cgroup removed and an independent
host-side census showing no live task in the private PID namespace. Child-launch
cases also checked that an unrelated sibling process survived. The poison case
tests rejection, liveness and cleanup; it does not independently inspect proof
memory during quarantine. That retention property was source-reviewed, not
established by the liveness observation alone.

The genuine compiled envelope and HSACO were reused from the prior
`dev-proof-controller-execution-2026-10-04` packet, whose archive hash is recorded.
Analyzer/proof execution and resource measurements were fresh. This campaign did
not rerun the compiler. The initial wrapper's missing `/etc/alternatives` mount
and initial Clippy failures were corrected; their diagnostic logs are retained.

## Remaining Multi-GPU Work

1. Independent root manager deployment and its complete-tree death handling,
   plus the application Ready handoff and single-consumer activation protocol.
2. Conditional native invocation bound to actual allocations, kernargs, device
   identity and first-generation submission, retaining proof/currentness owners
   until settlement across both devices.
3. Matched MI300X qualification: completed native output, settled staging/H2D,
   public XGMI in both directions, complete readback, cancellation and cleanup.

The administrator must allocate a proof UID/GID distinct from signer and anchor
roles; this opener does not admit those other deployments. Cross-service
deployment qualification, arbitrary language/kernel support, formal refinement
of the new launcher implementation, and performance parity remain unclaimed.
No MI300X work or host installation changes were made for this packet.
