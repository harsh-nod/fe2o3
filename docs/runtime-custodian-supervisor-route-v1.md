# Custodian Supervisor Route V1

An ordinary protected application explicitly selects retained proof custody with:

```text
cargo-fe2o3 authority release run --application-proof-custodian \
  --manifest-path <app/Cargo.toml> --target-dir <owned-target> \
  --offline --frozen --bin <app>
```

The existing pinned compiler, production configuration and deployment requirements
still apply. This option does not install a manager, grant proof authority or make
an unverified application executable. It is run-only, may appear once before the
first `--`, and is removed before forwarding arguments to Cargo. Arguments after
`--` remain application arguments. No environment variable selects the route.

## Ownership And Protocol

Protected release authenticates the original argv before route parsing. The
generated runner uses closed context `4` for custodian applications; context `3`
retains legacy behavior. Both require the original FD195 compiler service and the
same permanent application sandbox. The selected route is stored with the original
pre-spawn handoff custody and returned by the single-use registration transfer.

Custodian transfer sends a canonical 856-byte record and the same four original
rights: compiler peer, app pidfd, proof peer, Cargo pidfd. Its 16-byte outer header
is distinct from legacy registration; the exact 840-byte inner binding is unchanged.
The supervisor consumes one packet once, checks the same descriptor/process
relationships, and retains a distinct accepted custodian owner. Malformed input
never triggers a second receiver or a legacy retry.

Preparation calls `register_custodian_application`, retains the resulting typed
broker owner, and binds the original spawned issuer pidfd through
`bind_custodian_application_issuer`. The issuer's fourteen-descriptor ABI is
unchanged. The listener uses its existing bounded worker pool.

After issuer readiness and root observation, the supervisor sends the distinct
224-byte custodian readiness record, then consumes the reverse-publication gate
and closes control. There is no liveness check after that publication commit.
Publication uses fixed-size typed records, without a new allocation after
observation. Cargo requires the matching outer record, binding, original deadline
and terminal EOF, and retains the route through canonical revalidation.

Supervisor readiness acknowledges only registration, observation and issuer
readiness. The app must independently receive authenticated `CustodianReady` from
the existing manager/controller path before ACK. The consuming host API is
`consume_inherited_worker_v3_application_custodian_handoff_v1`, followed by
`into_remote_conditional_fill`. Missing manager/deployment or legacy Ready fails
closed. Production FD195 current-record admission still precedes long proof work.

## Qualification Boundaries

CPU tests cover canonical framing, mutations, legacy/custodian cross-rejection,
original rights, absolute deadlines, EOF, route retention and publication/cleanup
ordering. Local publication fixtures do not authenticate root registration.

The composed startup fixture adds `custodian-publication`: actual cross-UID
supervisor registration and issuer binding must produce exactly one extractable
custodian owner. It deliberately drops that owner before manager activation,
contains the original app and checks failed admission plus exact publication
recovery. Its synthetic carriage and test keys do not establish a fresh compiler
receipt, production FD195 audit, successful proof or native execution.

No new formal theorem or HIP/HSA parity/performance claim follows from this route.
Full proof deployment and fresh compile-to-application qualification remain before
the ordinary two-GPU fill and bidirectional PUBLIC XGMI campaign.
