# Runtime Release Installation Interface

The direct KFD runtime is a Rust library linked into its consumer; there is no
standalone runtime daemon or shared HIP/HSA runtime to install. Building
fe2o3-runtime does not provision the protected compiler, proof runtime, or device
authority. gfx942:xnack- is the selected maintained native application profile;
data-only gfx950 loader support is not native execution support.

## Build And Stage The Maintained Deployment Tools

Use an exact reviewed source checkout, its pinned Rust toolchain with
x86_64-unknown-linux-musl installed, a C linker, readelf and nm. The existing
builder runs deployment crate tests, builds five static tools, checks their ELF
closure and exercises argument/refusal gates. It neither installs services nor
packages an approved compiler.

```sh
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=2
export FE2O3_STATIC_DEPLOYMENT_VERIFIER_TARGET_DIR="$RELEASE_WORK/static-tools"
bash scripts/build-static-compiler-execution-deployment-verifier.sh
```

RELEASE_WORK is an operator-owned private scratch directory, not / or /opt.
Artifacts are under static-tools/x86_64-unknown-linux-musl/release:

- fe2o3-compiler-execution-manifest
- fe2o3-compiler-execution-deployment-verify
- fe2o3-compiler-execution-deployment-install
- fe2o3-compiler-runtime-deployment
- fe2o3-compiler-execution-qualification

Distribute the exact source commit/tree, toolchain identity, tool hashes,
readelf reports, original command receipts and staged file roster to consumers.
A private stage/usr/libexec/fe2o3 tree is transport, not protected deployment.
Do not copy credentials or release key seeds to a build host.

## Compiler Owner Input And Runtime Package

The [maintained compiler-runtime contract](compiler-runtime-deployment.md)
defines the canonical recipe and offline installer. The compiler owner supplies
an explicit complete code roster and actual expected closure hashes:
Cargo, trampoline, wrapper, rustc, canonical full rustc-library-tree, backend,
and reviewed proof-runtime identity. Also required is a genuinely provisioned
V3 client profile, not synthetic unit-test bytes or a profile reconstructed from
a documentation example.

```sh
"$TOOLS/fe2o3-compiler-runtime-deployment" package \
  "$RECIPE" "$COMPILER_SOURCE_ROOT" "$PROFILE_ROOT" "$ABSENT_BUNDLE" \
  "$WORK_LIMIT" "$STORAGE_LIMIT"
```

All paths and modes must satisfy the maintained contract. PROFILE_ROOT contains
etc/fe2o3/compiler-execution/client-profile-v3 in a private root-owned offline
root. Package output explicitly says UNAPPROVED and compiler_execution_authority=false.
Its measured hashes are review inputs, not independently approved installer pins.

After independent approval of the complete records, verify with externally
distributed raw policy/manifest SHA-256 pins:

```sh
"$TOOLS/fe2o3-compiler-runtime-deployment" verify \
  "$BUNDLE" "$APPROVED_POLICY_SHA256" "$APPROVED_MANIFEST_SHA256"
```

The root operator, not a tutorial build, installs into a dedicated offline root:

```sh
"$TOOLS/fe2o3-compiler-runtime-deployment" install \
  "$BUNDLE" "$APPROVED_POLICY_SHA256" "$APPROVED_MANIFEST_SHA256" "$OFFLINE_ROOT"
```

This requires real root ownership, filesystem immutable support and
CAP_LINUX_IMMUTABLE. The offline root must already contain the genuine profile
and protected opt/fe2o3 and etc/fe2o3 parents. Both target directories must be
absent. Never substitute a live host root, overwrite an installation, clear
immutable flags, or manufacture a profile to make installation succeed.

Published runtime files are at /opt/fe2o3/compiler-runtime-v1 inside that root;
approval records are under /etc/fe2o3/build-authority. The fixed production policy
is /etc/fe2o3/build-authority/policy-v2. Runtime and approval publication are two
separate transactions; a partial publication is not success or automatic rollback.

## Service And Proof Deployment Are Separate

The [service V3 contract](compiler-execution-service-v3.md) and
[deployment bundle contract](compiler-execution-deployment-bundle-v1.md)
define paired binary/config families. A V1 install is not an implicit V3 fallback.
The maintained service builder defaults to V3; it remains an unqualified
activation prerequisite, not a deployable default switch.

The original M1 owner controls its protected runner and exact credential/config
changes. Configuration below is an inventory, not permission to start services:

- /etc/fe2o3/compiler-execution/client-profile-v3
- /etc/fe2o3/compiler-execution/supervisor-deployment-v3
- /etc/fe2o3/compiler-execution/issuer-policy-v3
- /etc/fe2o3/compiler-execution/anchor-deployment-v3
- /etc/fe2o3/compiler-execution/anchor-provisioning-v3
- /usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator
- /usr/libexec/fe2o3/fe2o3-compiler-execution-supervisor-v3
- /usr/libexec/fe2o3/fe2o3-static-preexec-launcher
- /opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5

The exact current source/artifact proof, effective service configuration,
credential delivery, independently retained lifetime custodian and terminal
retirement evidence must pass before a protected compiler/application run.
Root privileges or cooperative locks do not exclude arbitrary root/kernel writes.
A builder or successful installer does not discharge these obligations.

## Compiler And Tutorial Consumer Environment

The compiler install manifest must supply actual absolute artifact paths and
measured hashes, not values copied from another source tree. The maintained
tutorial default runner requires:

- CARGO and FE2O3_BACKEND
- FE2O3_AUTHORITY_BACKEND_SHA256_V1
- FE2O3_AUTHORITY_CARGO_SHA256_V1
- FE2O3_AUTHORITY_RUSTC_PATH_V1 and FE2O3_AUTHORITY_RUSTC_SHA256_V1
- FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1 (canonical full tree identity)
- FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1
- FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1
- Exactly one existing FE2O3_PRODUCTION_BUILD_CONFIG_V1 or V2 file.

The environment is not a substitute for the admitted production configuration,
approval profile, executed proof, publication chain or runtime enforcement.
Consumers must use cargo-fe2o3 authority release build, not a direct codegen
shortcut. The tutorial owner retains the complete advertised SIMT/tile/target
matrix; a first U32 example does not qualify all tutorials.

## Device Admission And Shutdown

Before any native attempt, reconcile original boot, PID/start/cgroup, terminal
receipts, device holds and process descendants. Establish actual current
ownership of /dev/kfd and the exact /dev/dri/renderD* node/physical GPU identity.
Device accessibility, render-group membership or an idle rocm-smi snapshot is not
protected admission. Linux KFD is required; /dev/dxg is not a replacement.

Retain the original execution owners, bounded deadline and outside whole-cgroup
cleanup duty through completion/copy/teardown. Require actual original-child
termination and cgroup/device retirement evidence. Lost SSH output is ambiguous,
not permission to replay. Do not reset GPUs, kill unowned processes or relax
supported-target/proof/custody checks to finish an installation rehearsal.

## Release Handoff

The staging receipt must name exact source, built tools, hashes, readelf reports,
configuration inventory, missing approved inputs and deployment status. Only the
release coordinator may relay reviewed objects and decide merge/deployment after
independent review. Staged tools may be reviewed now; actual compiler packaging
waits for the genuine compiler closure and provisioned profile.
