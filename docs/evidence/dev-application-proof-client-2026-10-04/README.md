# Application Proof Client And Host Join

Source: `c7e6b56fc9e5bae8dd06bdb165fa1aeecaeded80`, based on
`b7d15aca42decc5571f448fc8c65d543ef6b279c`. Both are on
`codex/r65-runtime-drain-versions`. The source commit's SSH signature was verified.

## Results

- 262 library tests passed: compiler-execution client 33, host 229.
- 82 doctests passed, including opaque custody and Send + Sync constraints.
- Four targeted compile-fail suites passed all 53 cases.
- Strict Clippy passed for both packages with all targets and warnings denied.
- Host tests compile with `hardware-test-hooks`; no GPU test was run.
- Three real-root isolated campaigns passed all 53 cases: legacy registration 24,
  custodian transport 25, exact Cargo syscall allowlist 4.

The root campaigns use the actual consuming Rust client with UID1000 application
and UID1001 synthetic controller roles. They check distinct sealed input files,
Active/Request/Proved/Retained order, original pidfds, credentials, replay,
substitution, deadlines, controller death before/after the first probe, and
continued custody after the original root sender is positively reaped.
Both legacy/custodian cross-route substitutions reject without fallback.

The allowlist campaign installs Cargo's unmodified filter after helper entry.
It found and fixed the memfd reader's `open` versus `openat` mismatch without
widening the sandbox. It is not production pre-exec or initial-exec qualification.

## Limits

The synthetic controller returns inert subject bytes. These campaigns do not
execute Verus, authenticate a deployed proof-controller image, or compose the
new host constructor with a real production FD195 audit. Host checks cover
compilation, subject matching, existing regressions and ownership constraints,
not a positive end-to-end remote host admission. No native execution, GPU,
performance, settlement, systemd deployment or HIP/HSA parity claim follows.

An initial full host test invocation passed the unit suite, then exhausted disk
in trybuild. The recorded four targeted UI suites were subsequently rerun and
passed using owned tmpfs scratch; other integration suites were not rerun.
All test namespaces and owned temporary build scratch were cleaned. The shared
MI300X host was not used; unrelated worktree files were preserved.

## Artifacts

`evidence.tar.gz` contains exact source patch/identity, test logs, command scripts,
review/cleanup notes and a SHA-256 manifest. The source patch SHA-256 is
`1775768850e2cd74c26f9730d4f6db527c66c5137842891727a58cb6e79b307a`.
The archive SHA-256 is
`b0ee240f3d608933f4df2d31e8c43a722dd648d6a29e7b015aa1635e3d14696d`.

See the [client design](../../runtime-application-proof-client-v1.md) and
[remaining multi-GPU work](../../runtime-multi-gpu-critical-path.md).
