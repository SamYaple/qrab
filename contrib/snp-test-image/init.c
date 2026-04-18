/* Minimal init for SEV-SNP boot testing.
 * No glibc — raw syscalls only. No AVX-512 ifunc problem.
 */
#include <linux/reboot.h>

#define __NR_write 1
#define __NR_open 2
#define __NR_close 3
#define __NR_mount 165
#define __NR_reboot 169
#define __NR_sync 162
#define __NR_uname 63
#define __NR_read 0
#define __NR_lseek 8
#define __NR_nanosleep 35
#define __NR_finit_module 313

#define O_RDONLY 0
#define O_WRONLY 1
#define O_NONBLOCK 04000

struct timespec {
    long tv_sec;
    long tv_nsec;
};

struct utsname {
    char sysname[65];
    char nodename[65];
    char release[65];
    char version[65];
    char machine[65];
    char domainname[65];
};

static long syscall1(long nr, long a1) {
    long ret;
    __asm__ volatile("syscall" : "=a"(ret) : "a"(nr), "D"(a1) : "rcx", "r11", "memory");
    return ret;
}

static long syscall2(long nr, long a1, long a2) {
    long ret;
    __asm__ volatile("syscall" : "=a"(ret) : "a"(nr), "D"(a1), "S"(a2) : "rcx", "r11", "memory");
    return ret;
}

static long syscall3(long nr, long a1, long a2, long a3) {
    long ret;
    register long r10 __asm__("r10") = 0;
    __asm__ volatile("syscall" : "=a"(ret) : "a"(nr), "D"(a1), "S"(a2), "d"(a3), "r"(r10) : "rcx", "r11", "memory");
    return ret;
}

static long syscall5(long nr, long a1, long a2, long a3, long a4, long a5) {
    long ret;
    register long r10 __asm__("r10") = a4;
    register long r8 __asm__("r8") = a5;
    __asm__ volatile("syscall" : "=a"(ret) : "a"(nr), "D"(a1), "S"(a2), "d"(a3), "r"(r10), "r"(r8) : "rcx", "r11", "memory");
    return ret;
}

static int my_strlen(const char *s) {
    int n = 0;
    while (s[n]) n++;
    return n;
}

static void my_write(int fd, const char *s) {
    syscall3(__NR_write, fd, (long)s, my_strlen(s));
}

static int my_open(const char *path, int flags) {
    return (int)syscall2(__NR_open, (long)path, flags);
}

static void my_close(int fd) {
    syscall1(__NR_close, fd);
}

static int my_strstr(const char *hay, const char *needle) {
    int nlen = my_strlen(needle);
    int hlen = my_strlen(hay);
    for (int i = 0; i <= hlen - nlen; i++) {
        int j;
        for (j = 0; j < nlen; j++) {
            if (hay[i+j] != needle[j]) break;
        }
        if (j == nlen) return 1;
    }
    return 0;
}

/* Load a kernel module via finit_module(fd, "", 0). */
static void load_module(const char *path) {
    int fd = my_open(path, O_RDONLY);
    if (fd >= 0) {
        syscall3(__NR_finit_module, fd, (long)"", 0);
        my_close(fd);
    }
}

void _start(void) {
    /* Mount virtual filesystems */
    syscall5(__NR_mount, (long)"proc", (long)"/proc", (long)"proc", 0, 0);
    syscall5(__NR_mount, (long)"sysfs", (long)"/sys", (long)"sysfs", 0, 0);
    syscall5(__NR_mount, (long)"devtmpfs", (long)"/dev", (long)"devtmpfs", 0, 0);

    /* Load VGA/DRM modules for framebuffer console (VNC display) */
    load_module("/modules/drm_kms_helper.ko");
    load_module("/modules/drm_shmem_helper.ko");
    load_module("/modules/drm_client_lib.ko");
    load_module("/modules/bochs.ko");

    /* Open consoles — serial for tests, tty0 for VNC framebuffer */
    int serial = my_open("/dev/ttyS0", O_WRONLY);
    int vga = my_open("/dev/tty0", O_WRONLY);

    my_write(1, "===== SNP BOOT OK =====\n");
    if (serial >= 0) my_write(serial, "===== SNP BOOT OK =====\n");
    if (vga >= 0) my_write(vga, "===== SNP BOOT OK =====\n");

    /* Write to all consoles: stdout, serial, VGA */
    #define emit(msg) do { \
        my_write(1, msg); \
        if (serial >= 0) my_write(serial, msg); \
        if (vga >= 0) my_write(vga, msg); \
    } while(0)

    /* Get kernel version */
    struct utsname uts;
    if (syscall1(__NR_uname, (long)&uts) == 0) {
        emit("Kernel: ");
        emit(uts.release);
        emit("\n");
    }

    /* Read kmsg to check SEV status */
    int kmsg = my_open("/dev/kmsg", O_RDONLY | O_NONBLOCK);
    if (kmsg >= 0) {
        char buf[512];
        int found_snp = 0, found_sev = 0;
        long n;
        while ((n = syscall3(__NR_read, kmsg, (long)buf, 511)) > 0) {
            buf[n] = 0;
            if (my_strstr(buf, "SEV-SNP")) found_snp = 1;
            else if (my_strstr(buf, "SEV")) found_sev = 1;
        }
        my_close(kmsg);
        if (found_snp) {
            emit("SEV-SNP: active\n");
        } else if (found_sev) {
            emit("SEV: active (non-SNP)\n");
        }
    }

    emit("===== BOOT COMPLETE =====\n");

    /* Sleep 5 minutes so VM stays alive for QMP queries / health checks */
    emit("Sleeping 300s before poweroff...\n");
    static struct timespec ts = { .tv_sec = 300, .tv_nsec = 0 };
    syscall2(__NR_nanosleep, (long)&ts, 0);

    emit("Sleep done, powering off.\n");
    if (serial >= 0) my_close(serial);
    if (vga >= 0) my_close(vga);

    /* Sync and power off */
    syscall1(__NR_sync, 0);
    syscall3(__NR_reboot, LINUX_REBOOT_MAGIC1, LINUX_REBOOT_MAGIC2, LINUX_REBOOT_CMD_POWER_OFF);

    /* Should not reach here */
    for(;;) __asm__("hlt");
}
