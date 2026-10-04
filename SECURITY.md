# Security Policy

## Supported versions

fe2o3 is a developer preview and has not made a stable release. Security fixes
are applied to the canonical `main` branch. Older commits and preview tags are
not supported unless a release note explicitly says otherwise.

| Version | Supported |
| --- | --- |
| Current `main` | Yes |
| Developer-preview tags | No backport guarantee |
| Unreleased forks or mirrors | No |

## Reporting a vulnerability

Do not open a public issue or discussion. Use GitHub's private vulnerability
reporting form for the canonical repository:

<https://github.com/harsh-nod/fe2o3/security/advisories/new>

The private form is the project's only currently published private intake. If
it is temporarily unavailable, do not disclose report details in a public
issue, discussion, pull request, or profile message. Retry the form later; this
policy will be updated before the project designates another private channel.

Include, when available:

- affected commit or preview tag;
- threat model and security boundary;
- minimal reproducer or malformed artifact;
- impact and required privileges;
- AMD GPU, kernel, and KFD configuration when hardware is involved;
- whether the issue affects compiler authority, artifact identity, direct-KFD
  resource isolation, debugger/profiler data, or simulator correctness; and
- any proposed mitigation.

Maintainers will acknowledge reports and coordinate disclosure on a
best-effort basis. Response and remediation timelines depend on severity and
maintainer availability. Please allow time for a fix and qualification before
public disclosure.

## Scope

Examples of in-scope issues include unintended host or GPU memory access,
privilege-boundary violations, forged authority or provenance, unsafe parsing
of untrusted artifacts, command execution through developer tools, leakage of
captured kernel data, and incorrect fail-open admission.

General correctness bugs without a security boundary, unsupported hardware,
and performance differences should use the normal issue tracker.
