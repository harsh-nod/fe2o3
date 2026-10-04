# Native V3 Same-Host Installer

Date: 2026-09-26. Continuation of the
[runner and record-graph checkpoint](conditional-native-runner-provisioning-20260926.md)
for [issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 completion is claimed. Installed coordinator,
provisioning commands, deployment inventory and production clients still use V1.**

Base: `a845cb0358f00634c70cbf83f3d5428f8630f3f3`.
Implementation: `a9cd5eb4b4fa79705b179cd4f426274ceef61456`.
Exclusive-lifecycle native worker: `d37cda0be14ac6ad11d908519fa115d3d2887464`.

## Implemented Boundary

The unsafe dedicated-process `run_compiler_execution_reference_provisioner_v3`
uses one original work/storage account. It checks root and single-threaded startup,
reads one bounded canonical generation argument, snapshots and clears the bounded
C environment, and resolves the two fixed service accounts with bounded NSS
buffers. Each OS operation is a single attempt; short operations, EINTR and
oversized inputs refuse rather than retry or grow a buffer.

The native exclusive lifecycle owner opens only the existing canonical production
lock. It does not construct or upgrade a legacy shared lease, expose descriptors,
or allow a public path/owner override. It pins the parent and lock identities,
checks the absolute parent without symlinks, acquires nonblocking exclusive flock,
and revalidates custody on the original account. Drop closes only, never LOCK_UN.
The shared service lifecycle pathname and inode must remain unchanged across
protocol migration so that old services still exclude a new installer.

The installer locks the configuration directory, requires the listener absent,
and measures the five fixed static images. Shared root-source metadata checks and
the existing ELF validator enforce file policy and static-image structure. Stable
double reads and SHA-256 bind the measured bytes. Source descriptors and full
logical charges remain live; revalidation checks both named and retained objects
and rereads/hashes bytes before and after record publication. Metadata equality
alone was insufficient in the same-inode mutation test.

Two seed owners retain zeroizing bytes, original descriptors and metadata through
publication. Canonical names and bytes are checked before and after publication.
The native public-record bundle constructs the five matching V3 records. Writes
use exclusively created temporary files, fixed modes, file sync, no-replace rename,
directory sync and stable readback. Existing records must match exactly. Existing
files and their directory are synced too, allowing a rerun to finish durability
after an earlier rename succeeded but its directory sync failed. All five public
records are verified again before success. Mismatches are never overwritten;
partial publication is retained, not rolled back.

Administrative writers must exclusively control these deployment paths throughout
provisioning. Advisory locks exclude cooperating installers/services, not a root
writer ignoring them. Inode checks around temporary cleanup and rename are
point-in-time observations, not atomic protection against privileged substitution.
Measurement configures trust; it is not sealed executable admission, compiler
execution, proof execution or GPU authorization. Root startup freshly admits its
own sources. Resource bounds are logical, not filesystem/NSS latency, libc-internal
allocation, generated stack, process RSS or hardware execution bounds.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time with compiled inputs frozen. A native worker implemented the
exclusive lease in its private worktree while the primary implemented filesystem
provisioning. The worker independently reviewed the installer and its fixes.
No Qwen or remote job was used.

| Check | Result |
| --- | --- |
| Full coordinator GNU unit suite | 173 passes, 7 failures, 0 ignored, 0 filtered |
| Full lifecycle GNU unit suite | 59 passes, 0 failures, 1 existing subprocess-role ignore, 0 filtered |
| Full coordinator musl release suite | 173 passes, same 7 failures, 0 ignored, 0 filtered |
| Full lifecycle musl release suite | 59 passes, 0 failures, same existing ignore, 0 filtered |
| Both crates' doctests | 12 positive and 90 compile-fail pass |
| Fifteen-package all-target check | Pass with existing warnings |
| Changed Rust formatting, whitespace, hygiene delta, DCO | Pass |

Both full unit commands use `--no-fail-fast` and exit 101 because of the seven
existing coordinator socket EPERM failures. No new ignore/filter hides failures.
All 41 new unit tests pass on both targets: 19 exclusive-lease tests, 14 filesystem
tests, 7 installer-composition tests and 1 argument-parser test. ACL fixtures now
name a mapped UID instead of `uid ^ 1`: this sandbox maps only UID 1000, and the
unmapped ACL user previously caused EINVAL. Their rejection assertions are not
weakened; unsupported/permission-limited fixture branches remain as before.

The composition tests use genuine same-owner files and injected lifecycle
observations. Separate lifecycle tests use genuine locks, including exclusion,
path substitution, exact/short quotas and unwind. Combining these results does
not claim a genuine integrated root boot. Tests cover same-inode image changes,
equal-byte seed replacement, no-replace collisions, partial-publication reruns,
post-rename sync refusal, storage retirement, denial history and ledger identity.
The sync-refusal test injects the error at the real post-rename boundary; it is
not a simulated power-loss experiment. Root-only tests that return early under
UID 1000 receive no privileged credit.

The all-target selection is coordinator, lifecycle, protocol, supervisor, issuer,
client and deployment in `fe2o3-compiler-execution-*`; coordinator, provisioner,
service and protocol in `fe2o3-external-anchor-*`; `fe2o3-runtime-protocol`,
`fe2o3-protected-service-spawn`, `fe2o3-host`, and `cargo-fe2o3`.

Logs in `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
0652656085e11b5e43e8b4c9fcd8d8e798c75d07d6a1e2dbb6cae3ce8bf3f41b  conditional-native-installer-full-gnu-final.log
b6e2d4c817db7a2610b75c3204283e4f280197f21d686bcf3469f688f0244f9d  conditional-native-installer-full-musl.log
ce33f3a80f6dfe73b1e597d88401716642def77134d8d3893cefa86f8388e46b  conditional-native-installer-docs-final.log
95f507bdb0721c7bb2df0778b69d0bfa6f44a83f93dba04c99c3c6b9e2cf49a7  conditional-native-installer-all-targets-final.log
```

## Coherent Migration Gate

1. Add bounded native supervisor handoff/readiness in the production client. Migrate
   client-check, Cargo's `authority_release.rs` and `compiler_execution_boundary.rs`,
   and backend `protected_compiler_execution.rs`/`production_pipeline.rs` together.
   The native V3 terminal exchange alone does not migrate those V1 consumers.
2. Select V3 supervisor, anchor helper and daemon in the deployment builder. Add a
   conditional issuer build selector: the existing issuer `--native` selects V2,
   not `fe2o3-compiler-execution-issuer-conditional`.
3. Change bundle destinations, deployment manifest and installed inventories,
   systemd image/record/seed paths, qualification record admission and transactions,
   smoke tests and source assertions as one coherent change. Preserve all fourteen
   activation roles/order, launcher, state roots and the existing lifecycle lock.
   Manifest schema V1 need not change solely because its file inventory changes.
4. Switch both installed coordinator/provisioner entrypoints only with those
   consumers. No runtime family fallback or service-only switch.
5. Distinguish a fresh installation from existing-state migration. Native issuer
   recovery rejects legacy journals, and persistent anchor state requires signing
   key continuity. Do not reset journals, rename versions or regenerate keys to
   manufacture compatibility.
6. Validate actual root/NSS/environment startup and complete-account exact/short
   quotas, mid-publication substitution, protected boot, readiness/EOF refusal,
   post-spawn unwind and retained cleanup. Then finish compiler/proof/publication/
   host integration and the 47-kernel GPU matrix.

Fresh SSH attempts to all three GPU aliases failed DNS, creating no remote work.
Both GitHub fetch attempts failed DNS. The origin main push again reached the
server and was rejected as non-fast-forward; the upstream main push failed DNS.
Newer remote commits still require a fresh fetch and integration without force.
Neither main publication is confirmed. The issue API was reachable and the
[earlier comment](https://github.com/harsh-nod/fe2o3/issues/272#issuecomment-5852608906)
was successfully corrected to distinguish the non-fast-forward rejection from
the upstream DNS failure. Source worktrees, active build cache and evidence are
preserved. This session's private empty scratch was removed with `rmdir`.
