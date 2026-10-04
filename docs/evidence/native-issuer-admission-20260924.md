# Native Issuer Admission Checkpoint

This is custody admission for #272, not activation of the production issuer,
protected semantic proof, GPU execution, milestone completion, or 47/47
qualification. The shipping V1 issuer entrypoint is unchanged.

## Boundary

`ProtectedCompilerExecutionIssuerAdmissionV2` consumes the hardened process,
native service admission, caller-pinned policy, native signing-key capability
and external-anchor transport. It independently opens and measures the running
static executable, checks both policy measurements, and retains these owners
for continuity validation. Native admission does not read or duplicate the
key seed, convert through V1, sign a receipt, publish readiness or write a
durable journal. Anchor transport custody is not an authenticated anchor reply.

Process, executable-shape and descriptor predicates are shared with V1 through
an allocation-free error representation. Image reads are bounded positional
reads with an EOF probe and metadata checks. Short reads and interruptions
refuse rather than retry. Every image pass prepays its image/parser storage
and conservative work bound on the caller's cumulative budget.

The admission owner borrows the original work meter's lifetime and checks its
identity before continuity I/O. A different live meter cannot restart the work
budget, even with identical limits and storage reservations. Moving the budget
view is allowed. Input reservations remain the caller's responsibility; scopes
restore entry storage without refunding work or denial history. These are
logical quotas, not allocator, whole-process RSS or latency bounds.

## Validation

The following results used the combined tree based on `34a2db6c8` plus this
checkpoint's code and manifest changes. The later concurrent compiler commit
`a2d4fea57` was subsequently fast-forwarded without altering these changes.

| Check | Result |
| --- | --- |
| Broker library | 263 passed; 6 explicitly gated tests ignored |
| Issuer library | 8 passed; 1 explicitly gated test ignored |
| Broker doctests | 64 passed, including 6 new compile-fail cases |
| Full manifest regression suite | 79 passed |
| Static public-admission matrix on MI350 | 17 cases passed |
| Dependency policy | 140 members, 8 layers, 504 internal edges |

After integrating `a2d4fea57`, `cargo check --all-targets` passed for the broker,
issuer, client, deployment crate, compiler backend and Cargo CLI. The backend's
existing unused-code/import warnings remain; this is not a warnings-free
workspace claim. Both affected libraries passed
`clippy --lib --no-deps -- -D warnings`, without new allowances. The manifest
validator also passed again, still reporting pending qualification and the
existing `gemm-proof-plan` source-binding gap. The check/lint source snapshot
was `2158480e92573490626141205a05a405461682a9f93a5167c9b35e59d10da2e4`;
final documentation edits followed those measured runs. This is scoped
validation, not a full workspace or protected-runtime test run.

The 12 native unit tests include exact and one-short image quotas, prior work,
malformed/changed/short images, policy/key substitutions, storage restoration
on refusal and unwind, resource-error preservation and foreign-meter refusal.
Compile-fail tests reject cloning, descriptor extraction, legacy conversion,
V1 activation, key access and replacement of the still-borrowed work meter.

The first manifest run found two snapshot-pin mismatches. One followed the
workspace lockfile dependency edge; the inventory pin was already stale after
the earlier fill change. Only the independently derived pins were refreshed:
all original-47, role, count, source-binding and pending-obligation assertions
remain. The complete 79-test rerun passed. An initial optimized musl build was
cancelled; the successful static fixture used the non-optimized profile.

Cargo runs used nightly `2026-04-03`, locked offline dependencies, one job,
incremental disabled, no GPU devices, nice 10, a 12 GiB virtual-memory limit
and a 1200-second outer timeout. Source and tool hashes stayed unchanged
within each completed guarded run. The static build, doctests and library
rerun shared the measured source snapshot
`bd1c1468877fb6bfaab5ec29a25f77deec478f7a8e2de4bbec2ce33cff395028`.
This hashes sorted source paths and content digests, not a Git tree.

## Public Admission Run

The fixture used its real sealed-static executable and two live peer processes
with distinct UIDs, not a synthetic ELF image or supplied executable digest.
The 17 fresh-process cases cover success, moved-budget continuity, a foreign
ledger, wrong executable/runtime measurements, five key/policy substitutions,
and root/socket/client/anchor invalidation before or after admission. Retained
descriptors are dropped, storage is retired, and the private root stays empty.

The tested executable SHA-256 was
`498d2becc2f7b4ded96d38b5df785ec21f6f8453df35bff7c970a3ae1bbbc30a`.
It was selected from Cargo's fresh artifact record and matched after transfer
and execution. ELF inspection found ET_EXEC and no interpreter or dynamic
section. Build it with:

```sh
cargo +nightly-2026-04-03 rustc --locked --offline \
  --target x86_64-unknown-linux-musl \
  -p fe2o3-compiler-execution-issuer --test native_admission_v2 \
  --message-format=json -- \
  -C target-feature=+crt-static -C relocation-model=static \
  -C link-arg=-static -C link-arg=-no-pie
```

Run only `isolated_static_public_admission_matrix --exact --ignored --nocapture`
with `FE2O3_RUN_NATIVE_ISSUER_ADMISSION=1` in a disposable root container.
The MI350 run used Ubuntu 24.04 image
`sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f`,
read-only root and executable, private namespaces, no network or GPU, default
seccomp, no-new-privileges, dropped capabilities except CHOWN/KILL/SETUID/SETGID,
2 CPUs, 512 MiB memory without additional swap, 128 PIDs, and a private 64 MiB
noexec/nosuid/nodev `/tmp`. The in-container deadline was 580 seconds with
10 seconds kill grace; the external attach deadline was 620 seconds.

The container exited 0 without OOM after all 17 markers. Its initial cleanup
checker rejected Docker's lowercase absence diagnostic after removal. A
separate successful check confirmed container absence, rechecked the exact
scratch binary hash, removed only that binary and its empty private directory,
and verified absence at `2026-09-24T08:12:07Z`. No shared runtime volume or other
user's files were changed. This is CPU/OS integration on MI350, not a GPU test.

## Remaining Work

The next native transition must authenticate the exact compiler occurrence and
currentness, verify the independently pinned anchor response, and durably
commit before signing or readiness publication. Native Cargo/client, finalizer
and runtime consumers must join that authority through the single production
pipeline. Source-bound conditional ownership continuation, protected semantic
proof, machine refinement and safe launch remain required. Neither synthetic
keys in this fixture nor custody admission establishes those claims. The
actual-source fill proof and the full 47-kernel production-to-safe-launch
matrix remain separate, unfinished obligations.
