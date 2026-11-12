#!/usr/bin/env bash
# Kanidm groups that grant per-VM BMC/Redfish access (docs/operator.md,
# "Access through Kanidm groups"). Run as a Kanidm administrator after
# `kanidm login -D idm_admin`. Prints no credentials.
#
#   vm-access-groups.sh setup             one-time: qrab_vm_users and client scope maps
#   vm-access-groups.sh create NS VM      the VM's admins/viewers groups, then the CR patch
#   vm-access-groups.sh check NS VM       verify all of the above
set -euo pipefail
admin=${KANIDM_ADMIN:-idm_admin}
gate=qrab_vm_users
clients=(qrab-bmc qrab-redfish)
scopes=(openid profile groups_uuid)
k() { kanidm "$@" -D "$admin"; }

name() { # NS VM ROLE -> group name, refusing what Kanidm or the dashboard cannot parse
  local ns=$1 vm=$2 role=$3 group
  [[ $ns =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ ]] || { echo "invalid namespace '$ns'" >&2; exit 2; }
  [[ $vm =~ ^[a-z0-9]([a-z0-9.-]*[a-z0-9])?$ ]] || { echo "invalid VM name '$vm'" >&2; exit 2; }
  group="qrab_vm_${ns}_${vm}_${role}"
  [ ${#group} -le 64 ] || { echo "$group is longer than Kanidm's 64 characters" >&2; exit 2; }
  echo "$group"
}
exists() { k group get "$1" 2>/dev/null | grep -q '^uuid: '; }
uuid() { k group get "$1" | sed -n 's/^uuid: //p'; }
members() { k group list-members "$1" 2>/dev/null | sed 's/@.*//'; }
scope_map() { # client group -> the scopes it maps, one per line
  k system oauth2 get "$1" | sed -n "s/^oauth2_rs_scope_map: $2@[^:]*: //p" | tr -d '{}",' | tr ' ' '\n' | sed '/^$/d' | sort
}

case ${1:-} in
setup)
  exists "$gate" || k group create "$gate"
  for client in "${clients[@]}"; do
    # update-scope-map REPLACES the set; pass every scope.
    k system oauth2 update-scope-map "$client" "$gate" "${scopes[@]}"
    current=$(scope_map "$client" qrab_operators | tr '\n' ' ')
    case "$current" in
      "openid profile " | "groups_uuid openid profile ") k system oauth2 update-scope-map "$client" qrab_operators "${scopes[@]}" ;;
      *) echo "qrab_operators on $client maps '$current'; add groups_uuid by hand, keeping every scope" >&2; exit 1 ;;
    esac
  done
  echo "ok: $gate and the groups_uuid scope on ${clients[*]}"
  ;;
create)
  ns=${2:?NS VM}; vm=${3:?NS VM}
  for role in admins viewers; do
    group=$(name "$ns" "$vm" "$role")
    exists "$group" || k group create "$group"
    k group add-members "$gate" "$group"
  done
  admins=$(uuid "$(name "$ns" "$vm" admins)"); viewers=$(uuid "$(name "$ns" "$vm" viewers)")
  echo "Groups ready. To use them (this restarts the VM once):"
  echo "  kubectl -n $ns patch virtualmachine $vm --type=merge -p '{\"spec\":{\"console\":{\"adminGroups\":[\"$admins\"],\"viewerGroups\":[\"$viewers\"]},\"components\":{\"redfish\":{\"adminGroups\":[\"$admins\"],\"viewerGroups\":[\"$viewers\"]}}}}'"
  echo "Omit the redfish part when components.redfish is disabled."
  ;;
check)
  ns=${2:?NS VM}; vm=${3:?NS VM}
  gated=$(members "$gate")
  for role in admins viewers; do
    group=$(name "$ns" "$vm" "$role")
    exists "$group" || { echo "FAIL: $group does not exist" >&2; exit 1; }
    grep -qx "$group" <<<"$gated" || { echo "FAIL: $group is not a member of $gate" >&2; exit 1; }
    echo "ok: $group $(uuid "$group") ($(members "$group" | wc -l) direct members)"
  done
  for client in "${clients[@]}"; do
    for group in "$gate" qrab_operators; do
      [ "$(scope_map "$client" "$group" | tr '\n' ' ')" = "groups_uuid openid profile " ] ||
        { echo "FAIL: $group on $client maps '$(scope_map "$client" "$group" | tr '\n' ' ')'" >&2; exit 1; }
    done
  done
  echo "ok: $gate and qrab_operators map openid profile groups_uuid on ${clients[*]}"
  ;;
*) sed -n '2,9p' "$0" >&2; exit 2 ;;
esac
