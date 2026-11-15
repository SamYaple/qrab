# Guest networking

A VM has no network interface unless `spec.nics` names one. Each guest NIC is
backed by its own **NIC Pod**, `<vm>-nic-<name>`. The NIC Pod's IP is the
guest's address on that NIC, its Cilium identity is the NIC's identity, and its
labels carry the NIC's security groups. Guest traffic never enters the VM
Pod's network namespace.

```yaml
spec:
  nics:                       # at most 4, names unique
  - name: lan                 # DNS label, 1..15 characters
    securityGroups: [web, ssh]   # at most 8 DNS labels
```

## Why a Pod per NIC

- **The VM Pod's namespace stays private.** It holds the BMC (8080), Redfish
  (8081) and the manager's control socket. A guest on the same network
  namespace could reach those listeners, and policies selecting the VM Pod would
  have to treat guest traffic and BMC traffic as one identity.
- **Upstream Cilium gives one endpoint per Pod.** A second interface inside the
  VM Pod would share its identity and IP. A separate Pod gets its own
  endpoint, identity and IP from the node's Pod CIDR, which the cluster's BGP
  already advertises (native routing). Nothing like OVN, Multus or a second
  CNI is involved.
- **The `qrab` namespace enforces Pod Security restricted.** There is no
  hostPath, no added capability and no tun device, so the two Pods cannot share
  a Unix socket or a tap. They talk over TCP, and passt needs neither tun nor
  privileges.

## Data path

```
guest virtio-net
  → QEMU -netdev stream (TCP client, VM Pod)
  → NIC Pod :7700, qrab-manager nic (one connection at a time)
  → passt Unix socket (/run/nic/passt.sock)
  → passt: guest L2 ⇄ the NIC Pod's own TCP/UDP/ICMP sockets
  → Cilium (NIC Pod identity and policies)
  → BGP-routed Pod IP
```

QEMU's `stream` netdev and passt use the same framing (a 4-byte big-endian
length, then the Ethernet frame), so the relay copies bytes. QEMU runs with
`"reconnect-ms":1000` and the virtio-net device has no option ROM
(`romfile=`), so it never enters the boot order.

passt answers the guest's ARP, NDP and DHCP itself and hands it the NIC Pod's
own address. That DHCP serves only this guest; it is not network DHCP.

The gateway is the fixed link-local `169.254.1.1` on every node (`-g`).
passt answers ARP for it and routes through it. With a Cilium /32 Pod address,
passt's DHCP adds option 121 so the gateway is reachable on-link. DNS sent to
`169.254.1.1` is forwarded to the NIC Pod's resolver (`--dns-forward`).
`--no-map-gw` keeps the gateway address from reaching the NIC Pod's loopback,
where the relay listens. Outbound connections from the guest leave from the
NIC Pod's IP.

The VM's NoCloud seed carries the same configuration statically, so guests
that run cloud-init need no DHCP. Each NIC is matched by MAC, gets the NIC Pod
IP as a /32, a default route `on-link` via `169.254.1.1`, and nameserver
`169.254.1.1`. A VM with NICs gets a seed even without `spec.cloudInit`, and
passt's DHCP remains as a fallback. Inbound
connections to the NIC Pod IP on any TCP or UDP port, except the relay port
7700 and the kernel's ephemeral range (32768–60999), are forwarded to the
guest. This is passt's `-t ~7700 -u ~7700`; passt skips the ephemeral range
because those ports carry its own outbound connections.

The NIC Pod runs `qrab-manager nic --listen 0.0.0.0:7700 --socket
/run/nic/passt.sock`. It binds the relay listener, then starts

```
passt -f -q --no-ns-isolation --socket /run/nic/passt.sock -P /run/nic/passt.pid \
  -t ~7700 -u ~7700 -g 169.254.1.1 --no-map-gw --dns-forward 169.254.1.1
```

- **Exit behaviour.** If passt exits, the relay exits non-zero and Kubernetes
  restarts the container (the Pod keeps its IP).
- **Second connections.** A second TCP connection while one is active is
  closed at once, as passt does with a second client; QEMU retries.
- **Stale sessions.** TCP keepalive (5 s idle, 3 probes 2 s apart) frees the
  slot when a VM Pod vanishes without closing the connection.
- **Readiness.** `qrab-manager nic-ready` passes once passt's pidfile and
  socket exist and the listener is bound. It reads the `ready` record and
  `/proc`, and never connects to passt or 7700, because both serve a single
  client.

### Resource cost

passt holds one listening socket per forwarded port, about 72,000 sockets
(TCP and UDP). Their kernel memory, about 260 MiB, is charged to the NIC
container, which requests 320 MiB and is limited to 512 MiB. passt's own RSS is
about 16 MiB.

### The passt patch

The manager image builds passt from the Debian source
`0.0~git20250503.587980c-2+deb13u1`, taken from the base image's snapshot and
checked against `contrib/images/passt/source.sha256`. It adds
[`0001-no-ns-isolation.patch`](../contrib/images/passt/0001-no-ns-isolation.patch).

Upstream passt always isolates itself:

1. `unshare` into a user namespace, then into IPC, mount, UTS and PID
   namespaces;
2. `pivot_root` into an empty tmpfs.

containerd's RuntimeDefault seccomp profile refuses `unshare`, `mount`,
`umount2` and `setns` to a container without CAP_SYS_ADMIN. In a restricted
Pod, upstream passt therefore exits with "Couldn't create user namespace". The
patch adds the opt-in flag `--no-ns-isolation`, which skips only those steps.

passt still keeps the rest of its hardening:

- it switches UID and GID;
- it clamps and drops capabilities (none are present);
- it sets `no_new_privs`;
- it installs its own seccomp filter. Under the Pod's RuntimeDefault filter,
  `/proc/<pid>/status` shows `Seccomp: 2` and `Seccomp_filters: 2`.

The Pod itself supplies the remaining isolation: a separate network namespace,
read-only root, no capabilities, no service-account token and RuntimeDefault
seccomp. Without the flag, passt behaves exactly as upstream.
`images.lock.json` records the version and the patch hashes.

Two more patches matter whenever a NIC Pod is replaced while its guest runs
(always the case with [stable addresses](#stable-addresses)):

- [`0002-tcp-buf-record-owner-of-flag-frames.patch`](../contrib/images/passt/0002-tcp-buf-record-owner-of-flag-frames.patch):
  inbound TCP that reaches a new passt before QEMU reconnects made it crash
  (SIGSEGV in `tcp_revert_seq`). passt queued flag-only frames (SYN, ACK, FIN)
  without their connection, then reverted sequence numbers through a NULL or
  stale pointer when the send to the absent guest failed. Unpatched upstream
  passt crashes the same way.
- [`0003-passt-preset-guest-mac.patch`](../contrib/images/passt/0003-passt-preset-guest-mac.patch):
  a fresh passt addresses frames to broadcast until the guest sends one, and
  Linux TCP drops segments not addressed to the host's MAC. An idle guest
  therefore never answered a replaced NIC Pod. The patch lets
  `--ns-mac-addr` preset the guest MAC in passt mode; `qrab-manager nic
  --guest-mac` passes the MAC the operator assigned.

QEMU carries one patch as well,
[`0001-net-stream-reset-send-state-on-disconnect.patch`](../contrib/images/qemu-patches/0001-net-stream-reset-send-state-on-disconnect.patch),
applied by `prepare-source.py` and locked in `qemu-source.json`. When the
stream peer goes away, 11.1.3 keeps a partly written frame's offset and the
`receive_disabled` flag a blocked write set, so the next connection could
start mid-frame and guest TX could stay queued. The patch resets the offset
and releases the queue on disconnect. (This hazard follows from the code; the
guest stall seen live was the broadcast-MAC issue above.)

## Security groups

Each entry in `securityGroups` becomes the label
`sg.qrab.yapplesauce.com/<group>: ""` on the NIC Pod. Security-group rules are
ordinary CiliumNetworkPolicies selecting those labels:

```yaml
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: sg-web
  namespace: qrab
spec:
  endpointSelector:
    matchLabels:
      sg.qrab.yapplesauce.com/web: ""
  ingress:
  - fromEntities: [world, cluster]
    toPorts:
    - ports:
      - {port: '80', protocol: TCP}
      - {port: '443', protocol: TCP}
  egress:
  - toEntities: [world]
```

Changing a NIC's `securityGroups` patches the NIC Pod's labels in place. Cilium
recomputes its identity; the guest, its IP and the VM Pod are untouched.

### Default allow, and a baseline

The operator's own policy for a NIC (below) never enables default deny. **A
NIC Pod that no security-group policy selects is default-allow** in both
directions: any source can reach every forwarded guest port, and the guest
can reach anything. That is Cilium's rule for an endpoint that no enforcing
policy selects. Install a default-deny baseline for every NIC Pod and open
only what each security group needs:

```yaml
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: vm-nic-baseline
  namespace: qrab
spec:
  endpointSelector:
    matchLabels:
      app.kubernetes.io/component: vm-nic
  # Empty rules: selecting the NIC Pods turns on default deny both ways.
  ingress:
  - {}
  egress:
  - toEndpoints:
    - matchLabels:
        k8s:io.kubernetes.pod.namespace: kube-system
        k8s:k8s-app: kube-dns
    toPorts:
    - ports:
      - {port: '53', protocol: UDP}
      - {port: '53', protocol: TCP}
```

The relay rule below keeps working under such a baseline.

## The relay policy

For each NIC the operator owns a CiliumNetworkPolicy `<vm>-nic-<name>-relay`
with two rules. Both set `enableDefaultDeny: {ingress: false, egress: false}`,
so they only add rules and never switch an endpoint into default deny.

1. **Rule for the NIC Pod.**
   - It allows TCP 7700 from this VM's Pod only (`app.kubernetes.io/component:
     vm` and `qrab.yapplesauce.com/owner-uid: <uid>`).
   - Its `ingressDeny` refuses 7700 from every other source: the world, host,
     remote-node, kube-apiserver, ingress, health, init and unmanaged
     entities; Pods in other namespaces; non-VM Pods; and other VMs' Pods.
   - Deny rules take precedence over allows, so no security-group rule, however
     broad, opens the relay port.
2. **Rule for the VM Pod.** It allows egress to its own NIC Pods on TCP 7700.
   This rule matters when another policy, such as `qrab-demo`'s in
   `console.yaml`, restricts the VM Pod's egress.

NIC Pods are labelled `app.kubernetes.io/component: vm-nic` and share
`app.kubernetes.io/instance: <vm>` with their VM Pod. A Service or policy meant
for the VM Pod must therefore also select `app.kubernetes.io/component: vm`, as
`console.yaml` does, so that NIC Pods never receive BMC or Redfish traffic.

## Addresses and replacement

- **Deterministic MACs.** The guest MAC is `02:` followed by five bytes of
  SHA-256 over the VM UID and NIC name. It is unicast and locally
  administered, and only QEMU knows it; passt learns it from the guest's
  frames.
- **Ordering.** The operator creates the NIC Pods first. It reports reason
  `NicPending` until every one is Ready with an IP. Only then does it render
  the VM's manager config with `nics: [{id, mac, host: <NIC Pod IP>, port:
  7700}]` and start the VM Pod. `status.nics` lists each NIC's `name`, `mac`,
  `podName`, `ip` and `ready`.
- **Replacing a NIC Pod.** NIC Pods hold no VM state, so a missing or Failed
  NIC Pod is recreated. With [stable addresses](#stable-addresses) it comes
  back with the same IP on any node. Without them a replaced NIC Pod normally
  gets a **new IP**, which changes the VM's config and triggers the operator's
  normal controlled VM Pod replacement (TPM and firmware identity are kept);
  the guest then gets the new address from passt's DHCP. A container restart
  inside the same Pod keeps the IP; QEMU reconnects and the guest does not
  restart.
- **While the VM is off.** NIC Pods stay up when `running: false`, so a
  restart keeps the IPs.
- **Removal.** Removing a NIC deletes its Pod and policy. Deleting the VM
  garbage-collects both through owner references (with stable addresses, the
  operator first stops the NIC Pods and releases their addresses).

## Stable addresses

With `QRAB_NIC_ADDRESS_RANGE` set (live: `10.244.128.0/17`, carved out of the
Cilium default pool in kubernetes-gitops), each NIC keeps one address for its
whole life, on whichever node its Pod runs:

1. **Claim.** The operator creates a cluster-scoped Cilium multi-pool
   `CiliumPodIPPool` named `qrab-nic-<a>-<b>-<c>-<d>` that holds only that /32
   (`maskSize: 32`). The name is the address, so `create` is the atomic claim:
   `AlreadyExists` means another NIC has it and the next free address is
   tried. Pools carry `qrab.yapplesauce.com/owner-uid` and `nic` labels and the
   finalizer `qrab.yapplesauce.com/nic-address`; the VM carries the same
   finalizer from before its first claim. `status.nics[].address` shows it.
2. **Pin.** The NIC Pod is annotated `ipam.cilium.io/ip-pool: <pool>` and
   `ipam.cilium.io/require-pool-match: "true"`, so Cilium gives it exactly that
   address. The guest's static NoCloud network-config therefore never goes
   stale. NIC pools are never listed in Cilium's `multiPoolPreAllocation`.
3. **Announce.** While a Pod holds the address, an `FRRConfiguration` of the
   same name in `frr-k8s-system` selects that Pod's node and announces the /32
   to the node's BGP neighbors. The operator copies the ASN (and VRF) and the
   neighbors from the node's base configuration, the one labelled
   `qrab.yapplesauce.com/nic-route-base: "true"` whose `matchLabels` select the
   node; frr-k8s merges the two. The route follows the Pod when it moves and is
   withdrawn while no Pod holds the address. The node mesh and the VM host
   accept /32s from the NIC range only.
4. **Fence.** A pool stays allocated to the CiliumNode that last used it until
   that node's agent frees it. If that Node is gone or not Ready and no Pod of
   the NIC is bound to it any more, the operator removes the pool from that
   CiliumNode's `spec.ipam.pools.requested` and `.allocated`, so the new Pod
   can take the address elsewhere. A Pod still bound to an unreachable node
   (Terminating) keeps the claim: as for a StatefulSet, only deleting it
   (`--force`) declares the old process gone.
5. **Release.** When the NIC is removed or the VM deleted, the operator waits
   until no Pod of the NIC exists, then deletes the route, the pool's finalizer
   and the pool. A periodic sweep releases pools whose VM was force-deleted.

The operator needs cluster-wide RBAC for this (pools, CiliumNodes, Nodes,
FRRConfigurations: `contrib/kubernetes/operator/nic-addresses.yaml`). The
ValidatingAdmissionPolicy in the same file confines it: pools and routes only
as labelled `qrab-nic-<address>` objects inside the range, holding or announcing
only that /32 and accepting no routes, and CiliumNode edits only removing
`qrab-nic-*` entries. `check-nic-address-policy.sh` checks the policy live with
dry runs.

A cold move takes about 15–20 s (the new node's first CNI ADD waits for the
pool; Cilium's `ipam.ciliumNodeUpdateRate` is 1s). The addresses do not
overlap, so this does not by itself allow live migration: two Pods cannot hold
one address at once.

The forked Cilium (`forks/cilium`, running since 2026-11-15) lets a second Pod
hold the address during a handoff: a Pod annotated
`ipam.cilium.io/handoff-from: <namespace>/<pod>` gets that Pod's IP as a fenced
standby endpoint, which drops its own traffic until the node-local agent API
activates it. The operator does not use it yet. The ValidatingAdmissionPolicy
in `contrib/kubernetes/operator/nic-handoff.yaml` lets only the qrab operator
set the annotation, only when it creates a `vm-nic` Pod in `qrab` that takes
over a Pod in `qrab` from its own `qrab-nic-<address>` pool in the range. Nobody
may add, change or remove the annotation later. `check-nic-handoff-policy.sh`
checks it live with dry runs.

### Egress keeps the guest's address

Cilium masquerades pod traffic to anything outside `10.244.0.0/16` as the
node's IP. NIC pools opt out: every `qrab-nic-<address>` pool carries
`ipam.cilium.io/skip-masquerade: "true"`, so Cilium's multi-pool IPAM gives the
NIC Pod's endpoint the no-SNAT flag (eBPF masquerade, which the cluster uses,
honours it). The admission policy requires the annotation on create and
refuses removing it. Cilium reads it when it allocates the address, so it
applies to NIC Pods created after their pool carries it.

A guest's connections therefore leave the cluster from its own /32 and
survive the NIC moving to another node. Beyond the cluster:

- **The VM host** (`10.203.0.1`) learns each /32 over BGP and forwards
  `10.244.128.0/17` untranslated. Its `/etc/kube-vms/isolation.nft` admits
  guest connections to non-private destinations and LAN (`192.168.0.0/24`)
  connections to guests. Guests still cannot open connections to the LAN or any
  other private range. Reload it only through `/etc/kube-vms/edge-dnat.nft`,
  which includes it and re-inserts the edge DNAT jump.
- **The LAN router** (192.168.0.1) routes `10.244.128.0/17` statically via
  `192.168.0.251` and is the only NAT, towards the internet. LAN hosts see
  guests by their own addresses.

## Limits

- passt is a userspace stack: every packet is copied through the NIC Pod's
  sockets, and throughput is far below a kernel bridge or tap.
- Only TCP, UDP and ICMP/ICMPv6 echo cross passt. Other IP protocols (GRE,
  SCTP, ESP, multicast routing, raw L2) do not.
- Inbound ports exclude 7700 and the ephemeral range. Ports below 1024 are
  forwarded only where the node lets an unprivileged Pod bind them.
- At most 4 NICs per VM. Each NIC Pod costs about 320 MiB of memory (see
  [Resource cost](#resource-cost)).

## Rollout record (2026-11-13)

- Images from `3c4d6a8`: manager `cb0c0e23…` (with the patched passt), operator `4fbf5455…`,
  dashboard `da24e9a`. Gitops re-pinned the operator (ClusterSPIFFEID and KVM
  device plugin) before `kubectl apply -k contrib/kubernetes/operator-storage-api`,
  which also brought the CiliumNetworkPolicy RBAC and the CRD.
- `contrib/kubernetes/console.yaml` (now `examples/qrab-demo/console.yaml`)
  selects `component: vm`, so qrab-demo's BMC Service never targets NIC pods. Apply it with `-n qrab`: the file
  carries no namespace.
- Live test VM `netvm2`: CirrOS 0.6.3 clone, `nics: [{name: eth0,
  securityGroups: [nettest]}]`, `cloudInit.hostname: netcheck`, and a user-data
  script reporting to a listener pod in namespace `qrab-nettest`. Security
  group `sg-nettest` allows TCP 22 in from `qrab-nettest`, plus egress to the
  listener on 8000 and to kube-dns.
  - Outbound: the listener logged `GET /?from=netcheck&step=outbound-ok` from
    `10.244.4.194`, the NIC pod's IP. Guest traffic leaves with the NIC
    pod's identity.
  - The security group blocks egress: the guest's `wget http://1.1.1.1/`
    failed, and it reported `step=egress-blocked`.
  - Inbound: a pod in `qrab-nettest` read `SSH-2.0-dropbear_2020.81` from
    the NIC pod's IP on port 22, which passt forwarded to the guest.
  - Inbound outside the group: port 8080 was refused. Hubble shows
    `Policy denied DROPPED` from `qrab-nettest/probe3` to
    `qrab/netvm-nic-eth0:8080`.
  - Relay lockdown: TCP 7700 from a non-VM pod was refused. Hubble shows
    only the VM pod's flows to `netvm-nic-eth0:7700` FORWARDED.
- A state PVC carries a VM's TPM/NVRAM identity, bound to its UID. A new
  VirtualMachine must get a fresh PVC: reusing a deleted VM's PVC makes
  the manager refuse to start (identity adoption), and the storage guard
  then fences the disks.

## Rollout record (2026-11-14): stable addresses

- kubernetes-gitops `5516540` declared the Cilium default pool as
  `10.244.0.0/17` (each node kept its /24; the Cilium operator logged them
  orphaned from the old /16, then re-adopted), let the node mesh accept /32s
  from `10.244.128.0/17` and labelled the base FRRConfigurations
  `qrab.yapplesauce.com/nic-route-base`. No Cilium agent restarted.
- Images from `fe93197`: manager `859ae526…` on QEMU `37ae004c…` (stream
  patch) with passt `+qrab3`, and operator `ead30f6f…` with
  `QRAB_NIC_ADDRESS_RANGE=10.244.128.0/17`. Gitops re-pinned the operator
  before `kubectl apply -k contrib/kubernetes/operator-storage-api`, which
  also brought `nic-addresses.yaml`.
- Test VM `netvm3` (CirrOS clone, one NIC in security group `nettest`) got
  `10.244.128.1` from pool `qrab-nic-10-244-128-1`; kube01 and kube02 learned
  the /32 via kube03 over iBGP. A pod in `qrab-nettest` read the guest's SSH
  banner every second throughout.
- Move: with kube03 cordoned, deleting the NIC Pod brought it back on kube01
  with the same address, Ready after 27 s. The FRRConfiguration followed in
  the same second, kube03's CiliumNode released the pool, the VM Pod (same
  UID) never restarted, and the SSH banner returned 39 s after the delete.
  The first attempts found the three bugs fixed by passt 0002/0003 and the
  QEMU patch above.
- Deleting `netvm3` stopped its NIC Pod, withdrew the route and released the
  pool within 9 s; the VM was gone after 24 s.
- The VM host (`10.203.0.1`) accepts NIC /32s since its FRR prefix list
  `KUBE-IN` gained `seq 15 permit 10.244.128.0/17 ge 32 le 32` (applied live
  with a soft inbound refresh, 16:46Z). Test VM `netvm4` (`10.244.128.1` on
  kube03) was then routed from the host via `10.203.0.13`, and the host read
  its SSH banner directly. Deleting the VM withdrew the host route.

## Rollout record (2026-11-14): egress keeps the guest's address

- qrab 7737251 annotates NIC pools with `ipam.cilium.io/skip-masquerade`; operator
  9e732de6 (gitops 6127706, applied with `kubectl apply -k
  contrib/kubernetes/operator-storage-api`), since superseded by the Redfish
  release's eaf5dcbd built from 0492566, which includes it.
  `check-nic-address-policy.sh` passes with the two new denials.
- `contrib/kubernetes/egress-check.sh`, 18:46Z: VM `egresscheck` got
  `10.244.128.1` on kube03; its pool carried the annotation and its lxc entry
  `flags=0x0004` (skip-masquerade). A listener on the VM host saw the guest as
  `10.244.128.1`, and the guest reached `1.1.1.1` through the LAN router's NAT.
  An ordinary pod on kube03 reached the same listener as `10.203.0.13`, its
  node. The run's control-pod step then hit a transient
  `etcdserver: request timed out`.
- Later runs were refused with `StorageRefused budget-exhausted`: every run keeps
  its 1 GiB root clone.
