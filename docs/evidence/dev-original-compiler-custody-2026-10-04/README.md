# Original Compiler Custody: 2026-10-04

Base: `969005bea7db49cbbd8896221574c123ed23d134`.

The selected protected compiler wrapper now captures the original child and
parent pidfds immediately after spawn. Compiler-service transfer uses that
retained child rather than reopening its numeric PID. Failed startup retains
the owner while terminating/reaping the child. Successful startup moves it into
parent readiness custody, where historical validation remains valid after reaping.
Compiler completion rejects missing custody, wrong children and changed descriptor
flags. Application readiness keeps its existing outer-owned child lifecycle.

Managed completion now borrows both original custody owners instead of consuming
them through nested callbacks. Their lifetime includes commit, explicit failure
revocation and revocation-guard unwinding. No timeout, capability, seccomp policy,
compiler authority or proof-admission requirement is weakened.

## Verification

- 10 compiler-boundary tests pass, including four new original-child regressions
  and the tightened missing-custody negative.
- 6 wrapper lifecycle tests and 7 retained-child client tests pass.
- Scoped strict Clippy, package formatting and `git diff --check` pass.
- The production CLI rebuilds with the engineering opt-level-1 profile.
- Its genuine private-layout application campaign reaches fresh-key provisioning,
  authenticated coordinator readiness and the actual selected kernel backend.
  It still fails at local generated-proof execution (`Runtime: ... Process`),
  followed by supervisor readiness EOF, exactly as the previous clean campaign.
  It exits 101 after 96.19 seconds and removes its enclosing test cgroup.
- CLI/backend image hashes are unchanged across that campaign.

This is a tested original-process custody prerequisite, not a new formal proof,
successful compiler-proof execution, application admission or multi-GPU result.
The next implementation is authenticated compiler proof execution through the
unfiltered protected Cargo broker, followed by retained device-to-host binding
and the two-GPU fill/native-XGMI qualification. The prior genuine campaign's
private-layout and administrative-approval limitations still apply.

MI300X SSH timed out before remote execution; no remote files or jobs were created.
The archive contains source patch, commands, logs and image measurements, not
executables, signing material or caches. The temporary trampoline/log directory
is removed after archiving.

Source patch SHA-256:
`f9eb9f761f744c18e50ef471d17c46f3d254e11b18c7418d4b451aef96468a94`.

Evidence archive SHA-256:
`d6ff4b120d852783d7bb88cf68984addc0ae33f1891ffc1ba2b8dc4d431454ec`.
