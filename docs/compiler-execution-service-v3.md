# Native V3 Activation Prerequisite

Status: **private, unqualified prerequisite, not a deployable default switch**.
The paired executable, unit and offline installer changes must remain private
until the intended original-root direct startup route is composed and tested.
The current V3 `Deployment::launch` starts the independently retained anchor,
then `PreparedCompilerExecutionSupervisorV3::launch` refuses with
`native V3 indirect launch requires the original-root FD12 route`.
Here FD12 means the issuer's original-root control socket, not the unit's
policy-input slot in the descriptor table below.
That guard is unchanged. The original cleanup pool must cancel and drain the
anchor; no `READY=1`, compiler intake, protected proof or GPU success is claimed.
M0 alone is complete; M1-M7 and the strict production matrix remain incomplete.

## Paired Contract

The dedicated coordinator and provisioner mains select their existing unsafe
V3 entrypoints exactly once and terminate on every return or unwind. There is
no argument-selected family, old-record upgrade, retry or V1/V2 fallback.
The coordinator retains the original activation, signal owner, request account
and independent cleanup account. `KillMode=mixed` and the external manager's
whole-cgroup termination obligation are unchanged; the library does not admit
the effective unit configuration merely from the checked-in file.

The unit opens exactly fourteen descriptors, in this order:

| FD | Role | Fixed input |
| --- | --- | --- |
| 3 | runtime-root | `/run/fe2o3` |
| 4 | supervisor-root | `/var/lib/fe2o3/compiler-execution` |
| 5 | anchor-root | `/var/lib/fe2o3/external-anchor` |
| 6 | supervisor | `fe2o3-compiler-execution-supervisor-v3` |
| 7 | launcher | `fe2o3-static-preexec-launcher` |
| 8 | issuer | `fe2o3-compiler-execution-issuer-conditional` |
| 9 | anchor-helper | `fe2o3-external-anchor-provisioning-helper-v3` |
| 10 | anchor-daemon | `fe2o3-external-anchor-service-v3` |
| 11 | supervisor-deployment | `supervisor-deployment-v3` |
| 12 | issuer-policy | `issuer-policy-v3` |
| 13 | anchor-deployment | `anchor-deployment-v3` |
| 14 | anchor-provisioning | `anchor-provisioning-v3` |
| 15 | issuer-key-seed | `issuer-signing-key-seed-v3` |
| 16 | anchor-key-seed | `anchor-signing-key-seed-v3` |

Images are beneath `/usr/libexec/fe2o3`, records/seeds beneath
`/etc/fe2o3/compiler-execution`. All OpenFile entries are read-only.
The native provisioner measures the same five images and publishes the same
records plus public `client-profile-v3`. The profile is not an activation FD.
The lifecycle lock remains `compiler-execution-lifecycle-v1`, derived from the
retained state roots; changing record families does not create a second lock.
Existing V1 configuration is not overwritten, renamed or inferred as V3.
Missing native files or V1 bytes in a native record slot refuse.

The bounded capability set adds only `CAP_DAC_READ_SEARCH` (lifecycle `..`
traversal through service-owned mode-0700 roots) and `CAP_SYS_PTRACE` (actual
namespace observation of the distinct-UID, nondumpable anchor after profile
readiness). These do not replace any process/namespace observation. The native
process fixtures do not qualify this exact systemd unit or its host LSM policy.

## Versioned Offline Installation

Schema ownership stays in `fe2o3-compiler-execution-deployment`, coordinated
under #272. This change allocates no KIR, proof, capability, RPC or compiler
opcode. The frozen V1 header, roster, BUILD-INFO and public verification and
qualification APIs retain their meaning. V3 has a separate header
`fe2o3-compiler-execution-install-manifest-v3`, file `INSTALL-MANIFEST-V3`,
`schema_version=3` BUILD-INFO and nominal verified/installed owners.
Neither nominal owner converts to its V1 counterpart.

V3 reuses the V1 canonical grammar, out-of-band manifest SHA-256 and Git commit,
static target, modes and byte limits. It binds twelve content files: BUILD-INFO,
SHA256SUMS, unit, sysusers, tmpfiles, coordinator, provisioner and the five images
listed above. The V1-only client checker is deliberately absent, not presented
as a native client. The manifest itself is the thirteenth installed file.
The exact independent grammar/roster goldens are test-only synthetic records,
not approved executables or installable bundles.

Both families share retained-directory inventory checks, bounded stable reads,
sealed source copies, create-new copying, no-replace publication, fsync,
reacquisition and interrupted-staging cleanup. The caller selects a closed
versioned API before opening inputs; bundle bytes never select the profile.
V3 roots use `compiler-execution-v3-<manifest_sha256>` and staging uses
`.compiler-execution-v3-staging-<random>`. Recovery refuses mixed-family roots
or staging before deleting anything. Recovery preserves a named published root
but does not verify its contents; installation/reacquisition performs that check.

The paired default builder uses the existing V3/conditional child build options
and the explicit V3 manifest/verifier mode. Protected child image builders retain
their secure pre-runtime entry checks. Coordinator/provisioner/tools retain
ordinary Rust entry with static/no-dynamic-loader checks; they are not claimed
to use the protected children's secure shim. No image is relabelled as V1.

The private candidate route, for later primary validation, is:

```console
scripts/build-static-compiler-execution-deployment.sh BUNDLE_ROOT
fe2o3-compiler-execution-deployment-verify --v3 BUNDLE_ROOT MANIFEST_SHA256 GIT_COMMIT
fe2o3-compiler-execution-deployment-install --v3 BUNDLE_ROOT MANIFEST_SHA256 GIT_COMMIT INSTALL_PARENT
fe2o3-compiler-execution-deployment-install --v3 recover MANIFEST_SHA256 INSTALL_PARENT
```

Installation requires the existing root-owned mode-0700 offline parent and
publishes an offline root, not an atomic update to a running host's `/usr`.
Within the prepared deployment, the dedicated native provision command accepts
one canonical nonzero generation and uses fixed root-controlled paths and NSS
accounts. It keeps exclusive lifecycle custody, no-replace record publication
and zeroizing seeds. No installer option supplies authority from arbitrary
files or turns install success into service readiness.

Tool invocations without `--v3` retain V1 behavior for old bundles. The V1
qualification executable still admits V1 only and cannot qualify this V3 root.
Do not pass a V3 digest to it or use its historical result as V3 approval.

## Validation And Remaining Work

Authored coverage includes exact descriptor/image/record roles, genuine V1
record rejection with no resource denial, V1/V3 manifest goldens, cross-family
and mixed inventory refusal, immutable source custody, install/reacquire/tamper,
all shared publication fault points, unwind, parent replacement and isolated
crash recovery. No builds, tests or privileged startup were run by this author.

Before landing default switches: compose and test the sole original-root direct
startup/listener route with complete cleanup and exact unit confinement. Then
wire authenticated per-compilation intake through the existing invocation,
Stage/Trace and root session. FD195 invocation authority must survive until the
exact root acknowledgment before using the child-created FD195 channel.
Genuine cwd/stdio/source/runtime/proof custody, original-account funding and
trace-derived completion remain required. `RuntimeEnforcementUnavailable` and
the indirect-launch guard must not be removed to make activation appear ready.
