# Retained Proof Controller Execution Qualification

Source: `98c56e1f0ab815ba336c2e0660652eb420344b6e`.
Parent: `d65e1f1fb1ff7430cd1fb8bfbdfe8b96566a78af`.
The fixed static proof-controller child now genuinely executes authenticated
machine analysis and protected conditional-fill Verus after secure exec. It
retains the original proof until authenticated release. This removes an execution
prerequisite for the ordinary two-GPU application; it is not deployed custodian
approval, application proof custody, native launch authority or A3 completion.

## Implementation

The production verifier's pre-exec callback used a libc `close_range` symbol that
static musl does not provide. It now calls the x86-64 Linux syscall directly, using
the existing supervisor pattern. Descriptor CLOEXEC behavior and fail-closed error
handling are unchanged; no unsupported-kernel fallback was added.

A feature-gated example and ignored root integration test exercise the existing
typed proof launcher. Root stages a sealed static executable, sealed exact envelope,
HSACO and bounded configuration, and a nonblocking UNIX SEQPACKET control endpoint.
All input cursors begin at EOF; positional reads must recover the exact bytes.
The child requires fixed argv, empty environment, UID/GID 61000, the full distinct
proof profile, original parent identity and retained SIGKILL parent-death behavior.

Worker policy is measured separately in the same pinned library/path overlay.
The child opens the analyzer against that supplied policy, never auto-approves a
candidate, and opens/revalidates the protected Verus runtime after exec. One
original boxed `RetainedWorkerV3ConditionalFillProofV1` survives reporting, artifact
transfer and Probe. Closure, subject, receipt/analysis identities and owner-buffer
addresses are rechecked before Release. Addresses are diagnostics, not authority.

The bounded fixture protocol checks exact per-message SCM_CREDENTIALS and nonce,
rejects unexpected descriptor transfer and fields, and revalidates the original
controller pidfd around nonterminal reports. Its JSON reports are inert test data,
not a production proof lease or a replacement for the retained original owner.

## Qualification

| Check | Result |
| --- | --- |
| Focused Worker V3 vertical tests | 55 passed; 8 ignored; 18 explicitly filtered |
| Verifier process-tree regression tests | 19 passed; 2 pinned-runtime tests ignored |
| Genuine native Worker input preparation | Passed |
| Final private-root controller campaign | All 5 cases passed |
| Targeted example/integration Clippy | Passed with `-D warnings`, `--no-deps` |
| Static musl ELF | Secure entry equals ELF entry; no dynamic loader or undefined symbols |
| Formatting, source/binary hashes and committed patch comparison | Passed |

The healthy root case retains strict-imported boundary-5 conditional proof evidence
and authenticated analysis while all four origin/currentness/load/launch authority
flags remain false. It transfers seven content-checked artifacts, including the
40,224-byte generated proof, then verifies unchanged custody at Probe and clean
release/reap. Wrong worker identity, wrong runtime identity and changed finalized
payload reject at their expected boundaries and exit with the exact rejection code.

Cancellation waits for an actual live Verus descendant with `--no-cheating` before
killing and reaping the original controller pidfd. Direct-child cleanup does not
claim descendant authority: the disposable PID namespace supplies outer containment.
After every final case, a root host-side task-namespace scan confirms no live task
remains in that recorded private namespace. These are Linux behavioral checks,
not a formal proof of the launcher, protocol or namespace implementation.

## Evidence

[Evidence archive](evidence.tar.gz) SHA-256:
`ca08892b498f1c94e0c57637fadfacc2af52f49d7642eb14fa70f0abb8723822`.
Qualified and committed source patch SHA-256:
`5f84b48886e5eda42ad23f2b05c28cdef21303bd77c3a2283ef3e358a5be7e76`.
The archive contains the verified signed source commit, matching patches, source
and binary hashes, exact commands/scripts, final captures, logs and a checked
manifest. Development failure logs are retained separately from final pass logs.
No host executables or private signing keys are included; the exact HSACO input
is retained as verification evidence.

All root work used disposable local namespaces and a private runtime/library
overlay. No host deployment was changed and MI300X was not used. Owned scratch is
removed after packaging; existing shared build caches are retained.

## Next Multi GPU Gate

Deploy the actual fixed keyless custodian through an independently approved
unfiltered launch boundary; the existing filtered coordinator cannot launch this
role. Implement root Ready handover of its exact proof endpoint/session and a
move-only remote owner, then consume that custody together with the existing
per-device conditional native premises. Keep one proof alive until both devices
settle. Genuine issuer/anchor receipt acquisition and FD195 auditing under the real
application filter remain required. The next hardware gate is ordinary fill on
both devices followed by actual completed output, staging, settled H2D, PUBLIC
XGMI and full guarded readback in both directions, with N=65 and G=128.
