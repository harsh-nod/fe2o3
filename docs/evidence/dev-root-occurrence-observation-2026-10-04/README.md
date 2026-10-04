# Retained Out-of-Issuer Occurrence Observation

Parent: `7627372d2a99fe1ffd84e06d210a6606bbd62406`.
This implements the process/publication custody needed by a root-side compiler
observer. It does not yet connect that observer to the deployed signing issuer,
qualify genuine compiler receipt acquisition, or grant kernel launch authority.

## Implementation

`RetainedCompilerClientSessionV1` owns the original connected client peer and
admitted live pidfd independently of the signing service's private state root.
Its shared continuity check validates close-on-exec, socket shape, original
descriptor identities, connection credentials and pidfd target/start/liveness.
The protected service still enforces its original effective UID, private-root
metadata and distinct-client-UID rules. No service profile is weakened.

`ValidatedRemoteRustcProcessObservationV1::observe_client` consumes that client
owner and reuses the existing fixed-FD observation, bounded process-input reads,
executable/backend measurements and descriptor rechecks. The old service-based
constructor still retains its full service admission, not just client custody.
Neither constructor grants observation permissions; Linux must permit each read
and `pidfd_getfd` call.

`RetainedCompilerExecutionOccurrenceV1` joins this observation with the existing
independently derived producer, managed build attempt and exact Production-slot
publication. It owns the observation, publication lease and current token together,
without self-referential borrowing. Revalidation reconstructs the exact subject
and observation identity. Failed revalidation does not release the publication
lock. The value has no signing method or conversion from a caller subject/digest,
and no conversion to the signing issuer's private guard is introduced.

The artifact layer's new
`try_observe_compiler_module_handoff_currentness_in_slot_v3` retains one unbroken
lock through receipt reconstruction, lease admission and returned token creation.
It returns `Busy` for cooperative contention. Its observation-only policy survives
lease binding and descriptor cloning: it never creates a missing artifact lock,
does not replay an attempt-recovery registry, and does not clean or repair a slot.
Registry and lock opens reject FIFOs without blocking. Existing strict receipt,
canonical payload, physical metadata and currentness validators are reused.

The existing recovery and publication APIs retain their repair/create behavior.
The issuer's internal occurrence observation intentionally now uses the nonrepairing
path too: incomplete or contended publications reject rather than being repaired
inside acquisition. The compiler must finish its normal publication before asking
for a receipt. No wire format, signing key or issuer capability policy changes.

## Qualification

Two isolated real-root cases pass with the ordinary client running as UID/GID 1000:

- Observe its live process and mode-0700 artifact root/private publication, drop
  the separate service-admission fixture, revalidate, block a competing writer,
  and retain the lock after client exit makes revalidation fail. Only explicit
  occurrence drop releases it.
- A separate UID 1000 process acquires the real publication token. Root occurrence
  admission returns `Busy` without waiting; the holder subsequently revalidates
  its token and exits normally. This exercises kernel cross-process locks, not
  only the process-local lock registry.

Both use the normal observation/currentness code and kernel descriptors. The
client is a deliberately waiting executable with rustc-shaped arguments and a
synthetic canonical compiler handoff; this is not genuine Rust extraction,
protected compilation, issuer signing or GPU execution. Fixture setup assigns
client ownership before observation; the observer does not change file ownership
or modes. The root namespace is private, network-disabled and disposable, with
explicit ptrace, DAC and credential-management capabilities. No host-wide ptrace setting or real deployment
configuration is modified. The shared MI300X host is unused.

The tested executable hash is unchanged before and after both cases:
`b737998854aaef9404e7fd66839d5e6e35e6009bc9957e05068e7b7d3692b04d`.

| Check | Result |
| --- | --- |
| Broker library | 186 passed, 7 helper/root-fixture ignores |
| Broker ownership/API doctests | 47 passed |
| Artifact-transaction library | 213 passed |
| Artifact-transaction doctests | 11 passed |
| Isolated root/UID1000 cases | 2 passed |
| Compiler execution coordinator | 31 passed |
| Compiler execution issuer | 3 passed |
| Compiler execution supervisor | 47 passed, 2 static-launcher ignores |
| Host library | 212 passed, 3 fixture ignores |
| Broker all-target and artifact library Clippy | Passed with `-D warnings` |
| Artifact all-target Clippy | Passed with the baseline exception below |
| Targeted rustfmt and whitespace | Passed |

Artifact all-target strict Clippy found an existing eight-argument integration
helper at `tests/worker_v3_publication_intent.rs:185`. The all-target campaign
allows only `clippy::too_many_arguments`; no unrelated source or lint annotation
was changed. The first root qualification failed because the fixture's temporary
artifact root defaulted to 0755; explicitly setting the intended 0700 fixture mode
corrected it. Both diagnostics are retained. Ordinary CPU campaigns run as UID1000.

## Evidence

[`evidence.tar.gz`](evidence.tar.gz) contains the exact code patch, scripts, final
logs/statuses, executable hash pair, initial/intermediate runs and the two diagnostic
logs described above. It contains no compiled executables or signing secrets.
`final.sh` completed with status 0 after all checks. These are scoped CPU and
isolated Linux qualification results, not a full workspace or deployed issuer run.

- Archive SHA-256: `6453a7829e9a2be8415dbde5d1906190093f6e42b747672a4bf60cefe1d5ba2d`
- Code patch SHA-256: `28829936426999fc45628c6e1642f8e3ffaf71355ab37acd020429069e07baf3`

## Trust Scope

Client-session and occurrence constructors provide local custody, not proof of a
trusted coordinator channel or deployed issuer session. Root observation requires
the appropriate ptrace and filesystem capabilities; UID 0 alone may be insufficient.
The actual path is descriptor-derived `/proc/self/fd/N`, avoiding a root-selected
per-UID path-guard domain while retaining the same artifact-lock inode and directory
lock as the client. The original observation must outlive all token rechecks.

Filesystem operations remain subject to kernel/filesystem latency. Nonblocking
cooperative locks and FIFO rejection do not impose a wall-clock I/O deadline.
No new formal theorem for process, filesystem or coordinator adapters is claimed.
The changes do not establish HIP/HSA parity or a performance improvement.

## Next Integration

Add a bounded session table to the existing root coordinator. Register only from
the supervisor's admitted handoff through a private inherited control channel;
authenticate per-message kernel credentials against the retained supervisor PID,
not the socketpair creator's `SO_PEERCRED` alone. Bind the exact launched issuer
pidfd/start/profile before enabling its private endpoint.

Observe only on the issuer's acquisition request, not during launch/readiness.
Keep the retained occurrence in the session table between bounded Begin,
Revalidate and Finish steps. Do not block the monitor waiting for the issuer to
sign. The move-only remote guard must span signing and durable ledger commit.
On timeout, channel failure or shutdown, contain the exact issuer and confirm exit
before dropping its occurrence/token. Supervisor exit or delivery of a parent-death
signal alone does not establish descendant exit.

Then qualify real selected-rustc acquisition through the unchanged empty-capability
signer, complete prepared-dispatch memory/device binding, and run the admitted
two-GPU compute/XGMI/readback campaign. A3 remains open.
