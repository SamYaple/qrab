# Authority across resource lifetimes

A VM operation crosses several owners: human identity, declarative intent,
runtime process, kernel resource and storage export. Each crossing needs a
reference that still identifies the intended resource when the effect occurs.
This is a useful architecture-review perspective alongside the service map.

## Match the reference to the responsibility

| Entry / name | Authority used for the effect | Lifetime / recovery boundary |
| --- | --- | --- |
| Human login | Verified issuer/audience/subject plus VM action role | Token/session expiry and application authorization |
| VM name | Immutable CR UID and accepted spec | A new same-name VM gets a new identity |
| Pod name | Exact Pod UID/controller relationship and current API state | Conditional patch/delete must not affect a replacement object |
| State claim name | PVC UID, VM-bound initialized state, lease and driver attachment contract | Retain/recover the intended identity rather than silently initializing another |
| Dashboard launch label | Recorded launch/VM/claim/group provenance under protected API writers | Cleanup of created resources is separate from current access grants |
| Local socket PID | Original connector pidfd, validated cgroup/Pod and held proc directory | Dead/reused numeric processes cannot substitute another actor |
| Namespace path | Already-open namespace descriptor from the validated anchor process | Entry/FD grants use that resource through completion |
| Mounted VPN file | Read relative to the validated held process directory, with a bound | A later PID path cannot select another process's mount/root |
| Guest address | Owned pool/endpoint and controlled route/handoff state | Standby/active authority and route retirement are explicit |
| CHAP login | Exact export generation plus required hard fencing | Authentication does not establish that an old writer is gone |
| Downloaded image digest | Verified standalone source and independently sealed raw generation | Metadata cannot extend integrity to unverified backing/data inputs |
| Event sequence | Manager epoch plus sequence/gap/drop information | Bounded observations have restart/retention limits |

The references here are not all credentials. A launch ID is public provenance;
a namespace descriptor is an OS capability; a storage generation participates
in a durable backend protocol. Their enforcement owners are different.
Source: [boundary contracts](boundaries.md), [evidence](evidence.md) and
[the audited repairs](audit.md).

## Follow the local attachment case

The caller connects to a Unix socket. The kernel supplies its original peer
process handle. netd opens that process's proc directory, validates the host
unified cgroup and checks the API-owned VM relationship. It authorizes only
the VM's own NIC. The anchor's process and namespace are separately selected
and held; the helper enters that already-open namespace on a worker and grants
the resulting queues. Guest packets then use the kernel datapath independently
of this control connection.

![Held local process and resource authority](diagrams/rendered/light/30-process-resource-handles.svg)

The audit found both a classification error (a host scope's name looked like
a Pod UID) and a lifetime gap (a numeric PID/path could change between check
and use). Exact cgroup parsing addresses the former; held process/namespace
descriptors and the original peer pidfd address the latter. These are local
kernel/Pod authority checks, separate from the node-to-node SPIFFE conntrack
connection and the human's OIDC management role.

## Apply the same review to cleanup and images

State-PVC adoption and access-group deletion must follow the resources the
launch created, even when current grants change. Object UID/resourceVersion
preconditions bind a write to what was checked. Image metadata must stay
inside the integrity boundary of the verified input. Storage replacement
requires backend evidence about the old writer, not just a missing API object.
Those source repairs are recorded as A01/A02/A04; local process repairs are
A05/A06 in the audit.

## Use this perspective in the OpenStack comparison

The target moves responsibilities to Kubernetes, manager, Cilium/netd and a
scoped storage service. Those owners still have to preserve identity and
authority across retries, restarts, migrations and deletion. Removing Nova,
libvirt, Neutron or Keystone does not complete those contracts by itself.
The comparison should show both the simpler ownership map and the concrete
seams that make it work. No claim about the other cluster's internal process
handling or identity federation is inferred from this source review.
