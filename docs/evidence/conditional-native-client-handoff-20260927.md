# Native V3 Parent Handoff

Date: 2026-09-27. Continuation of the
[native installer checkpoint](conditional-native-installer-20260926.md) for
[issue #272](https://github.com/harsh-nod/fe2o3/issues/272).
**No whole M0-M7 milestone or 47/47 completion is claimed. Cargo, backend,
client-check, deployment inventory and installed entrypoints remain V1.**

Base: `f00365bbc234251dfbb1da02b681897db8f1aede`.
Implementation: `80ae471fa3a8e98f193af516452b69c44f211743`.
Conditional-issuer selector worker: `3d0e844798e5db1c1e1c5e6454fc1162503c6e65`.

## Implemented Boundary

`CompilerExecutionServiceLaunchV1::handoff_to_supervisor_v3_until` consumes the
protocol-neutral child-created service peer and pidfd. It constructs native V3
manifest/handoff records, connects to the fixed production supervisor socket,
checks its pathname, flags and credentials, sends exactly two ordered rights,
then accepts exactly one credential-bound native readiness packet and EOF.
No V1/V2 policy, manifest or readiness record is admitted or converted. The child
channel's V1 suffix describes descriptor construction, not the issuer family.

The original mutable resource-account borrow spans the entire public call. The
private pending state cannot escape between transfer and readiness. Independent
review found an address-reuse flaw in the earlier public pending design: the
work-ledger token only has meaning while the originating borrow remains live.
Combining the two phases removes that gap rather than pretending an address is a
persistent identity. The reviewer accepted the corrected ownership and accounting.

The caller prepays the launch and full authenticated profile. The outer scope
reserves intermediate growth and returns only final growth above the consumed
launch input. Entry storage is restored on success, error and unwind; work, peak
and denial history are never refunded. Input reservations remain caller-owned
on consuming failure. Published quotas bound logical work and storage, not kernel
buffers, libc internals, generated stack, process RSS or syscall latency. Nested
work is admitted at each phase; insufficient total work can consume a packet and
then refuse, but never return a successful owner.

Every transport operation is a single attempt. EINTR, AGAIN, timeout, partial
send, truncation, unexpected ancillary data and trailing packets refuse. One
absolute deadline covers connect through EOF, with the existing 120-second cap.
`SO_PASSCRED` is enabled after connection to avoid changing the required unnamed
local address through autobind. It distinguishes a zero-length packet from EOF;
the Linux Unix-socket receive path emits packet credentials for the former, but
not for an empty receive queue at shutdown. See
[Linux Unix-domain socket implementation](https://raw.githubusercontent.com/torvalds/linux/master/net/unix/af_unix.c).
The receiver also rejects and closes unsolicited rights, including at EOF.

Returned manifest/readiness records are inert observations, not protected-key,
compiler, publication or GPU authority. Child preparation/spawning and profile
authentication remain separate caller obligations. The private injected effects
in unit tests exercise ordering/accounting only, not real transport or boot.

The issuer builder now explicitly accepts `--conditional`, selecting the actual
V3 binary, a distinct target-directory variable and the matching static-image
test. Default V1 and `--native` V2 behavior is unchanged. The deployment bundle
builder has not switched families.

## Validation

Pinned `nightly-2026-04-03`, offline dependencies, HIP disabled, one bounded Cargo
command at a time with compiled inputs frozen. The native worker supplied the
selector and independently reviewed the primary's handoff. No Qwen was used.

The full GNU client run exits 101. Top-level library results are 36 passes and
12 failures; integration targets have 6 passes, 38 failures and two existing
subprocess-role ignores; the client-check binary has 6 passes. Its 5 positive
and 19 compile-fail doctests pass. Subprocess-role results are not double-counted.
The musl release library run has the same 36 passes and 12 failures, with no
ignores or filters. It also exits 101; both targets compile the new code.

Of the 13 new unit tests, six accounting/order tests pass. Four genuine readiness
tests fail fixture setup at `SO_PASSCRED` with EPERM. Three real-child transfer
tests fail existing child-channel admission with `InvalidServicePeer`, before
the new transfer executes. A direct socketpair diagnostic confirms that this
sandbox rejects `SO_TYPE`, `getsockname`, `getpeername` and `SO_PASSCRED` with EPERM;
existing child/socket suites fail at those same boundaries. No new ignore or
skip hides these failures. Those seven tests receive no runtime-validation credit.
The new child fixture kills and reaps its private child even on panic.

The selector shell test passes 13 invalid-argument cases and nine family/target
combinations using inert Cargo/image/smoke stubs. The existing native package
negative test passes. These are command-routing tests, not actual ELF or service
validation.

The actual conditional issuer builds successfully for musl release using the
builder's exact static/link/secure-entry flags. The shared readelf/nm checks and
the selected production ELF-profile test pass. An empty-environment invocation
with descriptor slots 3-11 closed exits 1 with zero output bytes. To avoid a
second build cache on the nearly full shared filesystem, these real stages ran
separately against the existing pinned cache rather than the builder's default
`profile-test` subdirectory. The mocked script test separately covers routing.
These results prove neither the image family cryptographically nor protected
boot, root provisioning, live compiler observation or GPU execution.

Binary SHA-256:
`0c05e1dfbc9bfdc2c5c536d1373d087576af238d9999cbff13a17ef3d1b20e5c`.

The final doctest rerun passes all 24 tests. All-target checking passes for the
same fifteen packages listed in the installer checkpoint, with existing warnings.
Changed Rust formatting, whitespace, hygiene delta and DCO checks pass.

Logs in `/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`:

```text
9b8c8529c6211877da11ecb7466590da7d3d139e592d00965eb50aff7904b2a1  conditional-client-handoff-full-gnu-final.log
6e1ab95d2ae26470ae6cc886a9934f3d4afbe6a5075c70b99e2c23af2daf7b8f  conditional-client-handoff-full-musl.log
966b1b234b04e467057247d468780ae3389b45315d5bf83b4b6fa087ceb74a3d  conditional-client-handoff-docs-final.log
35010099d24615266f813ec29e677b84101024cccb2f6377a173e6630be2f89c  conditional-client-handoff-all-targets.log
ea90ac32659cdc6de3e63d9f1573598bbc7ed58370d577a0eab72000be79e6e6  conditional-issuer-static-build.log
0d17e933e1ace36f9dad5cc2235871d66f28c1c83efa34a646bc6303c73d4392  conditional-issuer-static.readelf.txt
dbfad6ba913a7a0d6784f5b5ee06cb87d3e6e36193bb048dd97abb669a7cd8bc  conditional-issuer-static-profile.log
```

## Remaining Integration

Migrate native profile/policy custody, Cargo parent readiness and authority
release, backend subject preparation/publication and native terminal receipts
together. Switch the complete deployment inventory, builders, qualification and
installed entrypoints only with those consumers. Preserve existing lifecycle
lock identity, state roots, all fourteen activation roles/order and signing-key
continuity; legacy journals must not be silently reset or relabeled. The
[installer checkpoint](conditional-native-installer-20260926.md#coherent-migration-gate)
lists the remaining root startup, protected boot, publication and GPU gates.

Fresh SSH checks to mi350, mi350-2 and mi300x fail DNS, creating no remote jobs or
scratch. Fresh origin and upstream fetches fail DNS. The source commit's origin
push reaches GitHub but is rejected as non-fast-forward; its upstream push fails
DNS. Neither main publication is confirmed. Newer main commits still require
fetch and integration without force. Source worktrees, active cache and evidence
remain preserved. The run's six empty failed-listener fixture directories and
private scratch root were removed with `rmdir` after all test commands finished.
