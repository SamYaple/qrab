#!/usr/bin/env python3
"""Disposable TCG Secure Boot enforcement experiment; never opens host efivarfs.

Requires QEMU, clang/lld-link, openssl, sbsign, and a pinned virt-fw-vars helper.
All firmware VARS, signing keys, EFI applications and logs live under --output.
The source firmware images are read-only. Results include CPU/RSS/elapsed data.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

PROBE = r'''
typedef unsigned char u8; typedef unsigned short u16; typedef unsigned int u32;
typedef unsigned long long u64; typedef u64 status;
struct guid {u32 a;u16 b,c;u8 d[8];};
struct hdr {u64 signature;u32 revision,size,crc,reserved;};
typedef status (*getvar)(u16*,struct guid*,u32*,u64*,void*);
struct runtime {struct hdr h;void *a[6];getvar GetVariable;};
typedef status (*getmap)(u64*,void*,u64*,u64*,u32*);
typedef status (*exitboot)(void*,u64);
struct boot {struct hdr h;void *functions[27];};
struct system {struct hdr h;u16 *vendor;u32 rev;void *console_in_handle,*console_in,*console_out_handle,*console_out,*stderr_handle,*stderr;struct runtime *runtime;struct boot *boot;};
static u64 memory_map[8192];
static void out(char c){__asm__ volatile("outb %0,%1"::"a"((u8)c),"Nd"((u16)0xe9));}
static void text(const char *s){while(*s)out(*s++);}
static void hex(u64 n){for(int i=15;i>=0;i--){u8 v=(n>>(i*4))&15;out(v<10?'0'+v:'a'+v-10);}}
static struct guid global={0x8be4df61,0x93ca,0x11d2,{0xaa,0x0d,0,0xe0,0x98,0x03,0x2b,0x8c}};
static struct guid enable={0xf0a30bc7,0xaf08,0x4556,{0x99,0xc4,0,0x10,0x09,0xc9,0x3a,0x44}};
static void report(struct runtime *rt,u16 *name,const char *label,struct guid *guid){
 u8 value=0xff;u64 size=1;u32 attr=0;status s=rt->GetVariable(name,guid,&attr,&size,&value);
 text(label);text(" status=");hex(s);text(" attrs=");hex(attr);text(" value=");hex(value);text("\n");}
status efi_main(void *image,struct system *st){
 (void)image;text("QRAB_SB_PROBE_START\n");
 report(st->runtime,(u16*)L"SecureBoot","SecureBoot",&global);
 report(st->runtime,(u16*)L"SetupMode","SetupMode",&global);
 report(st->runtime,(u16*)L"AuditMode","AuditMode",&global);
 report(st->runtime,(u16*)L"DeployedMode","DeployedMode",&global);
 report(st->runtime,(u16*)L"SecureBootEnable","SecureBootEnable",&enable);
 u64 map_size=sizeof(memory_map),map_key=0,descriptor_size=0;u32 descriptor_version=0;
 status mapped=((getmap)st->boot->functions[4])(&map_size,memory_map,&map_key,&descriptor_size,&descriptor_version);
 status exited=mapped?mapped:((exitboot)st->boot->functions[26])(image,map_key);
 text("ExitBootServices status=");hex(exited);text("\n");
 if(!exited){
  report(st->runtime,(u16*)L"SecureBoot","RuntimeSecureBoot",&global);
  report(st->runtime,(u16*)L"SecureBootEnable","RuntimeSecureBootEnable",&enable);
 }
 text("QRAB_SB_PROBE_DONE\n");
 __asm__ volatile("outl %0,%1"::"a"((u32)0x10),"Nd"((u16)0xf4));
 for(;;)__asm__ volatile("hlt");return 0;}
'''


def run(argv):
    result = subprocess.run([str(x) for x in argv], check=True, capture_output=True, text=True)
    return result.stdout


def measured(argv):
    # wait4 returns this helper's exact rusage rather than cumulative maxima
    # from earlier clang/openssl children. Files avoid pipe-buffer deadlocks.
    import tempfile
    command = [str(x) for x in argv]
    start = time.monotonic()
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        process = subprocess.Popen(command, stdout=stdout, stderr=stderr)
        _, status, usage = os.wait4(process.pid, 0)
        process.returncode = os.waitstatus_to_exitcode(status)
        stdout.seek(0)
        stderr.seek(0)
        output, errors = stdout.read().decode(), stderr.read().decode()
        if process.returncode:
            raise subprocess.CalledProcessError(process.returncode, command, output, errors)
    return output, {"elapsed_seconds": time.monotonic() - start,
                    "cpu_seconds": usage.ru_utime + usage.ru_stime,
                    "max_rss_kib": usage.ru_maxrss}


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def variables(text):
    result = {}
    for line in text.splitlines():
        fields = line.split()
        if len(fields) == 4 and fields[1].startswith("status="):
            result[fields[0]] = {key: int(value, 16) for key, value in (field.split("=", 1) for field in fields[1:])}
    return result


def boot_case(args, label, vars_source, image):
    case = args.output / label
    (case / "esp/EFI/BOOT").mkdir(parents=True)
    shutil.copyfile(image, case / "esp/EFI/BOOT/BOOTX64.EFI")
    shutil.copyfile(vars_source, case / "vars.fd")
    debug, serial = case / "debug.log", case / "serial.log"
    command = [args.qemu, "-machine", "q35,smm=on", "-accel", "tcg", "-m", "128M",
               "-smp", "1", "-display", "none", "-nodefaults", "-no-reboot",
               "-global", "driver=cfi.pflash01,property=secure,value=on",
               "-drive", f"if=pflash,unit=0,format=raw,readonly=on,file={args.code}",
               "-drive", f"if=pflash,unit=1,format=raw,file={case / 'vars.fd'}",
               "-drive", f"if=none,id=probe,format=raw,readonly=on,file=fat:ro:{case / 'esp'}",
               "-device", "virtio-blk-pci,drive=probe,bootindex=1",
               "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
               "-debugcon", f"file:{debug}", "-global", "isa-debugcon.iobase=0xe9",
               "-serial", f"file:{serial}"]
    start = time.monotonic()
    max_rss = 0
    cpu_ticks = 0
    with (case / "stderr.log").open("wb") as stderr:
        process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=stderr)
        reason = "timeout"
        try:
            while process.poll() is None and time.monotonic() - start < args.timeout:
                try:
                    stat = Path(f"/proc/{process.pid}/stat").read_text().rsplit(") ", 1)[1].split()
                    cpu_ticks = int(stat[11]) + int(stat[12])
                    for line in Path(f"/proc/{process.pid}/status").read_text().splitlines():
                        if line.startswith("VmHWM:"):
                            max_rss = max(max_rss, int(line.split()[1]))
                except FileNotFoundError:
                    pass
                text = debug.read_text(errors="replace") if debug.exists() else ""
                firmware = serial.read_text(errors="replace") if serial.exists() else ""
                if "QRAB_SB_PROBE_DONE" in text:
                    reason = "probe_completed"
                    break
                if "Access Denied" in firmware or "Security Violation" in firmware:
                    reason = "firmware_rejected_loader"
                    break
                time.sleep(0.02)
            if process.poll() is not None and reason == "timeout":
                reason = "process_exited"
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=5)
    text = debug.read_text(errors="replace") if debug.exists() else ""
    return {"case": label, "reason": reason, "probe_executed": "QRAB_SB_PROBE_DONE" in text,
            "returncode": process.returncode, "elapsed_seconds": time.monotonic() - start,
            "cpu_seconds_sampled": cpu_ticks / os.sysconf("SC_CLK_TCK"), "max_rss_kib_sampled": max_rss,
            "debug": text, "variables": variables(text), "serial_tail": serial.read_text(errors="replace")[-3000:] if serial.exists() else "",
            "command": command, "vars_sha256_after": sha(case / "vars.fd")}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--virt-fw-vars", required=True)
    parser.add_argument("--qemu", default="qemu-system-x86_64")
    parser.add_argument("--code", type=Path, default=Path("/usr/share/edk2/OvmfX64/OVMF_CODE.secboot.fd"))
    parser.add_argument("--vars", type=Path, default=Path("/usr/share/edk2/OvmfX64/OVMF_VARS.fd"))
    parser.add_argument("--timeout", type=float, default=10)
    parser.add_argument("--native-disabled", type=Path, help="check an exported native Rust candidate against this existing output run")
    parser.add_argument("--native-enabled", type=Path, help="native re-enabled candidate; must match the original signed-boot VARS bytewise")
    args = parser.parse_args()
    args.output = args.output.resolve()
    if args.native_disabled or args.native_enabled:
        if not (args.native_disabled and args.native_enabled):
            parser.error("both native candidates are required")
        original = args.output / "enabled-signed/vars.fd"
        disabled = args.native_disabled.read_bytes()
        assert len(disabled) == len(original.read_bytes())
        assert sum(a != b for a, b in zip(disabled, original.read_bytes())) == 1
        assert args.native_enabled.read_bytes() == original.read_bytes(), "native roundtrip modified unrelated bytes"
        run([args.virt_fw_vars, "-i", original, "-o", args.output / "native-check-helper-disabled.fd", "--set-false", "SecureBootEnable"])
        inventories = []
        for label, image in [("native", args.native_disabled), ("helper", args.output / "native-check-helper-disabled.fd")]:
            output = args.output / f"native-check-{label}.json"
            run([args.virt_fw_vars, "-i", image, "--output-json", output])
            inventories.append({(v["name"], v["guid"]): v for v in json.loads(output.read_text())["variables"]})
        assert inventories[0] == inventories[1], "native active variables differ from virt-fw-vars"
        cases = [boot_case(args, "native-check-disabled-unsigned", args.native_disabled, args.output / "unsigned.efi"),
                 boot_case(args, "native-check-enabled-signed", args.native_enabled, args.output / "signed.efi"),
                 boot_case(args, "native-check-enabled-unsigned", args.native_enabled, args.output / "unsigned.efi")]
        result = {"active_variable_parity": True, "bytewise_roundtrip": True, "active_variables": len(inventories[0]), "cases": cases}
        (args.output / "native-check-result.json").write_text(json.dumps(result, indent=2))
        print(json.dumps(result, indent=2))
        assert cases[0]["probe_executed"] and cases[1]["probe_executed"]
        assert cases[0]["variables"]["SecureBoot"]["value"] == 0
        assert cases[1]["variables"]["SecureBoot"]["value"] == 1
        assert cases[1]["variables"]["RuntimeSecureBootEnable"]["status"] == 0x800000000000000e
        assert not cases[2]["probe_executed"] and cases[2]["reason"] == "firmware_rejected_loader"
        return
    args.output.mkdir(parents=True, exist_ok=False)
    source = args.output / "probe.c"
    source.write_text(PROBE)
    run(["clang", "-target", "x86_64-pc-windows-msvc", "-ffreestanding", "-fshort-wchar", "-mno-red-zone",
         "-fno-stack-protector", "-c", source, "-o", args.output / "probe.obj"])
    run(["lld-link", "/subsystem:efi_application", "/entry:efi_main", "/nodefaultlib", "/machine:x64",
         f"/out:{args.output / 'unsigned.efi'}", args.output / "probe.obj"])
    run(["openssl", "req", "-new", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1",
         "-subj", "/CN=qrab disposable secure boot probe/", "-keyout", args.output / "key.pem", "-out", args.output / "cert.pem"])
    (args.output / "key.pem").chmod(0o600)
    run(["sbsign", "--key", args.output / "key.pem", "--cert", args.output / "cert.pem",
         "--output", args.output / "signed.efi", args.output / "unsigned.efi"])
    owner = "17b5b3ca-1170-4b35-bf84-91427e1fb028"
    _, enroll_time = measured([args.virt_fw_vars, "-i", args.vars, "-o", args.output / "enabled.fd",
                              "--set-pk", owner, args.output / "cert.pem", "--add-kek", owner, args.output / "cert.pem",
                              "--add-db", owner, args.output / "cert.pem", "--secure-boot"])
    _, disable_time = measured([args.virt_fw_vars, "-i", args.output / "enabled.fd", "-o", args.output / "disabled.fd",
                               "--set-false", "SecureBootEnable"])
    _, reenable_time = measured([args.virt_fw_vars, "-i", args.output / "disabled.fd", "-o", args.output / "reenabled.fd",
                                "--set-true", "SecureBootEnable"])
    for label in ["enabled", "disabled", "reenabled"]:
        run([args.virt_fw_vars, "-i", args.output / f"{label}.fd", "--output-json", args.output / f"{label}.json"])
    cases = [boot_case(args, "disabled-unsigned", args.output / "disabled.fd", args.output / "unsigned.efi"),
             boot_case(args, "enabled-signed", args.output / "enabled.fd", args.output / "signed.efi"),
             boot_case(args, "enabled-unsigned", args.output / "enabled.fd", args.output / "unsigned.efi"),
             boot_case(args, "reenabled-signed", args.output / "reenabled.fd", args.output / "signed.efi")]
    result = {"qemu_version": run([args.qemu, "--version"]).splitlines()[0],
              "code_sha256": sha(args.code), "template_sha256": sha(args.vars),
              "helper": args.virt_fw_vars, "enroll": enroll_time, "disable": disable_time, "reenable": reenable_time,
              "cases": cases}
    (args.output / "result.json").write_text(json.dumps(result, indent=2))
    print(json.dumps(result, indent=2))
    assert cases[0]["probe_executed"], "disabled firmware did not execute unsigned probe"
    assert cases[1]["probe_executed"], "enabled firmware did not execute trusted signed probe"
    assert not cases[2]["probe_executed"] and cases[2]["reason"] == "firmware_rejected_loader", "enabled firmware did not prove unsigned-loader rejection"
    assert cases[3]["probe_executed"], "reenabled firmware did not execute trusted signed probe"
    assert cases[0]["variables"]["SecureBoot"]["value"] == 0
    for case in [cases[1], cases[3]]:
        assert case["variables"]["SecureBoot"]["value"] == 1
        assert case["variables"]["RuntimeSecureBoot"]["value"] == 1
        assert case["variables"]["SecureBootEnable"]["attrs"] == 3
        assert case["variables"]["RuntimeSecureBootEnable"]["status"] == 0x800000000000000e


if __name__ == "__main__":
    main()
