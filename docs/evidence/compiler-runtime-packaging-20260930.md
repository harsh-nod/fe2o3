# Compiler Runtime Packaging Checkpoint

This is #272 M1 prerequisite work, not compiler execution or GPU qualification.
M0 is complete; M1-M7 remain incomplete; strict production-to-safe-GPU coverage
remains **0/47**. The separate source candidate is not silently merged into this
deployment change.

## Implementation

Code revision `3175b80d3` adds bundle verification and initial offline installation
to the existing `fe2o3-compiler-execution-deployment` crate. It reuses the canonical
policy V2 and runtime-manifest V1 formats, rather than creating another approval
gate. See [the deployment contract](../compiler-runtime-deployment.md).

The verifier requires independent raw record hashes, exact closure/manifest
binding, complete bounded inventories, exact modes/owners/lengths/hashes and
stable paths. It retains sealed streamed copies. Installation additionally
requires a private offline root and the actual V3 profile binding. It makes files
genuinely immutable, checks retained and named objects, and publishes directories
without replacement, runtime first and policy last. Verification never repairs
lost protection. Partial failure leaves explicit recovery diagnostics, not an
approval result or automatic rollback.

Independent reviews resulted in retained profile/file descriptors, final full-tree
checks, pre-rename staging identity checks, exact completed-directory modes,
single-attempt bounded I/O, explicit directory enumeration bounds and precise
partial-publication diagnostics. No new unsafe code was added; existing runtime
admission and `RuntimeEnforcementUnavailable` remain unchanged.

## Local Validation

All guarded runs use pinned nightly `2026-04-03`, locked/offline Cargo, one build
job, isolated per-worktree targets, disabled GPU visibility, a 12 GiB virtual
memory limit and a 1,200-second deadline. Source/tool inventories are captured
before and after each run. The guarded runner SHA-256 is
`006b2f79288c882bb436276b8d03bf6338b57a24d4d6f8512533514233286d28`.

| Run | Scope | Result |
| --- | --- | --- |
| r95 | Initial deployment library candidate | 112 passed, 1 native test ignored |
| r97 | Reviewed deployment package | 114 library, 4 command and 17 doctests passed; 1 native test ignored |
| r96 | Separate source candidate's immutable index-witness dependency | 10 focused tests passed |
| r98 | Actual-source Policy10 context/nominal matrix | All 80 sessions failed before the intended consumer, at two later boundaries |
| r100 | Final deployment library and all binary targets | 115 library and 6 command tests passed; 1 native test ignored |
| r101 | Corrected native immutable installation on MI350-2 | 1 passed; owned scratch/container cleaned and verified absent |
| r102 | Final deployment doctests | All 17 passed |

r97 source snapshot:
`81c46737d749fa94c8f07cab6d0b00bef2a5c7dda41a49a9c84ced4bb1a517e0`.
r97 log SHA-256:
`ddeb5220ceb6c409d6899bf9b7dd7525211928c5df912412134a4ba100262789`.
Both source/tool snapshots remained stable. The deployment fixtures contain
synthetic code bytes; ordinary installer tests explicitly model immutability.

The source dependency consumes only two immutable file deltas from
`a566403b7..1492da698`, not live #271 owner work. Its candidate commit is
`8b2ec0fe5`. r96 source snapshot:
`42ad211c6a19aef131d1d6d60f314d1ea0c23931789ee432b02511f474837346`.
r96 log SHA-256:
`4e7f0cfc7b79ca9448f765462bb72b0d13421c70417b41918f0de2e0e3be8ab1`.
The controls preserve exact witness argument/loan identity, reject dead/replaced
referents and retain resource boundaries. They do not establish an actual-source
continuation, context-derived memory association or proof/launch authority.

## First Native Run

r99 ran the public installer on MI350-2 against real filesystem immutable flags,
inside the existing pinned image
`sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f`.
The test artifact SHA-256 was
`dcef3a80703adfb67fb1bdc896a307203a7604a40788906149a41b2fed8dae5a`.

The run **failed** in test cleanup after installation, flag, deletion-refusal and
no-overwrite checks. `flags & !IFlags::IMMUTABLE` truncated an ext4 flag unknown to
Rustix, making the cleanup ioctl fail with EOPNOTSUPP. The test now removes only
the raw immutable bit while retaining all other bits, with a regression test.
Production installation already used OR and did not have this cleanup bug.

The outer runner still cleared immutable bits only within its fresh private
scratch, removed the data and labeled container, and verified both absent.
r99 stdout SHA-256:
`2b4826d1404136334c0274f9d82ade660bbcf5545e9a3737713c10f97a9cc8b7`.
r99 stderr SHA-256:
`060499a4c55add3a0bba75cbfec9ca3b7327b6eb1747fce1e92ea40841994953`.

The runner retains requested arguments and actual created/terminal container
inspection, exact binary/build hashes and cleanup results. Only a fresh private
scratch bind is writable; the root image and test artifact are read-only. There
is no network, host PID namespace, GPU device or privileged-container mode.
Capabilities are restricted to CHOWN, DAC_OVERRIDE, FOWNER, LINUX_IMMUTABLE and
KILL. Observation timeout never authorizes restarting or deleting a live job.
If terminal cleanup fails, the runner preserves the container and named scratch
for explicit recovery rather than claiming cleanup succeeded.

## Remaining Scope

Packaging does not generate or approve a real measured compiler release, discover
its ELF dependencies, demonstrate the five installed-runtime composition tests,
complete runtime enforcement, execute a protected proof, or grant safe launch.
The genuine context/index/view association and full finalizer/evidence chain
remain required. No tutorial website pin or milestone-completion claim advances.

## Final Reruns

r100 and r102 validated code revision `3b0123b59`, including the static-tool build
integration and CLI pin-parser tests. Source/tool inventories were stable; both
source snapshots were
`5cdaaa8e262202c11e80b722243150fcf574a71173bd6eba94b17bccceb2884f`.
r100 log SHA-256:
`4720c819d7ca7f34e02899b043eb2be3bf0a622b934ba274a68e1b82dcb989a6`.
r102 log SHA-256:
`f96d03df3bae0a00d900b680cde5578944577a9f692bffbf99d9717dc2f6d318`.
The hygiene delta and deployment-bundle shell source-contract test passed.
The full musl static build/ELF-inspection script was not executed in this checkpoint.

r101 used the same pinned native image and runner as r99, but the corrected test
artifact SHA-256 was
`7b05e1a69853d8ed4da8b69b4ad8bc2aadda9ea7bfe3377369cf404ec04dad9a`.
The public installer, real immutable flags, deletion refusal, no-overwrite check,
test cleanup and outer confined cleanup all passed. No installed shared runtime
or GPU was touched. Native stdout SHA-256:
`fc17add09bf628aacd2cd61d0835e975d87937ef4f5eb243dc32c36e57150380`.
Final native status SHA-256:
`7203e91eceae2db7d65acbeb101b34687dc4a421b04e639136e46157281a3132`.

## Actual-Source Boundary

r98 completed all 80 fresh compiler sessions on source candidate `8b2ec0fe5`.
The previous `source reference intrinsic effect is not represented` refusal no
longer occurred. There are two new observed boundaries:

- MIR optimization 0: 40 sessions refused with `execution availability differs
  from its source SSA instance`.
- MIR optimization 2: 40 sessions refused with `execution CFG transport differs
  from its captured SSA state`.

Both cover gfx942/gfx950, backend optimization 0/3, five modes and two repeats.
All failed before the intended source consumer. This is not negative-test success
or source/LLVM/artifact/proof/launch qualification. No mismatch was bypassed.
r98 source/tool inventories were stable and its source snapshot equals r96's.
Log SHA-256:
`55d4ece9b0eab43e8cc6a2a7d98449a4b90ffb010c425c227983b785e05e704e`.
The next integration work must reconcile original SSA availability and CFG
transport without weakening their exact-owner/source checks.

## Borrow Diagnostic Follow-Up

The separate source candidate now contains commit `10add0374`, which attaches
the retained function/block/statement and the failing borrow phase to otherwise
context-free availability/CFG errors. Existing precise errors and resource
failures pass through unchanged; admission, occurrence claims and accounting
are unchanged. This source candidate is not merged into the deployment branch.

r103 failed compilation on two test-only ID constructors and ran no tests.
After using the actual `from_index` constructors, r104 passed all 303 selected
`source_reference_` tests. Its source snapshot was
`bc7ccc0331673c28c8d9ef74a36e85e25e74f76794fc1f72196f3f27d950b931`;
log SHA-256 was
`af7fa31a3084bbe58110fcb0ee56697edc9c8ba6122d5cfe0106416b05eff51d`.
The final candidate additionally corrects the occurrence-claim message to cover
both duplicate and out-of-order claims, without changing that check.

r105 rebuilt the backend and completed all 80 actual-source sessions on that
final candidate. All still failed before their intended consumer:

- MIR0: 40 context-free execution-availability refusals remain. None was
  localized by the borrow-site instrumentation, so the first failing predicate
  is still unconfirmed.
- MIR2: all 40 now identify `source reference promoted borrow archive differs
  from source SSA`, with actual function/block/statement coordinates. This is
  the archive check before referent resolution or payload flattening, not
  evidence that index-witness payload conversion itself was reached.

The source/tool inventories were stable. r105 source snapshot:
`935db37683fbf18cb85ba8289b6d598d07965ddbce342d4fbc69665c9ae28b88`.
Log SHA-256:
`23d6d51c201312128e13a8f6c309cd0bae88a6516172ce800bfcd6bf1d5b653a`.
The parent terminated with status101; its private source scratch was verified
absent. These failures are not negative-boundary successes or proof/GPU credit.

M1 also still needs genuine context-derived invocation/index and global-view
association. The current SDK supplies context issuance and workgroup scope,
but not those APIs; the current fixture still uses independent thread indexing.
An API rename or an extra unused context parameter cannot close that gap.

## Measured Package Assembly

Code revision `3ef979edf` adds bounded release assembly to the same deployment
crate and command. It consumes a canonical recipe containing independently
reviewed compiler/proof pins and exact per-file roles, lengths and hashes. It
retains and measures source files, binds the actual root-owned offline V3
profile, writes a new private bundle, and checks it with the existing verifier.
It rechecks retained and named inputs and outputs before returning inert hashes.
The result is explicitly `UNAPPROVED`; it grants no compiler execution authority.

The implementation uses the caller's original resource account, rejects changed
or displaced inputs/outputs and does not overwrite or reuse partial packages.
It neither invents compiler pins nor discovers ELF dependencies. Independent
review strengthened mutation tests to require the intended checkpoint and exact
error category, including same-inode writes, record replacement and directory
displacement after verification.

| Run | Scope | Result |
| --- | --- | --- |
| r109 | Integrated assembler at `3ef979edf`, all binary targets | 130 library and 7 command tests passed; 2 native tests ignored |
| r110 | Integrated assembler doctests | 17 passed |
| r111 | Public package API on MI350-2 | 1 native test passed; owned scratch/container cleaned and verified absent |

r109 and r110 source/tool inventories were stable, with source snapshot
`f8af633a79a6b7744ebe339f07666d4cca6deb01ccc021a6060192569c115817`.
r109 log SHA-256:
`d0309a0604e2ad08813b1e87391476bcae3d7a935500db894109ba65445928e4`.
r110 log SHA-256:
`62f76fc1ab1c972097907d002d4331d3d2648eddb0c71eb8d5847a7d4a1b9720`.
The deployment-bundle shell source-contract test passed after integration.
The full musl static build/ELF inspection remains unexecuted.

r111 used the same isolated native image as r101. It exercised public assembly
and verification with a genuinely root-owned profile, then rejected a changed
profile owner without creating another package. The fixture's profile and code
contents are synthetic, not an independently approved compiler release. Binary
SHA-256:
`a74f8fcb121f4b3e73ed1867a924a76124422464469bb1e439da7679358801ba`.
The runner now selects a closed `install` or `package` mode; its SHA-256 was
`066d01f030e5f881d3e8d28ef904c858e837690f02bfa9590472f5bee6b13021`.
Native stdout SHA-256:
`7c1dce9be36e200ca34d9fdb2080f269a5994f23ff036bd66ca1068111b12252`.
Final native status SHA-256:
`129446e2c03cf9c08597596fc1cd95c97a441e4d1769a13c98d5fb40b20e361b`.
The container exited with status 0; scratch and container removal were verified.
No protected runtime installation, proof execution or GPU run was performed.

The next runtime requirements remain a real reviewed compiler/proof closure and
V3 service profile, installed-runtime composition tests, and completion of the
production execution enforcement and finalizer/launch chain. M1 is not complete.
