#!/usr/bin/env bash
# Operator-run installation of one reviewed binary and the dedicated demo LUN.
# No password/credential values are accepted in argv or emitted to stdout.
set -euo pipefail
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
umask 077
[[ $(id -u) == 0 ]] || { echo 'Run this reviewed installation step as root.' >&2; exit 1; }
[[ $# == 2 || $# == 3 && ${3:-} == --allow-demo-automation ]] || { echo 'usage: install-demo.sh SOURCE_BINARY EXPECTED_SHA256 [--allow-demo-automation]' >&2; exit 1; }
allow_demo_automation=false
if [[ ${3:-} == --allow-demo-automation ]]; then allow_demo_automation=true; fi
recipient_uid=$(id -u bob)
[[ $recipient_uid -ge 1000 ]] || { echo 'Invalid bob recipient UID.' >&2; exit 1; }
source_binary=$1
expected_sha=$2
[[ $expected_sha =~ ^[a-f0-9]{64}$ ]] || { echo 'invalid expected artifact SHA256' >&2; exit 1; }
program=/usr/local/sbin/qrab-iscsi-target
marker=/var/lib/qrab-iscsi-installer.sha256
if [[ -e $program || -L $program ]]; then
  [[ -f $program && ! -L $program && $(stat -c %u "$program") == 0 && -f $marker && ! -L $marker && $(stat -c %u "$marker") == 0 && $(stat -c %a "$marker") == 600 && $(stat -c %a "$program") == 755 ]] || { echo 'Refusing an unowned existing program.' >&2; exit 1; }
  [[ $(sha256sum "$program" | cut -d' ' -f1) == "$(cat "$marker")" ]] || { echo 'Installed program differs from its owned installation marker.' >&2; exit 1; }
fi
install_dir=$(mktemp -d /usr/local/sbin/.qrab-iscsi-install.XXXXXXXX)
trap 'rm -rf "$install_dir"' EXIT
install -o root -g root -m 0755 -- "$source_binary" "$install_dir/program"
[[ $(sha256sum "$install_dir/program" | cut -d' ' -f1) == "$expected_sha" ]] || { echo 'Reviewed artifact SHA256 mismatch; nothing executed.' >&2; exit 1; }
mv -T -- "$install_dir/program" "$program"
printf '%s\n' "$expected_sha" > "$install_dir/marker"
chmod 0600 "$install_dir/marker"
mv -T -- "$install_dir/marker" "$marker"
/usr/bin/systemctl start sys-kernel-config.mount
"$program" bootstrap
"$program" install-unit
"$program" plan --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --size-mib 128
"$program" apply --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --size-mib 128
"$program" handoff --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --recipient-uid "$recipient_uid"
/usr/bin/systemctl daemon-reload
/usr/bin/systemctl enable --now qrab-iscsi-target.service
if $allow_demo_automation; then
  rule=/etc/sudoers.d/qrab-iscsi-demo
  cat > "$install_dir/sudoers" <<RULE
# qrab dedicated demo only; installed by reviewed install-demo.sh.
bob ALL=(root) NOPASSWD: /usr/local/sbin/qrab-iscsi-target status --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59, /usr/local/sbin/qrab-iscsi-target fence --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59, /usr/local/sbin/qrab-iscsi-target apply --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --size-mib 128, /usr/local/sbin/qrab-iscsi-target handoff --vm-uid e309f751-5dda-4271-a0f4-4454c9527a59 --recipient-uid $recipient_uid
RULE
  chmod 0440 "$install_dir/sudoers"
  /usr/sbin/visudo -cf "$install_dir/sudoers"
  if [[ -e $rule || -L $rule ]]; then
    [[ -f $rule && ! -L $rule && $(stat -c %u "$rule") == 0 && $(stat -c %a "$rule") == 440 ]] && cmp -s "$rule" "$install_dir/sudoers" || { echo 'Refusing an existing foreign demo sudo rule.' >&2; exit 1; }
  else
    install -o root -g root -m 0440 "$install_dir/sudoers" "$rule"
  fi
  /usr/sbin/visudo -cf /etc/sudoers
  printf 'Explicit demo-only automation grant installed: four exact commands, no wildcard arguments or root shell.\n'
fi
printf 'Dedicated target prepared. Credentials are in the reported recipient-owned0600 file; no existing CSI export was changed.\n'
