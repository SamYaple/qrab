# Guest networking

The [connection atlas](connections.md#guest-network-and-migration-connections)
records each transport's encryption and authentication. See its
[TAP/VPN diagrams](connections/data-paths.md#guest-nic-how-tun-access-reaches-qemu)
and [capture guide](connections/capture.md) for descriptor handoff versus packet
traffic, anchor namespaces and the separate node conntrack stream.

A VM has no network interface unless `spec.nics` names one. Each guest NIC is
a kernel tap with vhost-net in its own restricted **anchor Pod**,
`<vm>-nic-<name>-<node>`, on the VM Pod's node. The anchor's IP is the NIC's stable
address and the guest's address on that NIC, its Cilium identity is the NIC's
identity, and its labels carry the NIC's security groups. Guest traffic never
enters the VM Pod's network namespace.

```yaml
spec:
  accelerator: kvm            # required with NICs
  nics:                       # at most 4, names unique
  - name: lan                 # DNS label, 1..15 characters
    securityGroups: [web, ssh]   # at most 8 DNS labels
```

Guest NICs need `accelerator: kvm` (the CRD refuses NICs without it: the KVM
device plugin's CDI spec brings qrab-netd's socket into the VM Pod), [stable
addresses](#stable-addresses) (`QRAB_NIC_ADDRESS_RANGE`; without it a VM with
NICs is `Blocked` with reason `InvalidSpec`) and qrab-netd on the VM Pod's
node.

## Why a Pod per NIC

- **The VM Pod's namespace stays private.** It holds the BMC (8080), Redfish
  (8081) and the manager's control socket. A guest on the same network
  namespace could reach those listeners, and policies selecting the VM Pod would
  have to treat guest traffic and BMC traffic as one identity.
- **Upstream Cilium gives one endpoint per Pod.** A second interface inside the
  VM Pod would share its identity and IP. A separate Pod gets its own
  endpoint, identity and address, which the cluster's BGP advertises (native
  routing). Nothing like OVN, Multus or a second CNI is involved.
- **The `qrab` namespace enforces Pod Security restricted.** Neither the VM
  Pod nor the anchor gets a hostPath, an added capability or a tun device. The
  privileged per-node agent qrab-netd builds the tap in the anchor's network
  namespace and hands its descriptors to the VM's manager, which never opens
  `/dev/net/tun` or `/dev/vhost-net` itself.

## Data path

```
guest virtio-net
  → vhost-net (the queues qrab-netd handed to the VM Pod's QEMU)
  → tap qt… in the anchor Pod's network namespace
  → the anchor's routes (the guest's /32 points at the tap)
  → Cilium (anchor identity and policies)
  → BGP-announced /32 (the NIC's stable address)
```

QEMU gets `-netdev tap,fds=…,vhost=on,vhostfds=…` with one queue per vCPU (at
most 8) and a `virtio-net-pci` device without an option ROM (`romfile=`), so it
never enters the boot order. Details are in [Tap datapath](#tap-datapath) and
[manager.md](manager.md).

The guest's gateway is the fixed link-local `169.254.1.1` on every node. It is
the tap's own address, so the anchor answers ARP for it and routes through it.
DNS sent to `169.254.1.1` is redirected to the anchor's resolver with
nftables. Outbound connections from the guest leave from the NIC's address,
and Cilium delivers traffic for that address to the anchor, which forwards it
to the tap.

The VM's NoCloud seed carries the configuration statically: each NIC is
matched by MAC, gets the NIC's address as a /32, a default route `on-link` via
`169.254.1.1`, and nameserver `169.254.1.1`. A VM with NICs gets a seed even
without `spec.cloudInit`. For guests that ignore the seed's network config
(CirrOS runs dhcpcd), qrab-netd answers DHCP on each tap with the same
contract as a static lease: only the guest's MAC, the NIC's /32, the gateway
as a classless static route (option 121) and as router, and DNS
`169.254.1.1`.

## Security groups

Each entry in `securityGroups` becomes the label
`sg.qrab.yapplesauce.com/<group>: ""` on the anchor Pod. Security-group rules
are ordinary CiliumNetworkPolicies selecting those labels:

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

Changing a NIC's `securityGroups` patches the anchor's labels in place. Cilium
recomputes its identity; the guest, its address and the VM Pod are untouched.

### Default allow, and a baseline

The operator creates no policy for a NIC. **An anchor that no security-group
policy selects is default-allow** in both directions: any source can reach
the guest, and the guest can reach anything. That is Cilium's rule for an
endpoint that no enforcing policy selects. Install a default-deny baseline for
every anchor and open only what each security group needs:

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
  # Empty rules: selecting the anchors turns on default deny both ways.
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

Anchors are labelled `app.kubernetes.io/component: vm-nic` and share
`app.kubernetes.io/instance: <vm>` with their VM Pod. A Service or policy meant
for the VM Pod must therefore also select `app.kubernetes.io/component: vm`, as
`console.yaml` does, so that anchors never receive BMC or Redfish traffic.

The operator deletes any CiliumNetworkPolicy `<vm>-nic-<name>-relay` that its
VM owns: these relay policies belong to the removed stream datapath (see the
[rollout records](#history-rollout-records-of-the-former-datapath)).

## Addresses and replacement

- **Deterministic MACs.** The guest MAC is `02:` followed by five bytes of
  SHA-256 over the VM UID and NIC name. It is unicast and locally
  administered. QEMU uses it, and qrab-netd installs a permanent neighbour
  entry for it in the anchor.
- **Ordering.** The operator claims each NIC's stable address first and
  renders the VM's manager config with `nics: [{id, mac}]` and the seed. The
  VM Pod does not wait for anchors: once it is scheduled, each NIC's anchor is
  created on its node, and the manager starts QEMU only after qrab-netd has
  plumbed every NIC. `status.nics` lists each NIC's `name`, `mac`, `podName`,
  `ip`, `address` and `ready` (the anchor's readiness gate).
- **Replacing an anchor.** Anchors hold no VM state, so a missing or Failed
  anchor is recreated on the VM Pod's node with the same address. A VM Pod on
  another node gets new anchors there. The running manager notices the new
  anchor (qrab-netd's `probe` names another anchor UID) and re-plugs the NIC
  with the new tap's queues ([Hot-attach](#hot-attach-and-detach)); the guest
  sees the NIC leave and return.
- **While the VM is off.** Existing anchors are left in place while
  `running: false`, and the NICs keep their addresses.
- **Removal.** Removing a NIC deletes its anchor, then releases its address.
  Deleting the VM stops its anchors first, then releases their addresses.

## Stable addresses

With `QRAB_NIC_ADDRESS_RANGE` set (live: `10.244.128.0/17`, carved out of the
Cilium default pool in kubernetes-gitops), each NIC keeps one address for its
whole life, on whichever node its anchor runs. Guest NICs require it:

1. **Claim.** The operator creates a cluster-scoped Cilium multi-pool
   `CiliumPodIPPool` named `qrab-nic-<a>-<b>-<c>-<d>` that holds only that /32
   (`maskSize: 32`). The name is the address, so `create` is the atomic claim:
   `AlreadyExists` means another NIC has it and the next free address is
   tried. Pools carry `qrab.yapplesauce.com/owner-uid` and `nic` labels and the
   finalizer `qrab.yapplesauce.com/nic-address`; the VM carries the same
   finalizer from before its first claim. `status.nics[].address` shows it.
2. **Pin.** The anchor is annotated `ipam.cilium.io/ip-pool: <pool>` and
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
`qrab-nic-*` entries or adding one `qrab-nic-<address>` pool in the range
(requesting one address, allocated exactly its /32) for a live migration's
pool move. `check-nic-address-policy.sh` checks the policy live with
dry runs.

A moved anchor's first CNI ADD on the new node waits for the pool (Cilium's
`ipam.ciliumNodeUpdateRate` is 1s). Upstream Cilium cannot let two Pods hold
one address at once, which live migration needs.

The forked Cilium (`forks/cilium`, running since 2026-11-15) lets a second Pod
hold the address during a handoff: a Pod annotated
`ipam.cilium.io/handoff-from: <namespace>/<pod>` gets that Pod's IP as a fenced
standby endpoint, which drops its own traffic until the node-local agent API
activates it. Tap [live migration](#live-migration) uses it for the target
node's anchor. The ValidatingAdmissionPolicy
in `contrib/kubernetes/operator/nic-handoff.yaml` lets only the qrab operator
set the annotation, only when it creates a `vm-nic` Pod in `qrab` that takes
over a Pod in `qrab` from its own `qrab-nic-<address>` pool in the range. Nobody
may add, change or remove the annotation later. `check-nic-handoff-policy.sh`
checks it live with dry runs.

### Egress keeps the guest's address

Cilium masquerades pod traffic to anything outside `10.244.0.0/16` as the
node's IP. NIC pools opt out: every `qrab-nic-<address>` pool carries
`ipam.cilium.io/skip-masquerade: "true"`, so Cilium's multi-pool IPAM gives the
anchor's endpoint the no-SNAT flag (eBPF masquerade, which the cluster uses,
honours it). The admission policy requires the annotation on create and
refuses removing it. Cilium reads it when it allocates the address, so it
applies to anchors created after their pool carries it.

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

## Tap datapath

The path is guest virtqueue → vhost-net → tap in the NIC's anchor Pod's
network namespace → Cilium, as for any Pod. Guest frames never enter the VM
Pod. The pieces:

- **The anchor** (operator, [operator.md](operator.md#guest-nics)) is a
  restricted Pod on the VM Pod's node that runs `qrab-manager anchor`. It
  holds the NIC's Cilium endpoint, the /32 from the NIC's pool and the
  security-group labels.
- **qrab-netd**, described below, builds the tap in the anchor's network
  namespace and sets the anchor's readiness gate.
- **The manager** ([manager.md](manager.md)) asks qrab-netd for the
  queues before each QEMU start.

`qrab-netd` (`projects/qrab-netd`) is a privileged per-node agent with host
PIDs. It runs as a DaemonSet in `qrab-node` (`contrib/kubernetes/netd/`) and
ships in the operator image with `ip` and `nft`. It watches the `qrab` Pods on
its node and handles only NIC anchors annotated
`qrab.yapplesauce.com/nic-datapath: tap`, which the operator sets on every
anchor, and leaves other Pods alone.
For each such anchor it finds a process of the Pod by its cgroup and enters
the Pod's network namespace. There it:

- creates a persistent multiqueue tap (`IFF_TAP|IFF_NO_PI|IFF_MULTI_QUEUE|IFF_VNET_HDR`)
  named `qt` plus 12 hex digits, with a MAC derived from the VM UID and NIC
  name. Both are the same on every node;
- gives the tap `169.254.1.1/32` and `eth0`'s MTU, routes the guest's /32 to
  it, and installs a permanent neighbour entry for the guest's MAC (the
  operator's `nic::mac`);
- enables forwarding, turns off reverse-path filtering, and redirects the
  guest's DNS (`169.254.1.1:53`) to the anchor's own resolver with nftables;
- gives `eth0` the placeholder `169.254.1.2/32` (scope link), then removes the
  /32 from `eth0` last, so Cilium still delivers the address to the anchor and
  the anchor forwards it to the tap. Without an address left on `eth0` the
  kernel flushes the anchor's default route; the agent checks the route
  survived;
- answers DHCP on the tap (see [Data path](#data-path)).

It reports the result as the Pod condition `qrab.yapplesauce.com/nic-plumbed`,
which anchors list as a readiness gate. The
`qrab-netd-anchor-status` ValidatingAdmissionPolicy confines that write to
this one condition on tap anchors of the agent's own node.

A VM's manager connects to `/run/qrab-netd/netd.sock` and sends
`{"nic":"lan","queues":N}` (`qrab_netd::wire::request`; the other operations,
used by live migration, are `prepare`, `activate`, `standby`, `forward` and
`unforward`). The KVM device
plugin's CDI spec mounts that directory into VM Pods read-only. The agent
resolves the peer (`SO_PEERCRED` pid → cgroup → Pod) and refuses anything
but a qrab VM Pod asking for its own NIC on this node. It also
refuses while any process still holds a queue of that tap. Otherwise it
attaches N fresh queues and opens N vhost-net descriptors, then sends both
sets with `SCM_RIGHTS`, ready for `-netdev tap,fds=…,vhost=on,vhostfds=…`.
The agent keeps no descriptors. Restarting it re-applies every plan, which
changes nothing, and leaves QEMU's queues alone. `qrab-netd request NIC [N]`
runs the client side for diagnosis.

At start it opens `/dev/net/tun` and `/dev/vhost-net`, which also loads
`vhost_net`. If either fails it exits with a message naming the device.
Checks:

- `cargo test -p qrab-netd`: unit tests and doctests.
- Its `tests/netns.rs` builds a plan in throwaway namespaces, passes a
  working queue to another namespace and checks the default route survives;
  it also adds and sweeps forward routes and gets a lease from the DHCP
  responder over a veth. It needs root, `ip`, `nft` and `wg`, and skips
  without them. The nodes have no `nft` or `wg`, so run it with
  `contrib/check-netd-netns.sh`: in a throwaway privileged Pod from the
  deployed qrab-netd image on a node, failing if any test skipped. All five
  passed that way on kube03 (kernel 7.0) on 2026-11-15; before that the two
  VPN tests had only ever skipped on the nodes.
- `contrib/kubernetes/netd/check-status-policy.sh` checks the status policy
  with dry runs.

## Live migration

A VM with NICs live-migrates ([migration.md](migration.md)) with each NIC's
address, MAC and security groups unchanged. The NIC switches nodes inside
QEMU's final stop-and-copy pause, so the guest sees no link change and no
extra loss beyond that pause. Egress stays unmasqueraded throughout.

1. **Standby anchor.** Before it creates the target VM Pod the operator
   creates the NIC's anchor for the target node, `<vm>-nic-<nic>-<target>`,
   annotated `ipam.cilium.io/handoff-from: qrab/<source anchor>` and
   `qrab.yapplesauce.com/migration: <migration>` (both outside the template
   hash, so the anchor is the VM's normal anchor after the move). The forked
   Cilium gives it the same address as a fenced **standby** endpoint
   (traffic dropped as `DROP_HANDOFF_STANDBY`). qrab-netd plumbs its tap as
   usual. The operator waits for the readiness gate, then starts the target
   with `incoming.node_ip` set to the target node's InternalIP.
2. **Prepare.** The target manager asks qrab-netd for the target taps' queues
   as for any start. The source manager sends `prepare` to its own qrab-netd
   and enables QEMU's `pause-before-switchover` migration capability.
3. **Switchover.** At `pre-switchover` (the guest is paused and the last dirty
   pages are about to go) the source manager, polling every 20 ms:
   - adds a **forward route** in the source host's namespace, the guest's /32
     via the target node's IP (`proto 252`, in table 252, which a rule at
     priority 200 consults before `main`), so stragglers that still reach
     the old node follow the guest. It has its own table because Cilium
     re-installs its multi-pool route for the /32 in `main`, to the
     now-standby endpoint (measured 16 s after a switch), and in a shared
     table the last writer wins;
   - asks the target manager, over a second SPIFFE mTLS connection to the
     same listener (hello channel `switchover`), to `activate`: the target's
     qrab-netd flips its Cilium endpoint from standby to active
     (`PATCH /v1/endpoint/{id}/handoff`);
   - sends `standby` to its own qrab-netd, which fences the source endpoint;
   - continues the migration (`migrate-continue`).

   If any step fails it undoes the earlier ones and the migration fails with
   the source still running. The manager logs `migration_switched` with the
   switch's duration in ms; `migration_completed` carries QEMU's
   `downtime_ms`.
4. **Announce.** After `cont` the target manager runs `announce-self`: QEMU
   sends RARP frames from the guest MAC and asks a virtio-net guest to
   announce itself, so anything on the path that learned the MAC updates.
5. **Route, pool and retirement.** After the target is adopted, the VM's
   reconcile moves the NIC's `FRRConfiguration` to the target node. While
   the source anchor (the new anchor's `handoff-from`) still exists, a
   second one, `qrab-nic-<address>-prev`, keeps announcing the /32 from the
   source node, which forwards to the target meanwhile: make before break.
   Without it the source withdrew before the VM host had the target's
   announcement, and the VM host routed the guest to its default route for
   5 s (measured below). The reconcile also adds
   the NIC's pool to the target's CiliumNode (`requested` one address,
   `allocated` the /32) while the source still lists it; the forked Cilium
   tolerates a one-address pool on two nodes and the target agent adopts the
   handed-off address into it, so the address stays IPAM-owned after the
   source anchor goes. Anchors
   not on the VM Pod's node are annotated `qrab.yapplesauce.com/retire-after`
   (20 s later) and then deleted, which also withdraws the `-prev`
   announcement. qrab-netd deletes forward routes whose anchor has been gone
   for 20 s. If the migration fails, the operator deletes the target anchors
   it created.
6. **Conntrack.** Twice, best effort: the target manager asks its node's
   qrab-netd (`ct-pull`), and that agent pulls the NIC's entries straight
   from the source node's qrab-netd (`qrab_netd::peer`, TCP 7911 in the
   nodes' host network namespaces). It rebases their lifetimes onto its own
   monotonic clock and writes them into its node's Cilium global IPv4
   conntrack maps. The target pulls the *precopy* itself before
   `migrate-incoming`, outside the guest's pause, so established flows are
   on the target node before it activates. Once its QEMU has accepted the
   incoming connection, QEMU answers no QMP until the stream starts, so a
   pull there that ran past the manager's 2 s QMP timeout failed the
   migration. The source asks for the *delta* over the switchover channel
   once QEMU reports the migration completed, for flows opened in between. Established flows then keep their verdicts
   on the new node, including flows only the guest sends on, which groups
   denying new egress would otherwise drop for good. A flow opened after the
   precopy loses its packets on the new node until the delta lands, a few
   hundred milliseconds (TCP retransmits them). The export streams
   bpftool's dump: a node's map can hold tens of thousands of entries (16 MB
   of JSON), while qrab-netd's memory limit is 128 MiB. qrab-netd imports
   only entries of the caller's own NIC whose other side is not local to the
   node (a local Pod or the host), so an import cannot open another
   workload's policy; service entries (node-local IDs) and entries beyond 384
   per NIC are not copied.

`contrib/check-nic-ct-handoff.sh` is the conntrack oracle. The guest's group
admits TCP 5000 in and no new egress. The guest pushes a counter line every
0.5 s to a client on the VM host, which only reads, so after the handshake
the guest's segments exist on the new node only through a copied entry. The
oracle failed as intended on a release without the copy (the stream stalled
for good). With the copy after the switch only, it passed one move (0.76 s
gap) but failed others: a 6.77 s gap with the entry present, and once a
stall because qrab-netd was OOM-killed exporting kube03's 36,000-entry map.
With the streamed export, the target-side precopy and the node-to-node
pull (manager `78f1f287`, operator `42cae9cc`), three moves passed with
stream gaps of 0.55, 0.72 and 0.56 s, each with a precopy and a delta.
The check now also requires the managers' reports of imported entries.

`contrib/check-nic-handoff-detail.sh` measures the handoff's parts. A probe
Pod on every node pings the guest every 0.1 s, each node's Cilium agent
logs the guest's ipcache entry and the node's routes for the /32 every
0.1 s, and the VM host pings and logs its own route. Over four fully
measured moves on 2026-11-15 (manager `5b93cb27`, operator `cba15278`),
relative to the end of the manager's switch:

| Part | Measured |
|---|---|
| Whole switch (forward, activate, standby) | 7–30 ms |
| Forward route, target active, source standby done | −23…−4 ms, −5…−2 ms, −2…−1 ms |
| ipcache names the new node, every node | +29…+119 ms |
| Longest probe gap, every node (pings every 0.1 s, 1 s timeout) | 0.10–1.11 s (one ping lost in the pause) |
| Hubble drops of the guest's address | none (no standby or host-policy drops) |
| VM host's route moves to the new node | +16.3…+17.2 s |
| Source sweeps its forward route | +59.7…+80.5 s |

In one of the four (kube02 → kube03) the VM host had no route for the
guest from +17.3 s to +22.3 s: kube02 withdrew before kube03's announcement
arrived. The VM host lost 47 pings, and kube01's probe, which reached the
guest through the VM host's route, saw a 5.11 s gap. An earlier run's move
failed the same way (a 5.13 s gap on a third node) before the check printed
routes. That matches the
soak's earlier 12–31 lost pings in 3 of 20 moves. The `-prev` announcement
removes the window. It then exposed the second fault: from +15.9 s the old
node's Cilium pool route had replaced its main-table forward route, and two
nodes' probes lost 22 s. With the forward route in table 252 (operator
`3db5c074`), three moves lost no ping outside the pause on any node, and the
VM host's route moved to the new node in one step (+19 to +47 s, while the
old node still announced and forwarded).

`contrib/check-nic-migration.sh` moves a CirrOS VM around three nodes while
pinging it, probing its SSH and receiving its per-second egress reports. It
requires the same boot id, TPM state and UEFI variables throughout, the
anchor and the VM host's route on the new node, egress from the guest's own
/32, no gap over 2 s, and that one TCP connection the guest opened before
the first move (a counter line per second to the VM host) survives every move.

Measured on 2026-11-15 (manager `0725c44f`, operator `a9eb8b99`, CirrOS,
2 vCPU, 512 MiB, pinging every 0.2 s):

| Move | NIC switch | QEMU downtime | Longest ping gap | Pings lost | Stream gap |
|---|---|---|---|---|---|
| kube03 → kube01 | 7 ms | 126 ms | 0.25 s | 0 | 1.12 s |
| kube01 → kube02 | 12 ms | 283 ms | 0.43 s | 1 | 1.25 s |
| kube02 → kube03 | 8 ms | 104 ms | 0.23 s | 0 | 1.05 s |

The NIC switch adds about 10 ms to QEMU's pause, and loss stays within the
pause. The guest kept its boot id, TPM state and UEFI variables, its egress
arrived from its own /32 after every move, and its TCP stream was never reset
(the stream and report gaps are the 1 s send interval plus the pause). One
SSH probe connection timed out during the second move: its SYN fell in the
283 ms pause, and the retransmit after Linux's 1 s initial timeout came too
late for the probe's 1 s limit. The kube01 anchor showed as
`ready (handoff-standby)` in `cilium-dbg endpoint list` until the switch, and
kube03's forward route was swept once its retired anchor's 20 s grace passed.

A second run on operator `c7e1b3b3`, which adds the pool move (the check now
requires the new node's CiliumNode to request and hold the pool), switched
in 7, 7 and 8 ms with QEMU downtime of 135, 119 and 111 ms, lost two pings in
all (each within a pause) and no SSH probe; the old node dropped the pool
once its anchor was retired.

### Conntrack peer SPIFFE enrollment

The updated netd peer listener on host TCP 7911 requires **TLS 1.3 mutual
SPIFFE authentication** in both directions. Its ID is
`spiffe://<local-domain>/kubernetes/qrab-node/qrab-netd/<node-name>`. The local
SVID must match `NODE_NAME`; the remote expected ID comes from current
nonterminating anchor Pods' `status.hostIP`/`spec.nodeName`, not a field in the
peer request. Unknown/ambiguous mappings fail closed. The source's on-link and
same-VM/same-NIC authorization still runs before exporting any entries.

The daemon has at most four incoming peer connections (including handshakes),
five-second TLS/request/write bounds, a one-second node lookup/connect bound,
256-byte request frames and 256KiB response frames; exports retain the 384-entry
cap. TLS completes before the address is read or the export callback runs.
The client bounds TLS plus the whole exchange to five seconds. A slow export
retains its incoming connection slot until the blocking backend completes,
even when the client has gone. The manager/VM never receives raw entries.

The library-only manager client builds without netd's TLS/Kubernetes dependencies.
No plaintext peer mode or opportunistic TLS fallback is provided. An old peer
cannot exchange state with the updated daemon. Missing SVIDs disable only peer
handoff; local TAP/DHCP/VPN plumbing remains available while netd retries SPIRE.
Established source identity follows Workload API certificate/bundle rotation.

Source preparation and rollout are separate:

1. Stage [clusterspiffeid.yaml](../contrib/kubernetes/netd/clusterspiffeid.yaml)
   in kubernetes-gitops. It binds node name from the Pod spec, namespace
   `qrab-node`, dedicated ServiceAccount `qrab-netd`, container `netd`, UID 0
   and the reviewed published image digest. The SPIRE controller adds the Pod
   UID selector. Do not give VM Pods or the KVM plugin the node identity.
2. Release the operator image containing netd and pin the **same digest** in
   registration and DaemonSet. Stage the CSI Workload API mount/environment in
   [daemonset.yaml](../contrib/kubernetes/netd/daemonset.yaml). The current reviewed pins are recorded in `images.lock.json` under
   `netd` and `kvm_device_plugin`; these artifacts share the operator image
   repository but have a release lifecycle separate from the operator
   Deployment. The examples are not applied automatically.
3. Verify enrollment and `peer_identity_ready` on each node, then verify a
   controlled NIC migration and conntrack handoff. A mixed old/new peer period
   fails the best-effort conntrack copy, not TLS open; coordinate the node roll
   before using migration as an availability check. No image publication or
   cluster rollout is implied by merging this source change.

Tests use fixture SVIDs over the real Unix Workload API and loopback TLS:
positive exchange/refusal, wrong local/peer IDs, untrusted CA, no client
certificate, plaintext client, malformed/oversized frames, stall deadline,
ambiguous host/node metadata and certificate rotation. These are transport
proofs without requiring a cluster, root, KVM or a guest image.

The first rollout completed on 2026-11-15 with node artifact `88818c3f…`
(source `b2022f5`), identity-ready logs on kube01–03 and two live conntrack
handoffs preserving the server-push stream (0.54s and 0.57s longest gaps).
See the [dated rollout evidence](connections/platform-evidence.md#netd-peer-mtls-rollout-2026-11-15-22022209-utc)
for the digest, GitOps merge and verification boundary.

## Hot-attach and detach

NICs can be added to or removed from `spec.nics` while the VM runs; the VM
Pod and the guest stay up.

- **PCI layout.** Every KVM VM (with stable NIC addresses configured) boots
  with four `pcie-root-port` devices `nicport0..3` (manager config
  `nic_slots: 4`), and every NIC sits on its own port, since q35's root bus is
  not hot-pluggable. The manager gives boot NICs slots in order and a new NIC
  the lowest free one.
- **Delivery.** The VM's template hash and the operator's restart decision
  leave out `nics` and the seed's network-config, so a NIC change patches the
  VM ConfigMap instead of replacing the Pod. The kubelet refreshes `vm.json`
  in the running Pod (up to about a minute), and the manager re-reads it each
  heartbeat.
- **Attach.** The operator claims the new NIC's address and creates its
  anchor as usual. The manager asks qrab-netd for the tap and vhost-net
  queues, hands them to QEMU with QMP `getfd` (SCM_RIGHTS on the QMP socket,
  [protocol.md](protocol.md)), then `netdev_add tap,fds=…,vhost=on` and
  `device_add virtio-net-pci,bus=nicport<slot>`. Refusals (the anchor still
  plumbing) back off 1 s, 2 s, 4 s … up to 60 s.
- **Detach.** `device_del`, then, once the guest has released the device (it
  is gone from `/machine/peripheral`; asked again after 30 s), `netdev_del`.
  The operator deletes the anchor and releases the address.
- **Guests.** A hot-added NIC is configured by qrab-netd's DHCP (the seed
  lists it from the next start); the guest must run DHCP on new interfaces.
- **Migration and power cycles.** An outgoing migration is refused while a
  NIC change is in flight; otherwise the source sends its live NICs and slots
  in the migration offer and the target recreates exactly those. An in-pod
  power cycle boots with the ConfigMap's current NICs and seed.

`contrib/check-nic-hotplug.sh` adds a NIC to a running CirrOS VM, removes it,
replaces the first NIC's anchor and finally migrates the VM with a hot-added
NIC, requiring the same VM Pod and boot id throughout:

Measured on 2026-11-15 (manager `47d2fca9`, operator `a3e8430b`, CirrOS,
1 vCPU):

| Step | Result |
|---|---|
| Add a second NIC | reachable from the guest after 70 s (71 s in an earlier run), same Pod and boot |
| Remove it | unplugged after 82 s (71 s), anchor and pool released |
| Delete the first NIC's anchor | the guest reaches the host through it again after 31 s |
| Live-migrate with the hot-added NIC | both NICs answer on the new node, same boot |

Nearly all of the add and remove time is the kubelet refreshing the ConfigMap;
the QMP steps take milliseconds. Re-attach is the anchor's recreation and
plumbing plus up to one 5 s probe interval.

`CYCLES=5` repeats the add and remove on one VM. On manager `5b93cb27` and
operator `cba15278` (2026-11-15), the five cycles took 70–88 s to attach and
60–74 s to detach. The NIC got the same address and `eth1` each time, the
manager attached it exactly five times with no refused try, the VM Pod and
boot id never changed, and the anchor replacement (34 s) and migration steps
passed afterwards.

## VPN NICs

`spec.nics[].vpn.secretName` turns a NIC into a WireGuard VPN NIC: all of its
guest traffic goes through a tunnel that the NIC's anchor terminates, and
nothing goes around it.

```yaml
nics:
- name: vpn
  securityGroups: [vpn-egress]      # must admit UDP to the VPN endpoint
  vpn: {secretName: vpn-proton}     # a Secret in the VM's namespace, named vpn-*
```

- **Config.** The Secret's key `wg.conf` is a wg-quick file with one
  `[Peer]`, as ProtonVPN and most providers export it (`PrivateKey`,
  `Address`, `DNS`, `MTU`; `PublicKey`, `PresharedKey`, `AllowedIPs`,
  `Endpoint`, `PersistentKeepalive`). The endpoint must be an IPv4 literal;
  wg-quick hooks (`PostUp` …) are refused. Only Secrets named `vpn-*` can be
  referenced, so a VM cannot have other Secrets mounted.
- **Where the key lives.** The operator mounts the Secret read-only (0400,
  root) into the NIC's anchor, whose own process cannot read it. qrab-netd
  reads it through the anchor's mount namespace and hands it to `wg syncconf`
  on stdin. The key never reaches the VM Pod or the guest, and qrab-netd
  never logs it.
- **Datapath.** In the anchor's namespace qrab-netd creates `wg0` (fixed
  ListenPort 51820, keepalive 25 s), whose UDP leaves through `eth0` from the
  anchor's pool /32, which stays on `eth0` as the tunnel's outer address.
  The tap's traffic is steered into table 100 (`default dev wg0`, with a
  blackhole fallback). nftables in the anchor (installed before anything
  else) forward only tap↔`wg0` and admit from the tap only DHCP and pings of
  the gateway: with the tunnel down nothing of the guest's leaves. The guest
  gets the tunnel's address, DNS and MTU by DHCP; its seed says `dhcp4: true`
  for that NIC.
- **Security groups** select the anchor as for any NIC, so they now govern
  the tunnel's UDP (admit the endpoint's port). Inbound traffic is whatever
  the provider forwards into the tunnel.
- **Live migration** works as for plain NICs: the standby target anchor
  builds the same `wg0` (its UDP dropped while standby), and once active it
  handshakes from the same address and port, so the server sees no roam.
- **Server switches.** qrab-netd compares a digest of the mounted config
  every 10 s, so a rewritten Secret (a ProtonVPN server switch) is applied on
  the next tick after the kubelet syncs it, not at the next 60 s re-verify.
- **Port forwarding.** A config carrying `# qrab: port-forwarding` (the
  ProtonVPN broker writes it when the certificate allows forwarding) makes
  qrab-netd keep a NAT-PMP (RFC 6886) mapping with the tunnel gateway (the
  config's first DNS, 10.2.0.1 at Proton): UDP and TCP, lifetime 60 s,
  renewed every 40 s (10 s after a failure), as Proton documents it
  (`natpmpc -a 1 0 <proto> 60`). The provider accepts only the guest's
  tunnel address, which the guest owns, so qrab-netd sends from the tap's
  gateway address 169.254.1.1 and the kill switch's `natpmp` chain
  source-NATs exactly those packets (to the gateway, UDP 5351, out `wg0`)
  to the tunnel address; conntrack brings the replies back to qrab-netd,
  never to the guest. The forwarded public port arrives at the guest's
  tunnel address on the same port number. qrab-netd reports it as anchor
  condition `qrab.yapplesauce.com/vpn-port-forward` (True with the port as
  its message, False with why not; a migration's standby anchor gets no
  reply until it is active), and the broker copies it to the profile's
  `status.forwardedPort`. The guest has to listen on that port, and should
  look it up there: it changes when the server does.

`contrib/check-vpn-nic.sh` runs against a lab provider (a WireGuard server
Pod that NATs to the world) or, with `VPN_SECRET`, a real config.
`tests/netns.rs` checks the datapath as root: a provider namespace answers
through the tunnel, plaintext never reaches the uplink, and with `wg0` gone
nothing leaves.

Measured on 2026-11-15 with the lab provider (operator `b8abf736`; the
provider Pod on kube01, the VM on kube03):

- The guest got the tunnel's 10.66.0.2 by DHCP; its egress reached the VM
  host from the provider's node (10.203.0.11), never from the anchor's pool
  address.
- The private key was in neither the VM Pod's filesystem nor qrab-netd's
  logs.
- Kill switch: with the provider's `wg0` down for 15 s no report arrived and
  none leaked from the pool address; reports resumed 1 s after it came back.
- After a live migration kube03 → kube01 the tunnel resumed from the new
  anchor with the same boot id.

### ProtonVPN

ProtonVPN is the supported provider; qrab keeps its `vpn-*` Secrets itself
instead of having them pasted in (towl container `protonvpn-nics`).

- **Account access.** The Proton account is a full account (Mail, Drive), so
  its password and TOTP never enter the cluster. `qrab-proton login` runs on
  a workstation and stores only the session (UID and refresh token) in
  Secret `qrab/proton-session`. Proton rotates the refresh token on every
  refresh, so its holder writes the new one back before using it.
  Run `qrab-proton login` (prompts for email, password, a TOTP code if the
  account has two-factor, and a browser step if Proton asks for human
  verification; `--print` writes the manifest instead of applying it). The
  command never refreshes the session it handed over: two holders refreshing
  one session would make Proton revoke it. `qrab-proton logout` revokes it;
  stop the broker first.
- **Keys.** Each profile has its own Ed25519 key; its X25519 form is the
  WireGuard key. Proton certifies the Ed25519 public key
  (`/vpn/v1/certificate`, at least a day, renewed with the same key), and the
  certificate binds the connection features: NetShield level, port
  forwarding, Moderate NAT and VPN Accelerator. A certified key works on any
  Proton server, so changing servers changes only the peer.
- **Servers.** `/vpn/v1/logicals` and `/vpn/v1/loads` need a session.
  Auto selection filters by the account's tier, status, features and
  location, then takes the lowest Score, Proton's own ranking; Secure Core
  and Tor servers only when asked for, and port forwarding only on P2P
  servers. A working server is kept while it stays eligible.
- **Profiles.** A `VpnProfile` is one Proton connection for one NIC; the
  broker keeps Secret `vpn-<profile>` for it, which the NIC references:

  ```yaml
  apiVersion: qrab.yapplesauce.com/v1alpha1
  kind: VpnProfile
  metadata: {name: ch-p2p, namespace: qrab}
  spec:
    proton:
      country: CH            # or server: CH#12 for a fixed server
      portForwarding: true   # P2P servers only
      netShield: 2           # 0 off, 1 malware, 2 + ads and trackers
  ---
  # in the VirtualMachine
  nics:
  - name: vpn
    securityGroups: [vpn-egress]   # admit UDP 51820 to any address: servers change
    vpn: {secretName: vpn-ch-p2p}
  ```

  `kubectl -n qrab get vpnp` shows each profile's server, country, load and
  readiness; the status also carries the entry IP, the certificate's expiry
  and why a profile is not ready (`NoServer`, `CertificateFailed`,
  `SessionRevoked`, `AppVersionRejected`). Profiles beyond the plan's
  simultaneous connections are configured but flagged.
- **The broker** (`qrab-proton broker`, Deployment `qrab-proton-broker`,
  `kubectl apply -k contrib/kubernetes/proton`) is the session's only holder.
  Every 15 s it lists profiles, keeps a key per profile in Secret
  `proton-keys`, renews certificates (a week, at half-life), and applies each
  profile's Secret (server-side apply, a no-op when unchanged). It refreshes
  the catalog every 6 h and loads every 15 min, and publishes them for the
  dashboard in ConfigMap `proton-catalog` (gzip JSON, no addresses or keys).
  When Proton is unreachable or the session is gone it reports that and leaves
  existing Secrets alone, so tunnels keep running until their certificates
  lapse.
- **Image pulls.** Zot authorizes pulls per ServiceAccount, so
  `qrab-proton-broker` needs its pull identity in kubernetes-gitops
  (`infrastructure/qrab-identity`, `infrastructure/pull-admission`) and the
  node pull policies applied (`spiffe-node-pulls.py --apply` on each node)
  before its Pod can start.
- **Tests.** `cargo test -p qrab-proton --test broker` covers session
  rotation and reconciliation against the fake API. With
  `QRAB_LIVE_BROKER_KUBECONFIG` set to a kubeconfig that impersonates
  `system:serviceaccount:qrab:qrab-proton-broker`, `--test live_broker` runs
  full passes against the cluster (fake Proton API, real RBAC and admission
  policy): profile Secret, status, stored session rotation, catalog
  ConfigMap, a live server switch with the same key, and garbage collection
  with the profile; it refuses to run when a real `proton-session` exists.
  `contrib/kubernetes/proton/check-policy.sh` checks the write confinement.
- **Live check.** `bash contrib/check-proton-vpn.sh` (after
  `qrab-proton login`) runs the whole path against the real account: a
  profile with port forwarding, a CirrOS VM whose `vpn` NIC is its default
  route and whose `ctl` NIC carries only the script's command channel (the
  tunnel cannot reach the VM host's private address), the guest's egress IP
  as a public echo service sees it, a TCP connection from the VM host's own
  uplink to `<egress IP>:<forwardedPort>` reaching a listener in the guest,
  the key absent from the VM Pod and qrab-netd's logs, a manual switch to a
  server in `SWITCH_COUNTRY` (default CH), and a live migration. `KEEP=1`
  leaves everything up after a failure. Measured on 2026-11-15 (operator
  `f3d7d09d`): auto selection chose US-GA#442 with a 7-day certificate; the
  guest egressed as a Proton address, never the VM host's; forwarded port
  41476 reached the guest from outside on the first try; switching to CH#496
  changed the guest's egress 41 s after the patch (58 s in a second run)
  and its new port 56682 worked; after a migration kube03 → kube01 the same
  boot kept the same egress and port.
- **Least privilege.** The broker reads only `proton-session` and
  `proton-keys` (by name) and writes profile Secrets without reading them.
  `ValidatingAdmissionPolicy` `qrab-proton-broker-writes` limits its writes to
  `vpn-<profile>` Secrets controller-owned by that profile and holding only
  `wg.conf` (never a `vpn-*` Secret it did not already own), the two Secrets
  above, and ConfigMap `proton-catalog`; it never deletes (profile Secrets go
  with their profile by garbage collection). Its CiliumNetworkPolicy allows
  egress only to the Kubernetes API, DNS for `vpn-api.proton.me`, and that
  host on 443.
- **The API is not public.** The `x-pm-appversion` header
  (`qrab_proton::api::APP_VERSION`) breaks when Proton retires a client
  version: a 5003 reply is reported as such, never retried in a loop.
- `cargo test -p qrab-proton` runs the client against an in-process fake
  Proton API with real SRP and a modulus signed by Proton's key: login, TOTP,
  human verification, refresh rotation, the catalog and certificates, and a
  `wg.conf` that qrab-netd's own parser accepts.

## Security

What is tested, and how:

- **Inputs of qrab-netd** (`cargo test -p qrab-netd`): seeded fuzz tests send
  50,000 random and mutated DHCP packets (none panics, none is answered for a
  foreign MAC, every answer carries the lease) and 20,000 random wire
  requests. A whole request must arrive within 5 s; NIC names are DNS labels;
  `queues` is bounded before any descriptor is opened. At most 16 control
  connections are served at once.
- **Who gets descriptors** (unit tests and `contrib/check-nic-security.sh`):
  qrab-netd authenticates the peer by `SO_PEERCRED` → cgroup → Pod, and hands
  a VM Pod only its own NICs; a NIC the VM lacks is refused, and a tap QEMU
  already holds is refused too.
- **What a guest can reach** (`contrib/check-nic-security.sh`, live
  2026-11-15): a guest in a group admitting only world egress reached the
  world but not its own VM Pod's BMC (8080), Redfish (8081) or migration
  port (7800), the node's kubelet (10250) or SSH, cluster DNS, another VM's
  NIC, or the anchor's own addresses. With the group widened to the whole
  cluster, its VM Pod's BMC, Redfish and migration ports still refused it;
  the kubelet and an ungrouped VM answered, as they would any Pod, so groups
  should not admit `cluster` or `host` without need.
- **Spoofing**: a datagram the guest sent from a forged source address never
  arrived (Cilium drops traffic whose source is not the endpoint's address),
  while its genuine datagram did.
- **VPN NICs**: see [VPN NICs](#vpn-nics) (no plaintext on the uplink, kill
  switch, key confinement).
- **Conntrack handoff** (`cargo test -p qrab-netd`, `peer` and `ct`): no VM
  manager carries entries, since a manager sits inside its VM's trust
  boundary (QEMU runs beside it). The target node's qrab-netd fetches them
  from the source node's qrab-netd, which listens in the host network
  namespace (a port only host processes can bind). The source serves an
  address only to the node of another anchor of the same VM's same NIC
  holding that address, judged from the API server's Pods, and the target
  imports only that NIC's entries whose other side is not local to its node.
  The exchange itself is plain TCP between nodes, as trusted as their iBGP
  sessions. Connections from anything not on one of the node's host links
  (Pods, guests, routed hosts) are dropped before any read or API lookup,
  at most four are served at once, and a seeded fuzz test sends 200 random
  requests. A port that cannot be bound is retried, never fatal.
- **Forward routes**: `forward` installs the caller's own /32 only, and only
  via an address on one of the node's host links (another node), checked
  locally inside the guest's pause.

## Stability

`contrib/check-nic-soak.sh` (`PHASES` selects `1`, `2` or both):

1. MOVES (default 20) live migrations through `check-nic-migration.sh`, then
   a cluster-wide audit. The VM must leave no address pool, FRRConfiguration
   (including `-prev`), Pod, CiliumEndpoint, CiliumNode pool entry or forward
   route behind.
2. A fresh VM pinged every 0.2 s and streaming over one TCP connection while
   qrab-netd, then the Cilium agent, restart on its node. The longest gap
   must stay within `RESTART_GAP_S` (3 s), without a reset.

Measured on 2026-11-15 (manager `5b93cb27`, operator `cba15278`):

- 20 consecutive moves passed: switch 6–31 ms (one 116 ms), QEMU downtime
  86–186 ms, the guest's TCP stream intact. Moves 2, 7 and 14 also lost
  12–31 pings, and the SSH probe failed 10 times, 40–50 s after their
  moves began. That fits the BGP withdraw-before-announce window measured
  under [Live migration](#live-migration), which `-prev` closes.
- On manager `78f1f287` and operator `3db5c074` (make-before-break routes,
  forward routes in table 252), 20 consecutive moves passed: switch 13–23 ms
  (one 149 ms), QEMU downtime 111–194 ms. At most one ping went unanswered
  per move (longest gap 0.41 s, pinging every 0.2 s), and the audit found
  nothing left behind. The SSH probe (a connection every 0.5 s with a 1 s
  limit) timed out once in 10 of the moves, each 33–44 s after the move
  began. That is consistent with a SYN lost in the pause and retransmitted
  after Linux's 1 s initial timeout, though the check does not log switch
  times to prove it.
- qrab-netd restart on the VM's node: longest ping gap 0.21 s, stream gap
  1.00 s (its send interval), stream intact. The guest holds its tap and
  vhost-net descriptors through QEMU; the agent re-plumbs nothing that
  works.
- Cilium agent restart on the VM's node: longest ping gap 0.21 s, stream gap
  1.00 s, stream intact. The active handoff endpoint kept its state.

## Limits

- Guest NICs need `accelerator: kvm`, stable NIC addresses and qrab-netd on
  the VM Pod's node.
- Each NIC has one IPv4 address, a /32 from `QRAB_NIC_ADDRESS_RANGE`.
- At most 4 NICs per VM.
- Conntrack is copied before and after the pause, not inside it: a flow
  opened during the RAM copy has no entry on the new node until the delta
  lands, so a packet of it the groups would not admit as new is dropped (TCP
  retransmits it). Flows through Cilium services and a NIC's entries beyond
  384 are not copied.
- A NIC change takes effect when the kubelet refreshes the VM ConfigMap,
  about a minute; a hot-added NIC needs the guest to run DHCP on it.
- A VPN NIC has one peer and IPv4 inside the tunnel; its endpoint must be an
  IPv4 literal.

## Benchmark (2026-11-15)

`contrib/check-nic-datapath.sh` measured the removed passt datapath and the tap
datapath under the same conditions before passt was removed:

- the same node (kube03), the operator 2b9633da with qrab-netd 1deec81;
- one CirrOS guest per datapath with 2 vCPUs, KVM and 512 MiB;
- 256 MiB per TCP transfer, using busybox `dd | nc` in the guest against a
  timed sink on the VM host (10.203.0.1);
- 50 pings each from the VM host and from a Pod on kube01.

| | passt (stream) | tap + vhost-net |
|---|---|---|
| guest → VM host, 1 TCP stream | 1.40 Gbit/s | 2.90 Gbit/s |
| guest → VM host, 4 TCP streams | 2.83 Gbit/s | 3.11 Gbit/s |
| VM host → guest, 1 TCP stream | 3.30 Gbit/s | 11.0 Gbit/s |
| ICMP RTT from the VM host | 0.24 ms | 0.48 ms |
| ICMP RTT from a Pod on another node | 0.38 ms | 0.67 ms |

Notes on reading the table:

- Guest sends are bounded by CirrOS's single-threaded busybox tools, which
  is why 4 streams add little over 1 in tap mode.
- passt answers ICMP echo itself in the NIC Pod, so its RTT never reached the
  guest. The tap RTT is the guest's own round trip.

Both datapaths passed the same probes:

- ingress from the VM host on an allowed port;
- a denied port dropped, shown in Hubble as `Policy denied DROPPED`;
- denied egress to the API server's ClusterIP;
- no reach from the guest to the VM Pod's BMC (8080) or Redfish (8081).

In tap mode the VM Pod's network namespace also held no tap and no guest
address. On these numbers tap became the only datapath.

## History: rollout records of the former datapath

The records below predate the tap-only datapath and describe the former
passt/stream datapath, removed on 2026-11-15: each NIC Pod ran passt behind a
one-connection TCP relay on port 7700, QEMU reached it with a `stream` netdev,
and each NIC had a relay policy `<vm>-nic-<name>-relay`. QEMU still carries
[`0001-net-stream-reset-send-state-on-disconnect.patch`](../contrib/images/qemu-patches/0001-net-stream-reset-send-state-on-disconnect.patch)
from that datapath.

### Rollout record (2026-11-13)

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

### Rollout record (2026-11-14): stable addresses

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
  The first attempts found three bugs, fixed by two passt patches and the
  QEMU patch above.
- Deleting `netvm3` stopped its NIC Pod, withdrew the route and released the
  pool within 9 s; the VM was gone after 24 s.
- The VM host (`10.203.0.1`) accepts NIC /32s since its FRR prefix list
  `KUBE-IN` gained `seq 15 permit 10.244.128.0/17 ge 32 le 32` (applied live
  with a soft inbound refresh, 16:46Z). Test VM `netvm4` (`10.244.128.1` on
  kube03) was then routed from the host via `10.203.0.13`, and the host read
  its SSH banner directly. Deleting the VM withdrew the host route.

### Rollout record (2026-11-14): egress keeps the guest's address

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
