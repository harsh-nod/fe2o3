# Native Anchor Peer Checkpoint

Date: 2026-09-26. Continuation of the
[durable-state checkpoint](conditional-native-durable-anchor-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open. This
checkpoint integrates native serving APIs, not protected startup, compiler
semantic equivalence, numerical refinement or GPU execution.

Base: `afab5228b652f8236bb2085301832c66394becee`.
Integrated code: `01796ff8eb515aabfbd17182d480d75f36f8f8c1`.

## Implementation

- `854cbb08b`: extracts shared peer validation and packet I/O. Native callers
  charge before every validation/poll/receive/send attempt, including retries;
  V1 supplies no-op accounting and preserves its existing transport mechanics.
- `36ec4206c`: adds nine deterministic precharge tests, including repeated poll
  readiness, denial before socket syscalls, deadline precedence and unwind.
- `01796ff8e`: integrates nominal `serve_connected_peer_v2/v3` with the actual
  deployment, native durable owners and original ledger. V1 and native owners
  share one receive/exchange/send schedule. Responses stay charged until sent
  and retired. No raw-key export, public replacement transport, or V1 upgrade
  is introduced. The consumed peer closes on every outcome.

See the [service contract](../compiler-execution-native-anchor-state.md#native-peer-loop)
for input floors, full report charges, work/peak accounting, timeout limits and
late-failure recovery. Logical work does not bound idle waits or syscall latency.
Protected peer identity, lifecycle and process admission remain caller duties.

A native worker implemented shared I/O and its tests in a private worktree;
the primary integrated native serving, shared scheduling and tests. Source review
found no concrete production defect and identified a missing send-budget refusal
case. Both families now test refusal immediately before send, with a durable
commit, no published response, exact denial, live response charge, closed peer,
restored storage, and drop/reopen recovery to Commit. Follow-up review closed that
gap. The worker ran no builds, tests, SSH or network commands and is now closed.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, one bounded Cargo job at
a time, HIP disabled, no incremental compilation, source frozen during builds.
Logs are in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Initial compile | Failed on test assertion expecting scalar `rustix::recv` result; corrected to its length pair |
| Initial native-peer tests | 14 passed, 4 socket failures; 50 filtered |
| Full service R1 | 57 passed, 13 failed; no ignored/filtered tests |
| Full service R2, with worker precharge tests | 66 passed, 13 failed; no ignored/filtered tests |
| Final full service R3, one test thread | 68 passed, 13 failed; no ignored/filtered tests; exit 101 |
| Final native tests, four test threads | 40 passed, 4 socket failures; 37 filtered; exit 101 |
| Service doctests | 4 positive and 12 compile-fail passed |
| Eleven-package all-target check | Passed with existing warnings |
| Changed Rust formatting, whitespace and hygiene delta | Passed |
| DCO through integrated code | 105 signed-off commits from `9350f2f6b73da5e249a56a297097c4919378b363`; no exceptions |

Focused runs overlap the full suite. All 24 existing native durable tests and
13 legacy durable tests pass. New passing tests cover actual-context rejection,
retry/recovery, all six service boundaries under error and unwind, response
retention, failed publication, exact/one-short quotas and input floors, cumulative
denials/peaks, repeated attempt charges, invalid public endpoint rejection,
zero timeout and checked counter overflow. The last two tests added after the
all-target/doctest runs only extend unit coverage; production code was unchanged.

The shared scheduler tests use a private scripted transport with real native
custody, filesystem state and signatures. They are not evidence of successful
Linux packet transport or protected process startup. The precharge tests exercise
actual regular-file polls, not injected socket EINTR/EAGAIN behavior.

The 13 full-suite failures consist of the same nine failures recorded in the
previous checkpoint, plus four new unignored native socket tests. Native EOF tests
fail socket-domain inspection with `EPERM`; native exchange tests fail client send
with `EPERM`. Existing failures include socket send/domain inspection, failed
response-direction shutdown, wrong-endpoint tests not reaching their expected
category, and two entrypoint errors cascading from a poisoned test mutex. This
is not a green full suite. No production checks were weakened or tests skipped.

The cross-package check covers supervisor, issuer, client, coordinator, closure
capability, anchor coordinator/service/provisioner, host, cargo-fe2o3 and backend.
Cargo target kinds are not GPU architectures. Static images, protected-root
reissue/startup and GPU launches were not executed. Prior supervisor transport
failures remain unresolved.

## Evidence Digests

SHA-256 of logs in the directory above:

```text
ba584f2ea329f39cf3f561b3b31a576ae6851c558cdc44c74ef09d75dc90e603  conditional-native-anchor-peer-initial.log
7af149c8bd7354c201128aa75631523fd85f065c51b1813d874dade071dcf664  conditional-native-anchor-peer-focused.log
e4a052d8ba75f0c66c62da1df385ed46ca948e6be2893da6f55185c455b8adaf  conditional-native-anchor-peer-full.log
af43bf56d290f71e3b5dd785f44f8fa99cc19af5275460514e8c9a846f314aa3  conditional-native-anchor-peer-full-r2.log
8feeaeb52944db8b00e6be4a87c3bbfa46bcec7fc3e15ccc7c23fe3680faa752  conditional-native-anchor-peer-full-r3.log
883aa523a4f233b074ce2a4521f063b41e1e4dfc5825fd230883565d03e60439  conditional-native-anchor-peer-parallel.log
494e94bb7527cb6095a9d157be0189e79deb85ef32a7b5fdbe46ef63a0b23311  conditional-native-anchor-peer-doctests.log
c732c1bd758d26ffd9aa2dac327bf78bcde0ebbcf429fc5208861f50ad6b7f59  conditional-native-anchor-peer-integrated-check.log
dfa340ac986265a0f7b31e320c369a9f9e135880e64cd2a2a2ebe110bb904cdd  conditional-native-anchor-peer-dco.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-anchor-peer-hygiene.log
```

## Remaining Gates

1. Consume native owners in inherited daemon, helper and root coordinator startup.
   Preserve lifecycle and sealed key descriptors during descriptor cleanup; do
   not extract raw V1 signing keys. The native peer API is implemented, but actual
   successful packet transport and protected readiness/recovery remain unvalidated.
2. Execute protected-root reissue, startup/readiness/recovery and exact cleanup.
3. Complete native admission/preparation, postchecks, invocation finish,
   revalidation, V5 publication and SubjectV3 transport in the single production
   compiler pipeline on its original ledger. Backend receipt admission remains
   V1, and conditional production finalization still refuses.
4. Complete source/machine/numerical proofs and all 47 target-matched GPU runs,
   including generic non-AMD validation. Numerical differences require explicit
   compiler-proved bounds, not unchecked tolerances.

Fresh SSH probes failed DNS resolution for mi350
(`smci350-rck-g03-b19-03.rck.dcgpu`), mi350-2
(`asrock-1w300-g2-2b.mkm.dcgpu`) and mi300x (`sharkmi300x-1`). No remote job or
scratch directory was created, and no shared source/cache/report cleanup was
performed. This local checkpoint does not credit either remote main or the
tutorial site as updated. Normal pushes to both remotes remain required.
