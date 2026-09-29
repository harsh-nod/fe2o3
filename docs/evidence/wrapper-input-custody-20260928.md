# Wrapper Input Custody Checkpoint

Issue [#272](https://github.com/harsh-nod/fe2o3/issues/272) remains incomplete.
This is tested wrapper/process integration, not 47/47 protected compiler-to-GPU
qualification. No new kernel, proof-runtime or GPU execution is claimed.

Tested code: `482357cbb551aedb3f26030944d18c026e6469f8`.
The documentation-only publication commit follows it.

## Implemented

- The actual binding-wrapper entry captures stdio before opening wrapper files.
  Its final Command hook owns high-FD duplicates, preserves shared offsets/status
  flags, and closes absent or original-CLOEXEC streams. Parent invocation custody
  retains the original capture. No dormant alternative wrapper entry is required.
- Capture is safe Rust observation, not an atomic snapshot or authenticated
  invocation. External aliases remain mutable. Rust startup may already have
  sanitized inherited standard slots; pre-runtime absence is not reconstructed.
  Parent-slot checks prevent executable pins or Command's error pipe from being
  allocated into slots replaced by the final hook. Sampled flag drift refuses.
- The cwd hook owns its CLOEXEC duplicate through Command reuse/drop. Dropping
  the original pin or replacing its pathname no longer invalidates its FD lifetime.
- Service-channel transfers require kernel message credentials matching both
  the declared child PID and the socket's creation credentials. The original
  same-UID/GID handoff requirement and pidfd/endpoint checks remain unchanged.

## Verification

All four bounded Cargo checks used the same frozen 9,255-file source snapshot:
`32ca2da33cac7ae411927620d888834ca16c434afeb647395307254e97573e5f`.

| Check | Result |
| --- | --- |
| Complete cargo-fe2o3 binary unit suite | 449 passed, 5 ignored |
| Process identity, compiler client and supervisor suites | 397 passed, 46 ignored |
| Those three libraries' doctests | 168 passed |
| Cargo/codegen/coordinator all-target checks | Passed; existing warnings remain |
| Unsafe-source inventory | 5 passed; maintenance command ignored |
| Scoped formatting, whitespace, hygiene delta and 8 hygiene tests | Passed |

Coverage includes an actual wrapper query using a sealed echo fixture, the
27-case stdio state matrix, shared offsets, descriptor reuse/drop, partial
allocation failure, exec failure, cwd replacement and malformed/relayed socket
transfers. Nested expected-failing subprocesses are negative controls, not suite
failures. Two underfunded cleanup fixtures now derive their allowances from the
production accounting constants; production limits were not reduced.

On MI350-2, an isolated root diagnostic passed the expected
`ParentCredentialsMismatch` refusal after a real child UID/GID transition to
65534. This verifies a refusal, not root deployment admission. The tested binary
SHA-256 was `e1c34dc90270c69aa240613b854e4020c842f115378e0a58ecd3a429b6283782`.
Its container was read-only/networkless with only SETUID, SETGID and KILL added,
bounded to one CPU, 2 GiB and 32 PIDs. The first attempt lacked KILL for cleanup
and is excluded. Both attempts' containers and scratch directories were removed;
shared installations were untouched.

## Remaining Integration

The native root coordinator must join authenticated wrapper intake, the original
clone-owned pidfd, a child-created service socket, retained deployment/runtime
inputs and issuer readiness. Private coordinator-to-issuer composition can reuse
the existing V3 launch manifest and descriptor layout; it must not fabricate a
same-UID handoff or relax its codec. Exact compiler mappings/input resolution,
helper association and shutdown, descendant enforcement, terminal success and
installed entrypoint/provisioner integration remain required. Existing protected
runtime refusals stay in place until that complete path is established.
