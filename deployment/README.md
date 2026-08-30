# fe2o3 protected compiler-execution deployment

These files define the sole systemd deployment for the root coordinator,
protected supervisor, and local reference external anchor. They require systemd
253 or newer because the service uses ordered `OpenFile=` activation.

Install the files under their matching system directories:

- `systemd/*.service` and `systemd/*.socket` under the system unit directory;
- `sysusers.d/*.conf` under the system sysusers directory; and
- `tmpfiles.d/*.conf` under the system tmpfiles directory.

Run `systemd-sysusers` before generating the four canonical records. The
records must bind the actual `fe2o3-compiler` and `fe2o3-anchor` UID/GID values,
the installed static image measurements, both pinned verification keys, and one
another's canonical identities. Install static images as root-owned,
root-group, single-link mode `0555`; public records as mode `0444`; and each
32-byte seed as mode `0400`. None may carry a file capability or POSIX ACL.

Run `systemd-tmpfiles --create` before starting the socket. Add only authorized
Cargo users to the `fe2o3-compiler` group. Enabling
`fe2o3-compiler-execution.socket` starts the coordinator on first connection.
The coordinator independently validates the complete activation environment,
all 14 descriptors, every record relationship, both service identities, and
all filesystem policy before either child is released.

The bundled same-host external anchor remains qualification-only and carries no
production rollback authority. Production deployment still requires an
independently administered monotonic backend and separately protected key
custody.
