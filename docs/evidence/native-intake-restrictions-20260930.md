# Native Intake And Compiler Restrictions Candidate

This is a **partially validated prerequisite checkpoint**, not protected
compiler execution or a deployable default. M0 remains complete; M1-M7 remain
incomplete and the strict production-to-safe-GPU matrix remains **0/47**.

The preceding context-source checkpoint is published identically on both public
mains at `edd71e6762252d1990d10d84d10cc53b666d09b7`. This candidate is separate.

## Integrated Source

The primary integrated these reviewed private changes without activating a
successful compiler path:

| Primary commit | Boundary |
| --- | --- |
| fe2ed6c06 | Paired native V3 activation and versioned offline deployment. |
| 84b59aa36 | Dedicated root-entry unsafe inventory. |
| 7a73f076b | Explicit V4 release/broker transport of genuine V3 client profiles. |
| f78afd2f5 | Bounded original-root intake and wrapper custody. |
| c007b4874 | Compiler-only prepaid pre-exec restrictions. |
| 3a05a74bb | Exact release-family child-entry validation. |
| ea4a00544 | Private broker profile-transport extraction; no wire change. |
| 5cbe65ed5 | Original output-directory custody through V4 root intake and FD197 staging. |
| 952a1143a | Field-scoped output validation borrows. |
| 37408e57b, f4b44dff2 | Exact and one-short original-account output validation tests. |
| 97b17d5aa | Finite intake progress, failure and retained-owner tests. |
| 649fe4a39, f9008e064 | Complete retained-runtime file transfers, including shared libraries, consumed by compiler backing. |

The V4 release/broker family preserves the legacy readers and selects an exact
family before authentication. It retains the original wrapper invocation stream
and account while transferring the actual input descriptors. The root keeps its
original listener, installs incoming descriptor ownership before fallible decode,
and retains it through original-pool cleanup. Its only terminal acknowledgement
is `RuntimeEnforcementUnavailable`; no ready compiler session is produced.

The new root-intake V4 record is 240 bytes; the frozen V3 record stays 224 bytes.
V4 transfers the broker-authenticated output directory as an explicit role.
The receiver retains that original right and a separately funded typed duplicate;
staging checks the actual FD197 binding and canonical `/proc/self/fd/197` output.
No pathname or process-ID reopen replaces the retained directory. This does not
yet constrain every source, loader or output pathname used by a compiler.

Compiler backing now retains one complete approved inventory transfer instead
of four independent image transfers. Original and transferred image bytes remain
separately charged; origin, account and file checks bracket duplication. The
existing four-source Stage map and FD197 check are unchanged. Independent static
review found no blocking defect, including in the private-module extraction.
All eleven new inventory library tests passed in the isolated r71 run below;
the complete capability package and doctests subsequently passed in r73.
A genuine approved-runtime-to-backing composition run remains outstanding.
Retained library files are not
evidence that the loader resolves its paths to those files.

The compiler-only native stage installs a fixed 57-instruction syscall filter
after credentials/channel setup and before profile-ready, gate and exec. Its
7,168 units of work are prepaid before clone, with 728 bytes of conservatively
quoted scratch. The filter denies explicit writable-executable/anonymous-exec
mapping requests and explicit executable protection changes, together with a
fixed set of incompatible memory/debugging/asynchronous-I/O primitives. Ordinary
data and file-backed RX mappings remain available.

This is **not full executable-memory or runtime enforcement**. It does not
validate inherited or exec-established `READ_IMPLIES_EXEC`, immutable executable
backing, loader/proc-macro resolution, procfs writers, source/output namespaces
or all descendants. No existing runtime-admission guard is weakened.

## Primary Corrections

Actual builds caught three fixture mistakes: the pinned rustix `recv` returns
two byte counts; resource assertions must use public `failed_work()`; and the
invocation fixture must use the existing exact `/proc/./self/fd/198` backend
capability spelling. These corrections do not loosen production checks.

The wrapper tests also found a production V4 integration defect: common field
validation still required the legacy child-entry argument. Encoding and decoding
now pass their already selected family into that validator, which requires
exactly `family.child_arg()`. A new bidirectional regression rejects a legacy
entry in V4 and a V4 entry in the legacy family. Neither reader accepts both.

## Validation

All guarded runs used pinned nightly `2026-04-03`, locked/offline dependencies,
one Cargo job, serial tests, no GPU visibility, a 12 GiB process-memory ceiling
and a 1,200-second deadline. Source/tool hashes were stable throughout each run.

| Run | Observed result |
| --- | --- |
| r51-native-intake-restrictions | Compilation stopped at the new recv assertion; no tests ran. |
| r52-native-intake-restrictions | Compilation stopped at the private budget accessor; no tests ran. |
| r53-native-intake-restrictions | Coordinator: 259 passed, 3 new fixture-path failures, 6 ignored. Later packages were not executed. |
| r54-native-intake-restrictions | Coordinator: 241 passed, 21 socket-setup failures, 6 ignored. All 21 report `Operation not permitted` after the environment became restricted. |
| r55-native-protocol-deployment | Protocol: all 114 passed, including the four new intake tests. Deployment: 85 passed, 2 socket-setup refusals. The combined command failed. |
| r56-native-scalar-controls | Spawn selection: 18 passed, 3 socket-inspection refusals. All three new fixed-filter tests and the new prepayment test passed. |
| r57-native-wrapper-controls | Wrapper selection: 46 passed, 8 failed. Three expose the V4 child-entry defect; three stop at socket operations and two at premature EOF. |
| r60-release-family-controls | All 19 focused release-family and wrapper-intake resource tests passed after the strict family correction. |

These counts overlap; they are not one all-green suite. The environment denies
socket credential/type operations and a standalone local socket send also
returns `EPERM`. That is consistent with the two r57 EOF failures but does not
replace a completed permissive-environment rerun. No tests were rewritten to
treat environmental refusal as success.

### Shared-Cache Results Are Provisional

Runs r62-r67 alternated two worktrees while sharing a Cargo target directory.
In r67, the floating-point worktree exported `mixed_conditional_v26`, but Cargo
selected descriptor metadata built from the other worktree, where that module
does not exist. Its dependency file used relative paths and the cached artifact
was newer than the selected source. The run stopped with unresolved imports;
no tests ran. Before/after source hashes alone did not catch this dependency
freshness problem. All affected passing results below require isolated rebuilds.

| Run | Observation, not accepted final validation |
| --- | --- |
| r62 | Compilation found an overlapping receiver borrow; corrected in 952a1143a. |
| r63 | Coordinator: 11 passed, 7 socket-setup refusals; protocol: 9 passed. |
| r64 | 25 selected release, broker and wrapper tests passed. |
| r65 | Compilation caught two nonexistent test budget accessors; corrected in f4b44dff2. |
| r66 | Coordinator: 249 passed, 27 socket-setup refusals, 6 ignored; protocol: 119 passed. Unsafe inventory: 4 passed, 1 failed, 1 ignored. |
| r67 | Floating-point worktree failed to compile against mismatched cached dependency metadata. |

The r66 inventory failure identified seventeen stale entries, subsequently
reviewed individually; see [unsafe policy](../unsafe-code-policy.md).
Its ten intake tests all failed before reaching the receiver because socket
credential setup was denied. They have no behavioral pass credit.

The guarded runner now assigns a separate target directory to each canonical
worktree and records that directory in its report. Cargo's download cache remains
shared. Optional `FE2O3_OPT` is hashed along with the compiler tools. No native,
protected-proof or hardware result is inferred from these local checks.

### Isolated Integration Rebuild

`r71-isolated-native-inventory` rebuilt the selected packages in the dedicated
primary-worktree target at `a6e3913f2a02c867764e2d3763f886bdc61b38eb`.
The source/tool snapshots stayed unchanged. The combined command failed;
these results replace the covered provisional shared-cache observations, not
the unexecuted broker, native-exec or protected-production tests.

| Suite | Passed | Failed | Ignored |
| --- | ---: | ---: | ---: |
| Compiler closure capability | 280 | 2 | 4 |
| Compiler execution coordinator | 249 | 27 | 6 |
| Compiler execution protocol | 119 | 0 | 0 |
| Unsafe source inventory | 5 | 0 | 1 |

The capability failures occur while installing the test ACL, with `EINVAL`,
before production rejection is exercised. The coordinator failures report
`EPERM` on socket setup or inspection. Neither is counted as passing. All eleven
new complete-inventory transfer tests passed, including the added outer-scope
accounting-failure cleanup control. The inventory suite also confirms the
reviewed seventeen-entry unsafe baseline repair.

The source snapshot SHA-256 is
`1cdeafff6284319f287ef7d71ba1e1915b5b2eadb7d689c7cfafbe0ffa648e55`;
the log SHA-256 is
`8fb316645bb421f13097ca805c44333f0a163231fc2ae13ade8de1cf00c247a9`.
No proof process or GPU execution occurred. M1 and the 0/47 strict-production
matrix are unchanged.

The subsequent `r72-isolated-broker-profile-output` run passed all 25 selected
release, broker and wrapper tests at `c0ae8c7cefd4e70d30a1d02068e52efdeb21bb49`,
replacing the provisional r64 observation. Its log SHA-256 is
`a4b845235c70f581b7902f81081674db180cf5e9e2631ae6b9910f5a80666652`.

The two ACL fixtures incorrectly named `current_uid + 1` in an environment
whose user namespace maps only the current UID. Commit `2a4239723` uses that
mapped UID in a still-extended ACL, checks the exact installed ACL bytes, and
requires a successful reload after attribute removal. Production rejection
is unchanged; `EINVAL` is not skipped or treated as success.

`r73-isolated-runtime-capability-acl` at
`2a423972320534d213d8fc948f610e7a7a864342` passed the complete capability package:
282 library tests, 48 integration tests and 134 doctests, with four library
tests still ignored. Both corrected ACL rejection tests and both new inventory
compile-fail doctests passed. Source/tool snapshots stayed unchanged; the log
SHA-256 is
`44fa8a2622d8efa654a0ff1917e7dfcef942271a8b5181dab3d5185694de5e0c`.
This does not replace the 27 coordinator socket failures or unrun isolated
native-exec, protected-proof and GPU checks.

`r75-isolated-native-binaries` checked every coordinator and deployment binary
at `fb2cca4f8351877cbdff7df3cab0f5dd4276f0fb`, with locked/offline dependencies
and stable source/tool snapshots. The command passed; its log SHA-256 is
`1a5d955dc1bb5e7d6bc4135139d11e2ec693ab04fa3661c560fcec014ddc8315`.
This is compilation, not installation, root execution or deployment approval.
The nine M0 consistency tests and source-hygiene delta against `edd71e676`
also passed after the ACL correction.

### Genuine Runtime Composition Fixtures

Commit `8bb7eacbd` adds five explicit installed-runtime integration tests for
approved runtime inventory to compiler invocation backing. They call the real
fixed-origin policy/runtime admission APIs, require complete shared-library FD
retention and inherited-account charging, and check wrong-source/closure/account
rejection, one-short funding, error and unwind cleanup. The descriptor is inert
matching process data, not authenticated cargo capture. Terminal cleanup here
means owner destruction, not native process/pool retirement or safe GPU launch.

`r85-native-backing-composition-build` compiled the tests at that commit and
passed six existing backing controls. The five protected-runtime tests were
ignored, not executed or passed. Selecting them explicitly without installed
immutable inputs fails rather than silently skipping. Source/tool snapshots
were unchanged; log SHA-256:
`5cf81891c2665d6fdbdbb74551ba106d0481d23a012ab0ed00ceab4c09d02191`.

Review subsequently corrected two fixture assumptions in `71e5e4e5a`:
bind-mount aliases can share an inode despite the hardlink prohibition, so
diagnostic FD counts preserve per-entry multiplicity; and wrong-backend
rejection uses the genuinely distinct executable rustc source, not an arbitrary
shared library that could alias the backend. Two pure census controls were
added. Their rerun is separate from r85. Neither correction changes production
admission, installed input files, or `RuntimeEnforcementUnavailable`.

| Run | Log SHA-256 |
| --- | --- |
| r64 | `101bf3ca27d2092f8ee49dd626d1781e90c1355d7ce5bc89162721d0bede27b7` |
| r66 | `dd5893df7b5e971ef192e011b727bd74fa87084a721d5c5a67d841bd46ce5b69` |
| r67 | `400738fabd86f8468e869bf2451dbdfcb4023b46c30803f1ec18c1a1eb9e9816` |

Both source-contract scripts passed:

```sh
bash scripts/tests/compiler-execution-systemd.sh
bash scripts/tests/compiler-execution-deployment-bundle.sh
```

The same-source C diagnostic compiled with
`-std=c11 -O2 -Wall -Wextra -Werror -static -pthread`. Its source SHA-256 is
`3289363bad1c152741f37db47ac0dd2f8a221f82fba80701aa393de28d0d5483`;
the executable SHA-256 is
`f3c325efc396b343d7655ac31c9b85d2defc96e5d777b6190235791c56c7b1e8`.
The four new native restriction tests and two existing native compiler tests
have **not run** on this candidate. MI350 preflight found Docker and the pinned
isolated image, but SSH subsequently failed DNS resolution after the permission
transition. No remote test container or scratch directory was created.

Local reports are in the sibling
`fe2o3-issue272-production-next-evidence-20260921` directory. Key log SHA-256:

| Run | Log SHA-256 |
| --- | --- |
| r54 | `db3f7e033ab81883d3be25e0df7305f987327f8cf781b39ea709c75b9cd2b7e7` |
| r55 | `76dedab605d6ccf0920e3cfb89be3319e4b8ec83e23528ea1b3cc3c4a04511e4` |
| r56 | `38d04dc90ef4855331b1af79ec66f73a943b4d6412e686b142aad079c812b02b` |
| r57 | `15344b96a3116f75c70c0ca285229b828959739d51a63f351ffa2bb10a112282` |
| r60 | `859c89441ccc83532ed268b50a9cc922864046ef835e4f2ed8bbc3d930562578` |

## Remaining Integration

The actual output-directory role and FD197 custody are integrated, but not a
complete consuming compiler launch. Immutable runtime/source/output namespace
enforcement, protected proof RPC,
original trace-derived completion, generic finalization and safe GPU launch
remain required. Source-side context/nominal import compatibility and f32
correspondence use the existing #271 pipeline; these native changes do not
complete that compiler continuation.

Required next validation includes the full affected libraries and wrappers in
an environment allowing their actual socket operations, unsafe inventory,
all affected binary targets, and the six isolated native exec tests. Passing
them would validate these prerequisites, not complete M1 or qualify any kernel.
