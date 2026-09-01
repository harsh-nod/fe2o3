# Maintainers and Governance

fe2o3 uses a maintainer-led governance model during the developer preview.
The canonical source, issue tracker, and release repository is
<https://github.com/harsh-nod/fe2o3>. `powderluv/fe2o3` is the code mirror and
the protected parity protocol's design-bound repository. That protocol is not
deployed or qualified while the repositories are user-owned and cannot use a
GitHub merge queue.

## Roles

| Role | Maintainer | Responsibility |
| --- | --- | --- |
| Project lead and release manager | [@harsh-nod](https://github.com/harsh-nod) | Direction, releases, security coordination, final merge decisions |
| Proposed protected evidence policy owner and release reviewer | [@powderluv](https://github.com/powderluv) | Pending explicit acceptance: review of assigned trust-boundary paths, stewardship of the undeployed protected parity design, and release-environment review |

The `powderluv` duties in this table and `.github/CODEOWNERS` remain proposed;
no public acceptance has been recorded. Configuration of a reviewer or code
owner does not itself confer authority or create an accepted duty. The first
developer-preview release remains blocked until that acceptance and the
required historical licensing consent are recorded. After acceptance,
`.github/CODEOWNERS` is authoritative for path-specific review. A maintainer
may delegate review without transferring release or security authority.

## Decisions

Routine changes are decided through pull request review. Changes to public
contracts, the direct-KFD runtime boundary, evidence authority, supported
hardware, governance, licensing, or release policy should begin with an issue
and record the decision in the pull request or a design document.

Maintainers seek technical consensus. When consensus is not reached, the
project lead makes the final decision and records the rationale. Maintainers
may revert a change that violates a security, evidence, or hardware-safety
boundary even if it previously passed generic CI.

## Becoming a maintainer

Maintainers are invited based on sustained contributions, sound review across
trust boundaries, reliable handling of reports, and adherence to the code of
conduct. The project lead records role changes in this file and updates
CODEOWNERS in the same pull request.

## Project assets

The canonical source history, issue tracker, release records, and security
advisories live under `harsh-nod/fe2o3`. Tutorial content is published from
`harsh-nod/fe2o3-kernels`. The protected parity OIDC and workflow design is
bound to `powderluv/fe2o3`, but no production environment, merge queue, or
qualified issuance route is deployed there. New parity-status promotions are
unavailable until an organization-owned authority or a receipt-import route is
implemented and qualified.
