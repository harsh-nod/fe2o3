# MI350 Protected Runtime API Evidence

Recorded 2026-09-17. This is a two-test protected-runtime checkpoint, not a
production compiler, service deployment, or GPU qualification report.

## Identities

| Input | Identity |
| --- | --- |
| Compiler source | `c18f7e5c1ea66321eb7dd4604a4a49e20b1511c6` |
| Exact source roster | 5,202 files; SHA-256 `0561f066cc26b6974903ebbe1a9611143606299dd0801a0445d4eb7f8249ce9c` |
| Harness build | `nightly-2026-04-03`, debug assertions, debug info disabled, two Cargo jobs |
| Harness SHA-256 | `ba365f63c1dc74fa395191ce78d51e7e32e8ea8a3da85e7ad225198b2876f8b8` |
| Runtime manifest SHA-256 | `ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c` |
| Ubuntu 24.04 base digest | `sha256:69cecf4bbf72d2d44a9eef1b71fb98c7fb973d78af11399deccef19beb008ad9` |
| Retained MI350 image | `fe2o3-proof-runtime:20260917-c18f7e5` |
| Image ID | `sha256:a2c76f4d0c6781a44d42479162479c48f6cfc44f0e00a21ada814b3b2ca6847d` |

The source roster was checked byte-for-byte after staging and after execution.
The image is local to MI350, not a published registry artifact. Its protected
root is `/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5`
inside the image. The host root at that path remains unprovisioned.

## Provisioning

The unchanged `scripts/functional-refinement-verus-runtime-v1.sh provision`
command ran as root during the isolated image build. It checked the complete
pinned closure, excluded launcher/Rustup provenance, and all 62 Rust target
files. The installed-file audit passed during provisioning and again as the
unprivileged user before each test.

The pinned libc6 `2.39-0ubuntu8.8` package came from the
[official Ubuntu snapshot](https://snapshot.ubuntu.com/ubuntu/20260901T000000Z/pool/main/g/glibc/libc6_2.39-0ubuntu8.8_amd64.deb),
archive SHA-256 `3b8d5391b6b484a4c81fd000b6064885ad967ec3cb966bc57603f3fb3ebf0ed5`.
Its extracted loader and libraries matched the checked-in manifest. Existing
MI350 GCC, C++, zlib, stable Rust, and Rustup inputs also matched. The pinned
Verus distribution was transferred from MI300X and independently audited.
No pin was relaxed and no host system library was replaced.

The first offline Cargo build failed because the pinned Pliron checkout was
not cached. A subsequent ordinary online `cargo test --locked -p
fe2o3-verifier --lib --no-run` build passed. Both build logs are retained;
the failed attempt is not counted as validation.

## Execution

The harness ran each complete test name separately with
`--ignored --exact --test-threads=1 --nocapture`, after asserting the listing
contained exactly that test. Neither test was skipped:

| Test | Result | Duration | Log |
| --- | --- | --- | --- |
| `functional_refinement_runtime_v1::tests::protected_public_lease_executes_real_verus` | 1 passed, 0 failed, 0 ignored | 89.88 s | [Positive proof](positive-proof.log) |
| `functional_refinement_runtime_v1::tests::protected_public_lease_rejects_false_proof` | 1 passed, 0 failed, 0 ignored | 89.46 s | [False proof](false-proof.log) |

Both tests use the public lease, canonical generated source, the retained
sealed-source execution path, a 120-second proof deadline, 64 KiB stream
limits, and before/after runtime revalidation. The positive test checks exact
Verus success output and exit 0; the negative test checks exact failure output,
exit 1, and the assertion diagnostic. A setup failure cannot pass either test.

Container controls were `--user 61073:61073 --cap-drop ALL`,
`--security-opt no-new-privileges=true --security-opt seccomp=unconfined`,
`--network none --read-only --init --pids-limit 64 --cpus 2`,
`--memory 8g --memory-swap 8g`, NPROC 4096, NOFILE 1024, CORE 0,
and a 64-MiB noexec/nosuid/nodev `/tmp`. Only the test harness directory was
bind-mounted, read-only. Each container had an outer 180-second deadline.
The supervisor requires zero inherited seccomp filters and installs its own
proof-child filter; the exception was container-local. AppArmor remained at
Docker's default. Host Yama was 1 and `vm.memfd_noexec` was 0; neither changed.

The reusable image can repeat its installed-file audit without relaxing
Docker's outer seccomp profile:

```sh
docker run --rm --pull=never --user 61073:61073 --cap-drop ALL \
  --security-opt no-new-privileges=true --network none --read-only \
  --tmpfs /tmp:rw,nosuid,nodev,noexec,size=64m,mode=1777 \
  fe2o3-proof-runtime:20260917-c18f7e5
```

That command audits files only; it does not execute a proof. The full local
evidence archive includes staging/build/run scripts, the Dockerfile, all 15
terminal log/identity files with checked SHA-256 values, and the source roster.
The published test transcripts normalize trailing whitespace; the local archive
retains the original bytes.

After archiving and verifying those files, both test containers were confirmed
absent. The dedicated builder, its cache volume, and its newly downloaded
BuildKit image were removed. The private staging/build directory was removed
(1,556,438,099 logical bytes). The reusable runtime image remains, 735,450,974
bytes; no service was left running. Host loader/libc/libm hashes were unchanged.

## Boundaries

Release-mode tests and the broader authenticated-execution reviewed-host suite
were not run. Production service deployment, protected backend integration,
source/machine refinement, ordinary safe GPU launch, and tutorial qualification
remain open. No GPU work was run and no tutorial status or website pin changed.
