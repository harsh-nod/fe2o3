# Native Issuer Packaging and Protected Transcript

The native issuer is a separately selected, measured executable family. It is
not selected by decoding a packet, by admission failure, or by replacing the
issuer image inside a V1 deployment bundle. Existing V1 build, systemd and
installation defaults remain V1.

## Package

From a clean committed checkout with the pinned musl toolchain and dependencies,
the serialized build owner runs:

```sh
bash scripts/package-static-native-compiler-execution-issuer.sh /absolute/new/package
bash scripts/verify-native-compiler-execution-issuer-package.sh /absolute/new/package EXPECTED_IMAGE_SHA256 EXPECTED_COMMIT
```

The explicit build selector is
`bash scripts/build-static-compiler-execution-issuer.sh --native`.
`FE2O3_STATIC_NATIVE_ISSUER_TARGET_DIR` selects its build cache, independently
of the V1 cache. No-argument invocation still builds only V1. The native package
records `artifact_family=compiler-execution-issuer-native-v2`, source commit,
target and exact file hashes. Verify using a pin supplied independently of the
package, not a digest read from an untrusted package's own manifest.

Both build families share ELF64/x86-64/ET_EXEC, no-loader, non-executable-stack,
no-undefined-symbol and exact `fe2o3_secure_start_v1` entry checks. Each separately
runs the production static-image profile test and a silent missing-descriptor
refusal smoke test. These checks are not a signature over build provenance.

The package is an issuer artifact, not a complete native deployment. It must not
be passed to the V1 thirteen-file deployment installer. Native supervisor
bootstrap/provisioning still needs deliberately pinned native program/policy/key
capabilities; no systemd or live installation is changed by these scripts.

## Protected Transcript

New ignored supervisor lib tests under
`native_consuming_test_process::native_issuer::` exercise:

- `inherited_native_readiness_and_client_cancel`: real native provisioning,
  handoff, static launcher, inherited entrypoint, durable recovery, readiness
  plus EOF, public client cancellation, exact issuer exit and cleanup drain.
- `legacy_image_cannot_serve_native_launch`: a separately measured real V1
  executable receives native launch custody and must exit without readiness.
- `corrupt_native_journal_prevents_readiness`: unchanged invalid state prevents
  readiness, with no recovery rewrite.

The root coordinator must run only inside an explicitly isolated disposable
container with private /tmp, read-only source/artifact mounts, no network/GPU,
finite CPU/memory/PID limits, an init/reaper and a 180-second outer deadline per
test. Reuse the admitted test runtime's seccomp/LSM/namespace profile. Missing
profile permissions are test failures, not reasons to disable those protections.
The coordinator needs the existing KILL/SETUID/SETGID/SETPCAP fixture bootstrap
capabilities; the supervisor and issuer themselves retain no capabilities.

Supply these absolute readable image paths to the isolated test process:

```text
FE2O3_RUN_NATIVE_ISSUER_SUPERVISOR_TEST=1
FE2O3_STATIC_PREEXEC_LAUNCHER=/images/fe2o3-static-preexec-launcher
FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE=/images/fe2o3-compiler-execution-issuer-native
FE2O3_STATIC_COMPILER_EXECUTION_ISSUER=/images/fe2o3-compiler-execution-issuer
```

Images and their traversed directories must be accessible read-only to fixture
UIDs 65532/65533/65534; the images must not be owned by those UIDs. Run only the
three coordinator tests, each `--exact --ignored --nocapture --test-threads=1`.
Never invoke their private helper roles directly. Distinct-UID peer credentials,
direct client parentage, pidfds, real namespace reports and original account
lifetimes are checked by the production APIs. Fixture keys are diagnostic input,
not installed production authority.

This transcript does not issue a compiler receipt or perform an anchor exchange.
Prepare/Issue still require independent pidfd observation under the real Linux
permission rules. No ptrace, namespace, seccomp or LSM bypass is provided. A
successful test gives readiness/cancellation evidence only, not protected source
equivalence, native acquisition/currentness, GPU execution or 47-kernel credit.
