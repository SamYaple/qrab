#!/bin/bash
# SEV-SNP readiness diagnostic
# Run as root for full results

echo "=== 1. CPU SEV flags ==="
grep -o "sev[^ ]*\|sme" /proc/cpuinfo | sort -u

echo ""
echo "=== 2. Kernel cmdline ==="
cat /proc/cmdline

echo ""
echo "=== 3. Memory encryption active? ==="
if [ -f /sys/kernel/mm/mem_encrypt/active ]; then
    echo "mem_encrypt/active = $(cat /sys/kernel/mm/mem_encrypt/active)"
else
    echo "NOT ACTIVE - /sys/kernel/mm/mem_encrypt/active does not exist"
    echo ""
    echo "Possible causes:"
    echo "  1. BIOS: SMEE/SME not enabled (check Advanced > CPU > SME/SMEE)"
    echo "  2. BIOS: SEV not enabled (check Advanced > CPU > SEV)"  
    echo "  3. BIOS: SNP not enabled (check Advanced > CPU > SNP)"
    echo "  4. BIOS: TSME not enabled (alternative to SMEE)"
    echo "  5. BIOS: IOMMU not enabled"
    echo "  6. Kernel cmdline missing mem_encrypt=on"
fi

echo ""
echo "=== 4. KVM SEV parameters ==="
for p in sev sev_es sev_snp; do
    f="/sys/module/kvm_amd/parameters/$p"
    [ -f "$f" ] && echo "  $p = $(cat $f)" || echo "  $p = (not found)"
done

echo ""
echo "=== 5. KVM VM types capability ==="
python3 -c "
import fcntl, os
fd = os.open('/dev/kvm', os.O_RDONLY)
val = fcntl.ioctl(fd, 0xAE03, 209)
os.close(fd)
print(f'  KVM_CAP_VM_TYPES = {val:#x}')
print(f'    Default VM: {bool(val & 1)}')
print(f'    SEV VM:     {bool(val & 4)}')
print(f'    SEV-ES VM:  {bool(val & 8)}')
print(f'    SNP VM:     {bool(val & 16)}')
if val == 1:
    print('')
    print('  PROBLEM: Only default VM type available.')
    print('  SEV/SNP not registered in KVM. mem_encrypt must be active.')
" 2>/dev/null || echo "  (cannot check - /dev/kvm not accessible)"

echo ""
echo "=== 6. /dev/sev status ==="
ls -la /dev/sev 2>/dev/null || echo "  /dev/sev does not exist"

echo ""
echo "=== 7. CCP/PSP driver ==="
lsmod 2>/dev/null | grep -E "ccp|psp" || echo "  Not a module (may be built-in)"
for p in CONFIG_CRYPTO_DEV_CCP CONFIG_CRYPTO_DEV_SP_PSP CONFIG_KVM_AMD_SEV; do
    val=$(zcat /proc/config.gz 2>/dev/null | grep "^$p=" | cut -d= -f2)
    echo "  $p = ${val:-not found}"
done

echo ""
echo "=== 8. IOMMU ==="
iommu_count=$(find /sys/class/iommu -type l 2>/dev/null | wc -l)
echo "  IOMMU groups: $iommu_count"
if [ "$iommu_count" -eq 0 ]; then
    echo "  PROBLEM: No IOMMU groups. Add amd_iommu=on iommu=pt to cmdline"
fi

echo ""
echo "=== 9. dmesg (if accessible) ==="
dmesg 2>/dev/null | grep -iE "sev|snp|mem_encrypt|SME|ccp|psp|SMEE" | head -20 || echo "  (no dmesg access - run as root)"

echo ""
echo "=== 10. BIOS SME check via MSR ==="
python3 -c "
import struct, os
try:
    fd = os.open('/dev/cpu/0/msr', os.O_RDONLY)
    os.lseek(fd, 0xC0010010, os.SEEK_SET)
    data = os.read(fd, 8)
    val = struct.unpack('Q', data)[0]
    sme_en = bool(val & (1 << 23))
    print(f'  SYSCFG MSR = {val:#018x}')
    print(f'  MemEncryptionModeEn (bit 23) = {sme_en}')
    if not sme_en:
        print('')
        print('  PROBLEM: SME not enabled in hardware.')
        print('  Enable SMEE/SME in BIOS settings.')
    os.close(fd)
except PermissionError:
    print('  (need root to read MSR)')
except FileNotFoundError:
    print('  (need modprobe msr)')
" 2>&1

echo ""
echo "=== Summary ==="
active="false"
[ -f /sys/kernel/mm/mem_encrypt/active ] && active="true"
vm_types=$(python3 -c "import fcntl,os;fd=os.open('/dev/kvm',os.O_RDONLY);print(fcntl.ioctl(fd,0xAE03,209));os.close(fd)" 2>/dev/null || echo "0")
if [ "$active" = "true" ] && [ "$vm_types" != "1" ]; then
    echo "SEV-SNP: READY"
elif [ "$active" = "true" ]; then
    echo "SEV-SNP: mem_encrypt active but KVM not registering SEV types"
else
    echo "SEV-SNP: NOT READY - mem_encrypt not active"
    echo ""
    echo "Most likely fix: Enable SMEE/SME in BIOS, then reboot."
    echo "BIOS path is typically: Advanced > CPU Configuration > SME or SMEE > Enabled"
fi
