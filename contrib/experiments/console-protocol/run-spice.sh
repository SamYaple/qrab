#!/usr/bin/env bash
# Experiment B: SPICE beside the existing VNC console, on the experiment image built
# from the pinned, unmodified QEMU source with --enable-spice --enable-usb-redir
# (Containerfile.qemu-spice). Audio source: beep-disk.py's PC-speaker tone.
#   run-spice.sh <workdir> [image]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mkdir -p "$1" && cd "$1" && pwd)
image=${2:-localhost/qrab-experiment/qemu-spice:11.1.3}
python3 -I "$here/beep-disk.py" "$work/beep.img"

start() {  # extra chardev/device args...
  rm -f "$work/spice.sock" "$work/vnc.sock"
  podman run -d --name qrab-exp-spice --network=none --user 0 -v "$work:/run/x:z" "$image" \
    -machine q35,pcspk-audiodev=snd0 -m 128 -accel tcg -nodefaults -display none \
    -device VGA,vgamem_mb=16 -device qemu-xhci,id=usb -device usb-tablet,bus=usb.0 \
    -drive file=/run/x/beep.img,format=raw,if=ide \
    -vnc unix:/run/x/vnc.sock,share=force-shared \
    -spice unix=on,addr=/run/x/spice.sock,disable-ticketing=on,disable-copy-paste=on,disable-agent-file-xfer=on \
    -audiodev spice,id=snd0 -device intel-hda,id=hda -device hda-output,audiodev=snd0 "$@" >/dev/null
  for _ in $(seq 300); do [[ -S $work/spice.sock ]] && break; sleep 0.1; done || true
  sleep 2
}
finish() {
  podman inspect qrab-exp-spice --format '## qemu container: {{.State.Status}} exit={{.State.ExitCode}}'
  podman logs qrab-exp-spice 2>&1 | grep -v '^$' | tail -4 || true
  podman rm -f qrab-exp-spice >/dev/null
}
usbredir=(-chardev spicevmc,id=usbredir0,name=usbredir -device usb-redir,chardev=usbredir0,bus=usb.0)
port=(-chardev spiceport,id=port0,name=org.qrab.experiment -device isa-serial,chardev=port0)

echo '## B1. channel links, playback stream, single-client behaviour (usbredir + port configured)'
start "${usbredir[@]}" "${port[@]}"
python3 -I "$here/spice_link_probe.py" "$work/spice.sock" --seconds 3 --second-main || true
echo '## B1. VNC console on the same VM still answers'
python3 -I "$here/vnc_audio_probe.py" "$work/vnc.sock" --seconds 1 --no-audio-encoding | grep -E '"(version|size|name)"' || true
finish

echo '## B2. link one channel, close it, check the server survives (fresh VM each)'
for channel in 6 9 10; do
  for hold in 0 1; do  # 6 = record: no capture voice exists, so spice-server has no record channel
    start "${usbredir[@]}" "${port[@]}"
    python3 -I "$here/spice_channel_churn.py" "$work/spice.sock" "$channel" --hold "$hold" || true
    finish
  done
done

echo '## B3. link usbredir and port, close both, then link playback (fresh VM each)'
for channels in 9,10 10,9 10,10; do
  start "${usbredir[@]}" "${port[@]}"
  python3 -I "$here/spice_channel_churn.py" "$work/spice.sock" "$channels" || true
  finish
done

echo '## B4. replay of the sequence that first crashed QEMU (fresh VM each, 5 runs)'
for _ in 1 2 3 4 5; do
  start "${usbredir[@]}" "${port[@]}"
  timeout 20 python3 -I "$here/spice_crash_repro.py" "$work/spice.sock" || true
  finish
done

echo '## B5. minimise: wrong-id playback link, then only the listed channels (fresh VM each)'
for middle in 10 9 9,10; do
  start "${usbredir[@]}" "${port[@]}"
  timeout 20 python3 -I "$here/spice_crash_repro.py" "$work/spice.sock" "$middle" || true
  finish
done
