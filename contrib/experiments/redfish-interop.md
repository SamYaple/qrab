# Independent Redfish compatibility spike

This experiment exercises the actual Rust HTTP router and authentication using
unmodified third-party clients and DMTF validation logic. It is separate from
the deployed POC. The local manager fixture has no QEMU or guest: resources
requiring live queries may return 503. These outcomes must be distinguished
from malformed successful responses and from authenticated live-VM acceptance.

## Reproduce

Use an isolated virtualenv; no validator dependency enters a production image:

```sh
python3 -m venv target/redfish-interop/venv
target/redfish-interop/venv/bin/pip install -r contrib/experiments/redfish-interop-requirements.txt
git clone --filter=blob:none --no-checkout https://github.com/DMTF/Redfish-Publications.git target/redfish-interop/schemas
git -C target/redfish-interop/schemas sparse-checkout set csdl json-schema
git -C target/redfish-interop/schemas checkout fa6d23d9709aef0aba4340ef6136d332a5c9f144
```

The schema snapshot is upstream DMTF's October 2026 publication commit, with
ServiceRoot 1.22.0 definitions and all earlier versions. Pinning the revision
prevents results changing when the live schema site updates. The versioned
DSP8010 zip download at dmtf.org returned 403 in this environment; the official
Git repository supplies the same kinds of CSDL definitions without that route.

Supply a disposable account password through `REDFISH_VALIDATION_PASSWORD`,
never a command-line argument. The fixture's read-only account file must hold
an Argon2id PHC verifier, with a 0600 mode on both files. The local fixture test
uses `REDFISH_LOCAL_ACCOUNTS_FILE` and binds only 127.0.0.1:18881:

```sh
target/redfish-interop/venv/bin/python - <<'PY'
import json, os, secrets
from pathlib import Path
from argon2 import PasswordHasher
password = secrets.token_urlsafe(32)
account = {"username": "validator", "role": "admin", "password_hash":
           PasswordHasher(time_cost=2, memory_cost=19456, parallelism=1).hash(password)}
for name, contents in (("password", password + "\n"),
                       ("accounts.json", json.dumps({"accounts": [account]}) + "\n")):
    path = Path("target/redfish-interop") / name
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as output:
        output.write(contents)
PY
REDFISH_LOCAL_ACCOUNTS_FILE="$PWD/target/redfish-interop/accounts.json" \
  cargo test --locked -p qrab-redfish --test api independent_client_fixture_server -- --ignored --nocapture
```

In another terminal, load the password privately into the named environment
variable and run:

```sh
read -r REDFISH_VALIDATION_PASSWORD < target/redfish-interop/password
export REDFISH_VALIDATION_PASSWORD
target/redfish-interop/venv/bin/python contrib/experiments/redfish-interop.py \
  --base-url http://127.0.0.1:18881 --username validator \
  --csdl-directory target/redfish-interop/schemas/csdl \
  --output target/redfish-interop/results.json
unset REDFISH_VALIDATION_PASSWORD
```

HTTP is accepted only for an explicit loopback fixture. HTTPS certificate
verification is explicitly configured for every client. The script runs DMTF Python
`redfish` 3.4.2 and OpenStack Sushy 5.14.0 with both stock Basic and session
authentication. It reads inventory and cleans up the sessions it creates.
Two-second spacing between client groups respects the service's bounded
password-verification budget.

The Service Validator 3.1.8 uses its real `metadata.parse_schema_files` and
`SystemUnderTest.validate("Tree", ...)` implementation with the pinned CSDL
cache. This is schema traversal, with no mockup response substitutions and no
OEM-check bypass. It also runs a separately labeled authentication-only scope
against the session/account/role resources, independent of unavailable live
hardware queries. The tool also checks whether the endpoint actually serves
`/redfish/v1/$metadata`; supplying a local cache does not make a missing
metadata endpoint compliant. Raw resources and credential-bearing reports are
not retained: the harness selects assertion paths/messages and aggregate
counts, excluding payload values.

The upstream DMTF Python client defaults to disabled TLS certificate checking
unless its `cafile` argument is supplied. This harness explicitly supplies it.
Service Validator 3.1.8 does not expose that argument, so the harness wraps only
its client constructor to provide the CA bundle; its HTTP authentication,
responses and validation logic remain unchanged. This transport configuration
is an explicit harness adaptation, not a stock validator default.

For actual verified HTTPS, use the optional loopback TLS frontend. It creates
a throwaway CA and leaf certificate, relays to the fixed loopback Rust fixture,
and passes authentication headers/bodies unchanged. It neither authenticates
requests nor injects credentials. The CA private key stays in memory; the
temporary leaf key is 0600 under the ignored target directory.

```sh
target/redfish-interop/venv/bin/python contrib/experiments/redfish-interop-tls.py \
  --tls-directory target/redfish-interop/tls
# In another terminal, with the private password environment variable set:
target/redfish-interop/venv/bin/python contrib/experiments/redfish-interop.py \
  --base-url https://127.0.0.1:18882 --username validator \
  --ca-bundle target/redfish-interop/tls/ca.pem \
  --csdl-directory target/redfish-interop/schemas/csdl \
  --protocol-disposable --output target/redfish-interop/results-final.json
```

The frontend is only a disposable finite-response fixture, with a 1 MiB response
bound. It is not a production proxy or an SSE/download performance test.

Adding `--protocol-disposable` runs the unmodified Protocol Validator 1.3.2
suite. **It sends mutating requests** (including account/session operations),
so use it only on an explicitly disposable fixture. The harness passes the
password to the validator within Python memory, captures counts and failed
assertion/request metadata, and removes temporary raw reports. A nonzero exit
means at least one client or selected validator failed; a failed validator is
never reported as a successful conformance run.

## First actual run, 2026-11-14

All four stock client combinations passed discovery, ComputerSystem inventory,
manager inventory and session cleanup: DMTF Python Basic/session and Sushy
Basic/session. Sushy parsed the advertised power state and allowed reset enums.
No custom connector, bearer injection or client response substitution was used.

The first independent Service Validator traversal returned 540 passing,
23 warning, 19 failing and 967 skipped assertions. Twelve failures came from
the fixture's six unavailable live-query resources and repeated references to
them. Seven failures exposed actual successful-payload defects: `TaskService`
instead of the ServiceRoot's standard `Tasks` link, an invalid `Virtual`
ChassisType, `Oem` instead of the LogService enum `OEM`, and four mandatory
properties missing from the serial LogEntryCollection's link-only member.
The separate metadata request returned 404.

The first Protocol Validator run returned 166 passing, 29 failing, one warning
and 57 not-tested assertions. It found absent `/odata` and `/$metadata`, missing
HTTP discovery headers, unsupported OData versions being accepted, missing
session-collection `/Members` creation, anonymous access beyond the standard
discovery allowlist, and missing standard role resources. The Basic-over-HTTP
failure is expected from the explicit loopback-only fixture; it is not evidence
about HTTPS termination in the deployed POC. Event subscription creation was
unavailable in the fixture. These are failed conformance results, even though
the independent clients already work.

## Final verified HTTPS run

After fixing the issues discovered above, the same independent tools ran over
real TLS to the unchanged Rust router fixture. The frontend used a temporary
trusted CA; a separate negative check rejected the certificate without that CA.
The first temporary certificate exposed Python 3.14's stricter requirement for
Authority Key Identifier. Adding proper AKI/SKI/KeyUsage extensions fixed the
certificate; certificate verification was never disabled to make the run pass.

| Check | Result |
| --- | --- |
| DMTF Python, stock Basic and session | Both passed inventory and cleanup |
| OpenStack Sushy, stock Basic and session | Both passed discovery and typed inventory |
| DMTF Protocol Validator | 233 pass, zero fail, zero warn, 55 not tested |
| DMTF Service Validator, full tree | 589 pass, 24 warn, 12 fail, 977 skip |
| DMTF Service Validator, authentication scope | 77 pass, one warn, zero fail, 85 skip |
| Actual `$metadata` response | HTTP 200, XML, 68 references and 110 included namespaces resolve against pinned schemas |

All twelve remaining full-tree failures are repeated references to the six
fixture resources that require a real QEMU/guest and return 503. There are no
remaining schema failures in successful responses reached by this fixture.
The full-tree run remains a failure and the harness exits 1 accordingly; it
does not establish complete live-VM conformance.

The 24 schema warnings comprise 22 missing qrab OEM schema definitions and two
deprecated-property warnings. The authentication scope's only warning is the
missing schema for `QrabSessionService.CreateFromOIDC`. Standard session/account/
role payloads passed, but the OEM action has not been independently schema
validated. These schema definitions remain useful follow-up work.

The event failure found a real capability-advertisement defect: EventService
advertised push subscriptions while its configured push transport was disabled.
It now omits that optional link when disabled and keeps SSE advertised. The
final validator honestly leaves the disabled optional feature untested. This
is the ordinary runtime behavior, not a validator-only response substitution.

After validation, two parallel Basic requests with Argon2id m19456/t2/p1 both
returned 200. The debug Rust fixture process measured VmRSS/VmHWM 60788 kB
(about 59.4 MiB), compared with 60612 kB immediately before those two requests;
threads increased from two to four. This includes allocator retention after
earlier password checks, excludes the Python TLS frontend, and has no QEMU.
It is not a production release-image benchmark. Optional password verification
has a material memory cost; the two-check concurrency bound must be retained.

Secret-free final summaries are under ignored
`target/redfish-interop/{results-final,metadata-final,memory-final}.json`.
Raw upstream logs/reports were suppressed or deleted, and scans confirmed the
ephemeral password and PHC verifier do not appear in the summaries. The fixture
and TLS frontend were stopped after the experiment; disposable password/account
files and leaf private keys were removed.

## References

- [DMTF Python Redfish library](https://github.com/DMTF/python-redfish-library)
- [OpenStack Sushy usage](https://docs.openstack.org/sushy/latest/reference/usage.html)
- [DMTF Redfish Service Validator](https://github.com/DMTF/Redfish-Service-Validator)
- [DMTF Redfish Protocol Validator](https://github.com/DMTF/Redfish-Protocol-Validator)
- [Pinned official schemas](https://github.com/DMTF/Redfish-Publications/tree/fa6d23d9709aef0aba4340ef6136d332a5c9f144)

## Source checks and delivery scope

`contrib/check-runtime.sh` passed: 566 tests, zero failures, 13 explicitly
ignored, plus formatting, generated-file comparisons and strict Clippy. The
manual fixture is explicitly ignored in ordinary tests. Five real SPIFFE TLS
tests cover allowed identities, unlisted identities, missing certificates,
stalled handshakes and connection admission limits. API tests cover OIDC expiry,
session ownership, revocation of admitted event streams and ambiguous credentials.

At the end of the research spike, `codex/redfish-auth-validation` had not been
merged or deployed. The subsequently authorized merged release and direct native
routing are recorded in [the rollout report](../../docs/redfish-auth-rollout-2026-11-14.md). Basic accounts, public discovery and the
separate workload mTLS listener are opt-in. Enabling the workload endpoint in
Kubernetes still needs explicit operator configuration, SPIRE registration,
socket projection, a Service port and Cilium policy. Cilium mesh authentication
was disabled in the inspected cluster; existing SPIRE usage did not protect the
Redfish HTTP listener.

The local fixture/TLS helpers have been stopped. Disposable account credentials
and leaf private keys were removed. Only public schemas/certificates, dependency
caches and sanitized evidence remain in ignored `target/redfish-interop`.
