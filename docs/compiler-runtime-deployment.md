# Compiler Runtime Deployment

This initial installer fills the compiler-code packaging gap left by the V3
service provisioner. It does not enable compiler execution or complete #272 M1.
The protected runtime guard, proof chain, finalizer and safe launch remain separate
requirements. An installed inventory is not evidence that its files executed.

## Inputs

The bundle contains exactly:

```text
bundle/                              mode 0700
  policy-v2                          mode 0444
  compiler-runtime-manifest-v1        mode 0444
  runtime/                           mode 0700
    <canonical manifest paths>       exact role modes
```

All directories are 0700 and every object has the bundle root's UID/GID. No
symlinks, hardlinks, extended attributes, subordinate mounts or extra paths are
accepted. Paths, counts and lengths use the existing manifest limits: 128 code
files, 256 bytes and 16 components per relative path, 1 GiB per file and 4 GiB
aggregate code. The containing directory must remain stable during verification.

Create the records with the existing `CompilerRuntimeManifestV1` and
`CompilerApprovalPolicyV2` codecs, using an independently reviewed release's
compiler closure, code hashes, proof-runtime identity and actual V3 client-profile
identity. The policy binds the complete closure and the manifest's domain-separated
identity. The proof-helper credentials must differ from both profile services.

Distribute the raw SHA-256 hashes of both complete record files independently of
the bundle. These external pins are not the records' domain-separated identities.
Do not obtain trusted pins from an untrusted bundle or use the synthetic unit-test
files as an approved compiler release. This tool does not discover ELF dependencies
or certify that the inventory includes every executable dependency.

## Verify

```sh
cargo run --locked -p fe2o3-compiler-execution-deployment \
  --bin fe2o3-compiler-runtime-deployment -- \
  verify /absolute/path/to/bundle "$POLICY_SHA256" "$MANIFEST_SHA256"
```

Verification streams exact bytes into sealed memfds, checks the canonical records
and complete roster, and rechecks source descriptors and paths. The returned
move-only owner exposes inert policy/inventory data, not descriptors or compiler
authority. Removing or changing the bundle afterward does not change its copies.

## Install

Use a dedicated, offline root. It must be root-owned mode0700, not the running
host root, and already contain protected `opt/fe2o3`, `etc/fe2o3`, and a genuinely
provisioned `etc/fe2o3/compiler-execution/client-profile-v3`. The installer does
not manufacture a profile, start services or prove that the root is offline.
The operator must exclude services and privileged concurrent writers.

Run the built tool as root with filesystem support and `CAP_LINUX_IMMUTABLE`:

```sh
fe2o3-compiler-runtime-deployment install /absolute/path/to/bundle \
  "$POLICY_SHA256" "$MANIFEST_SHA256" /absolute/path/to/offline-root
```

Both `opt/fe2o3/compiler-runtime-v1` and `etc/fe2o3/build-authority` must be absent.
There is no upgrade, overwrite, existing-install reuse, legacy-policy fallback,
or read-only-mount substitute for `FS_IMMUTABLE`.

The installer retains the original profile and created files, verifies the profile
binding, copies sealed inputs, sets exact ownership/modes and immutable flags,
and syncs content and directories. It validates exact rosters, inodes, hashes,
metadata and immutable flags before publishing the runtime directory, before
publishing the policy/manifest directory, and before reporting success. Checking
immutability never reapplies a lost flag. Publication uses no-replace directory
renames because immutable files themselves cannot be renamed.

These are two publications, not an atomic pair. Errors after staging begins report
the stage, root device/inode, planned paths and whether each staging directory was
created. Runtime-only publication and post-publication validation/durability
failure are not success. The tool neither clears immutable flags nor rolls back
published data. Keep the root offline and recover explicitly or discard the
dedicated root. Path replacement can displace the reported names; use the retained
root identity in the diagnostic, not blind recursive cleanup of a pathname.

## Validation Boundary

The library tests use synthetic code bytes and explicitly synthetic immutability
callbacks for filesystem and interruption controls. A separately ignored native
test exercises the public installer, real immutable flags, directory publication
and no-overwrite behavior. Its runner must confine it to fresh private scratch and
perform terminal cleanup even after failure. Neither test family executes rustc,
provisions an approved release, runs Verus or launches a GPU kernel.

The existing fixed-origin production approval/runtime constructors remain the
only admission path. `RuntimeEnforcementUnavailable` is not removed by packaging.
