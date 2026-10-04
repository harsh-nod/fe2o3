# Native Transfer Checkpoint

Date: 2026-09-26. Continuation of the
[root launch checkpoint](conditional-native-root-launch-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
M0-M7 and 47/47 production-to-safe-GPU-launch completion remain open.

Base: `ce9a9bf882128f50918135953e1f6f15214be22c`.
Integrated code: `a65729c4d07d256de03697aae1e9334316ab8ad6`.
Subsequent documentation changes do not change executable behavior.

## Implementation

- `4d342fc51020712c48e469ff19f27ffed4de0805`: native worker's V2/V3 managed
  anchor-to-supervisor transfer, integrated from its private worktree. One closed
  implementation retains nominal family distinctions. The worker ran no Cargo,
  SSH or network commands, then reviewed the primary's service-input changes.
- `a65729c4d`: primary's native budgeted listener/root custody and tests. Both
  families use this policy-neutral owner without converting V1 authority.

`RootManagedExternalAnchorV2/V3::try_clone_for_supervisor` requires actual
same-family supervisor and policy capabilities. It revalidates the managed
occurrence, clones through retained native endpoint admission, reserves the full
pair and nominal envelope, and validates the pair before returning opaque custody.
`validate_supervisor_transfer` checks final staged Files against the retained
managed occurrence and actual contexts. No pair-only admission, identity-only
context, public provider, V1 upgrade or public testing authority is introduced.

Transfer storage is the full new charge, unlike launch growth. Consuming
`into_ordered_descriptors` checks the full transfer floor and returns ordered
endpoint/pidfd ownership. The caller retires only the envelope while the pair
remains live; on refusal both FDs close before caller retirement. Quotas include
nested continuity/admission checks and simultaneous output/frame ownership.
The local transfer charge is 8456 units, plus nested work; extraction is 2312.
These are logical resource units, not elapsed-time, instruction or RSS bounds.

`ProvisionedProtectedIssuerServiceInputsV2` freshly admits the fixed production
listener and service-owned root. Existing mechanical predicates pin descriptor,
path and parent identities, exact ownership/mode, descriptor flags, and absence
of forbidden capabilities/ACLs. Bound-to-listening continuity is monotone, while
cloning or validating a deployment transfer requires the bound state. Final
validation borrows staged Files and rejects same-shaped replacement objects.
The owner grants neither compiler authority nor provisioning provenance.

Every operation uses the caller's ledger and restores entry storage without
resetting work, peak or first-denial history. Growth is reserved before the
bounded pathname allocation; path copying is now fallible in the shared helper.
Clones reserve output storage before duplication. Consumed failures close inputs
without silently retiring caller reservations. The fixed input-operation charge
is 262152 units, covering fewer than 256 non-retrying syscalls and bounded local
work. Legacy socket predicates and clone-check ordering remain shared.

## Validation

Pinned `nightly-2026-04-03`, frozen/offline dependencies, HIP disabled, one bounded
Cargo command at a time, source frozen during builds. Final unit run used four
test threads. Logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Result |
| --- | --- |
| Initial focused build | Tuple-struct alias constructor error; corrected to an import |
| Final three-crate unit suite, R5 | 299 passed, 48 failed, 42 existing ignored; no filters; exit 101 |
| New native anchor transfer cases | 20 passed, included in R5 |
| New service-input cases | 5 passed, 4 socket-bind EPERM failures, included in R5 |
| Three-crate doctests | 17 positive and 190 compile-fail passed; exit 0 |
| Fifteen-package all-target check | Passed with existing warnings; exit 0 |

R5 consists of compiler coordinator 24/7/0, supervisor 194/40/42 and anchor
coordinator 81/1/0 (pass/fail/ignored). Nested subprocess reports are not counted
again. Cargo all-targets covers Rust build targets, not GPU architectures.

The aggregate suite is not green: failures are socket bind/domain/send/peer
credential EPERM or ACL fixture EINVAL. Four new socket tests fail during fixture
binding, before exercising the positive clone/final-validation path. They remain
enabled; no admission check or test was relaxed to hide failures.

Transfer tests cover full floors, exact/short work and scratch, arithmetic,
original denial history, drop/unwind closure and every policy/supervisor context
axis. Packaging fixtures use non-authoritative file pairs, not fabricated
managed anchors. These tests do not establish successful protected transfer.
Service-input tests cover consuming closure, short admission quotas, bounded
paths and wrong object types. Socket-dependent tests additionally specify exact
clone/final-validation quotas, sticky history, descriptor flags, substitution,
activation and retained-owner floors, but those paths still need a capable host.

The worker found two test-accounting problems, both fixed before R5: cleanup
assertions now follow unique pipe identities rather than recyclable FD numbers;
replacement root/socket files carry additional storage while the original pair
remains live. No concrete production defect was found in that source review;
it is not a proof of the complete path.

Selected SHA-256 log digests:

```text
8115a38460b2178e771677619324652d00d61f15e1a255aa46863b3055af8bd4  conditional-native-transfer-r5-20260926.log
3dbfbc2dae4829ce2138dda63806279a838ee555cbdb7003ce31f14093b15382  conditional-native-transfer-docs-20260926.log
b7d83f9b2dbde84814d9ef97f41ac2c5051cf60b4e0f5f67c925dbef08dbf8d3  conditional-native-transfer-all-targets-20260926.log
```

## Remaining Gates

1. Compose native deployment/policy/key, service inputs, lifecycle, images and
   managed anchor in the compiler coordinator. Its production preparation,
   inherited input composition and supervisor launch still use V1 owners.
2. Validate a genuine protected native startup, exact supervisor transfer and
   recovery. Add adapter-level post-clone failure/unwind tests, including child
   storage refusal and failures after endpoint receipt. Packaging tests and
   lower-level cleanup tests do not substitute for these composed cases.
3. Complete compiler admission/postchecks/finish/revalidation, V5 publication and
   SubjectV3 transport in the single production path. Native backend receipt
   admission and conditional finalization remain unfinished.
4. Validate source/MIR/KIR/machine/numerical proof, generic non-AMD behavior and
   all 47 tutorial kernels on gfx942/gfx950. No new GPU or protected-runtime run
   is credited here.

Fresh SSH attempts to mi350, mi350-2 and mi300x failed DNS in this session. No
remote process or scratch was created. Source worktrees and reports are retained.
Publication of commits/issue updates is a separate gate from local validation.
