# Getting something booting

## Hypervisors

First things first, let's get something that boots a kernel working so that we have a base to build off of. I'll be doing this in a Virtual Machine (VM) for feedback loop speed so the first thing is to choose a [hypervisor](https://en.wikipedia.org/wiki/Hypervisor) - the thing that'll create me a VM that will boot the kernel and then my OS.

Quick definitions:

 - "Type 1 hypervisor" [^1] - hypervisors you boot directly into instead of booting an OS and then starting a hypervisor
 - "Type 2 hypervisor" [^1] - hypervisors that run on top of an OS
 - "KVM" - Kernel-based Virtual Machine, a set of extensions in the Linux kernel that make VMs _fast_ 🏃. Allows using Linux as a Type 1 hypervisor

We want a type 2 hypervisor.

A few choices:

 - [QEMU](https://www.qemu.org/)
    - I'm pretty familiar with it
    - Nice CLI interface
    - Supports KVM
 - [VirtualBox](https://www.virtualbox.org/)
    - Nice UI, _ok_ CLI (`VBoxManage`), reading the docs it does seem much better than I remember.
    - Less familiar with it, but definitely more popular so might have better support if I run into issues.
    - Oracle 🤮
 - [Gnome Boxes](https://help.gnome.org/users/gnome-boxes/stable/)
    - Non starter, no CLI afaict

## Something to boot

We need to be able to boot something. I'm not starting from _nothing_ so that something will be the linux kernel. Might as well use the one from my laptop (linux 6.68):

```shell
26/12/2023 10:29:35 AEDT❯ uname -r
6.6.8-200.fc39.x86_64
```

/boot has a few things in it:

```shell
26/12/2023 11:03:52 AEDT❯ ls /boot/*6.6.8*      
/boot/config-6.6.8-200.fc39.x86_64  /boot/initramfs-6.6.8-200.fc39.x86_64.img  /boot/symvers-6.6.8-200.fc39.x86_64.xz  /boot/System.map-6.6.8-200.fc39.x86_64  /boot/vmlinuz-6.6.8-200.fc39.x86_64
```

What are those?

### /boot/config-6.6.8-200.fc39.x86_64

```shell
26/12/2023 11:03:58 AEDT❯ file /boot/config-6.6.8-200.fc39.x86_64
/boot/config-6.6.8-200.fc39.x86_64: Linux make config build file, ASCII text
```

Text file containing the Linux Kernel Configuration that my kernel was built with (I assume) [^4]

### /boot/initramfs-6.6.8-200.fc39.x86_64.img

https://wiki.gentoo.org/wiki/Initramfs

initramfs is a file system that gets loaded by the kernel and is responsible for preparing the system before starting an init system.

```shell
~/repos/qos ± ● main
26/12/2023 11:04:49 AEDT❯ file /boot/initramfs-6.6.8-200.fc39.x86_64.img
/boot/initramfs-6.6.8-200.fc39.x86_64.img: regular file, no read permission

~/repos/qos ± ● main
26/12/2023 11:06:20 AEDT❯ sudo file /boot/initramfs-6.6.8-200.fc39.x86_64.img
/boot/initramfs-6.6.8-200.fc39.x86_64.img: ASCII cpio archive (SVR4 with no CRC)
```

Interesting that I don't have read permission

```shell
~/repos/qos ± ● main
26/12/2023 11:06:48 AEDT❯ ls -l /boot/initramfs-6.6.8-200.fc39.x86_64.img
-rw-------. 1 root root 40288504 Dec 25 17:03 /boot/initramfs-6.6.8-200.fc39.x86_64.img
```

Owned and readble only by root.

https://wiki.gentoo.org/wiki/Initramfs

"CPIO" [^6] is interesting - an archive format I haven't looked at much.

Let's try and dump it.

```shell
26/12/2023 11:09:05 AEDT❯ mkdir cpio
mkdir: created directory 'cpio'

/tmp 
26/12/2023 11:09:09 AEDT❯ cd cpio

/tmp/cpio 
26/12/2023 11:09:24 AEDT❯ sudo cp /boot/initramfs-6.6.8-200.fc39.x86_64.img .

/tmp/cpio 
26/12/2023 11:09:26 AEDT❯ sudo chown colin:colin ./initramfs-6.6.8-200.fc39.x86_64.img 

26/12/2023 11:10:12 AEDT❯ cpio --help                          
Usage: cpio [OPTION...] [destination-directory]
GNU `cpio' copies files to and from archives
```

wtf is `-i` extract lol

```shell
/tmp/cpio 
26/12/2023 11:10:17 AEDT❯ cpio -i < initramfs-6.6.8-200.fc39.x86_64.img 
416 blocks

/tmp/cpio 
26/12/2023 11:12:15 AEDT❯ ls
early_cpio  initramfs-6.6.8-200.fc39.x86_64.img  kernel
```

early_cpio?

```shell
26/12/2023 11:12:17 AEDT❯ file early_cpio 
early_cpio: ASCII text

/tmp/cpio 
26/12/2023 11:12:39 AEDT❯ cat early_cpio 
1
```

Not immediately obvious what this does

https://github.com/systemd/systemd/blob/5fd55b2c265a4df90c31081976c5bedde94baf4a/man/ukify.xml#L566

UKI? https://wiki.archlinux.org/title/Unified_kernel_image

`mkinitcpo` https://wiki.archlinux.org/title/mkinitcpio - might be useful later?

https://github.com/gentoo/gentoo/blob/461411849f77e2b4ed09bcfd1b5f9a26ef488148/sys-kernel/linux-firmware/linux-firmware-20231211.ebuild#L116 - seems to be [Microcode](https://en.wikipedia.org/wiki/Microcode) related?

https://github.com/torvalds/linux/blob/master/lib/earlycpio.c

https://www.kernel.org/doc/Documentation/x86/microcode.txt

https://github.com/torvalds/linux/blob/fbafc3e621c3f4ded43720fdb1d6ce1728ec664e/arch/x86/kernel/cpu/microcode/core.c#L218 - calls the earlycpio.c func to find a CPIO archive

https://github.com/dracutdevs/dracut/blob/4971f443726360216a4ef3ba8baea258a1cd0f3b/dracut.sh#L2315 - where it gets created

So this indicates that this CPIO archive contains a microcode that should be loaded - doesn't seem like this is actually _read_ anywhere, so it's just informational.

```shell
26/12/2023 11:31:02 AEDT❯ ls kernel/x86/microcode 
GenuineIntel.bin
```

This is the microcode to load. Makes sense

Weirdly, this is the end of the archive? Where's the rest of my initramfs?

```shell
26/12/2023 11:49:05 AEDT❯ binwalk ./initramfs-6.6.8-200.fc39.x86_64.img

DECIMAL       HEXADECIMAL     DESCRIPTION
--------------------------------------------------------------------------------
0             0x0             ASCII cpio archive (SVR4 with no CRC), file name: ".", file name length: "0x00000002", file size: "0x00000000"
112           0x70            ASCII cpio archive (SVR4 with no CRC), file name: "early_cpio", file name length: "0x0000000B", file size: "0x00000002"
240           0xF0            ASCII cpio archive (SVR4 with no CRC), file name: "kernel", file name length: "0x00000007", file size: "0x00000000"
360           0x168           ASCII cpio archive (SVR4 with no CRC), file name: "kernel/x86", file name length: "0x0000000B", file size: "0x00000000"
484           0x1E4           ASCII cpio archive (SVR4 with no CRC), file name: "kernel/x86/microcode", file name length: "0x00000015", file size: "0x00000000"
616           0x268           ASCII cpio archive (SVR4 with no CRC), file name: "kernel/x86/microcode/GenuineIntel.bin", file name length: "0x00000026", file size: "0x00033C00"
212732        0x33EFC         ASCII cpio archive (SVR4 with no CRC), file name: "TRAILER!!!", file name length: "0x0000000B", file size: "0x00000000"
212992        0x34000         gzip compressed data, maximum compression, from Unix, last modified: 1970-01-01 00:00:00 (null date)
8109851       0x7BBF1B        gzip compressed data, from Unix, last modified: 1970-01-01 00:00:00 (null date)
11000563      0xA7DAF3        xz compressed data
13377833      0xCC2129        xz compressed data
13884598      0xD3DCB6        xz compressed data
13907055      0xD4346F        xz compressed data
13912976      0xD44B90        xz compressed data
13966468      0xD51C84        xz compressed data
24097481      0x16FB2C9       Certificate in DER format (x509 v3), header length: 4, sequence length: 17280
25197943      0x1807D77       CRC32 polynomial table, little endian
```

_Interesting_, so this is a bunch of files concatenated together. Our CPIO archive for microcode, then a bunch of gzipped data, a certificate (for secureboot presumably), and a CRC32 table? 

Extracting it fully:

```shell
26/12/2023 12:11:43 AEDT❯ (cpio -i; gunzip | cpio -i) < ../initramfs-6.6.8-200.fc39.x86_64.img
416 blocks
cpio: dev/console: Cannot mknod: Operation not permitted
cpio: dev/kmsg: Cannot mknod: Operation not permitted
cpio: dev/null: Cannot mknod: Operation not permitted
cpio: dev/random: Cannot mknod: Operation not permitted
cpio: dev/urandom: Cannot mknod: Operation not permitted
160357 blocks

26/12/2023 12:11:53 AEDT❯ ls
bin  dev  early_cpio  etc  init  kernel  lib  lib64  proc  root  run  sbin  shutdown  sys  sysroot  tmp  usr  var
```

_Nice_. That gives us our initramfs layout.

```shell
/tmp/cpio/start 
26/12/2023 12:15:55 AEDT❯ find . | less
... lots of stuff
```

Notable things:

 - ./usr/share/plymouth (https://wiki.archlinux.org/title/plymouth) - the thing that gives fedora a nice loader while the system is booting
 - ./usr/lib/systemd/ - interesting, the initramfs also runs systemd
 - ./usr/lib/modules/ - lots of kernel modules
 - ./usr/lib/kbd/ - keyboard configurations
 - ./etc/udev/rules.d/ - https://opensource.com/article/18/11/udev

When linux loads the initramfs, it runs `/init` (https://github.com/torvalds/linux/blob/master/init/main.c#L159), or the kernel command line argument `rdinit` (ram disk init, https://github.com/torvalds/linux/blob/master/init/main.c#L603) which in our case is `systemd`:

```shell
26/12/2023 12:22:25 AEDT❯ ls -l init
lrwxrwxrwx. 1 colin colin 23 Dec 26 12:11 init -> usr/lib/systemd/systemd
```

https://github.com/torvalds/linux/blob/v4.15/init/main.c#L1031C1-L1038C70 - the logic seems to be, first try `rdinit=`, then `init=`, then `/sbin/init`, `/etc/init`, `/bin/init`, `/bin/sh`, and finally just die.

https://unix.stackexchange.com/questions/30414/what-can-make-passing-init-path-to-program-to-the-kernel-not-start-program-as-i seems to say the opposite, that it only tries /init _without_ an `init` but that's wrong - it tries /init first on the assumption that we _might_ be in an initramfs.

### /boot/symvers-6.6.8-200.fc39.x86_64.xz

```shell
26/12/2023 13:20:11 AEDT❯ unxz ./symvers-6.6.8-200.fc39.x86_64.xz

26/12/2023 13:21:18 AEDT❯ file symvers-6.6.8-200.fc39.x86_64
symvers-6.6.8-200.fc39.x86_64: ASCII text
```

```shell
26/12/2023 13:22:37 AEDT❯ head symvers-6.6.8-200.fc39.x86_64 
0x00000000      system_state    vmlinux EXPORT_SYMBOL
0x00000000      static_key_initialized  vmlinux EXPORT_SYMBOL_GPL
0x00000000      reset_devices   vmlinux EXPORT_SYMBOL
0x00000000      loops_per_jiffy vmlinux EXPORT_SYMBOL
0x00000000      init_uts_ns     vmlinux EXPORT_SYMBOL_GPL
0x00000000      wait_for_initramfs      vmlinux EXPORT_SYMBOL_GPL
0x00000000      init_task       vmlinux EXPORT_SYMBOL
0x00000000      cc_platform_has vmlinux EXPORT_SYMBOL_GPL
0x00000000      cc_mkdec        vmlinux EXPORT_SYMBOL_GPL
0x00000000      tdx_kvm_hypercall       vmlinux EXPORT_SYMBOL_GPL
```

This is the symbols exported from the kernel / compiled modules during the kernel build.

 - `EXPORT_SYMBOL` - available to any module
 - `EXPORT_SYMBOL_GPL` - only available to modules that have a GPL compatible license

Interesting - I thought the first column would be memory addresses, but apparently they're _CRCs_? All mine are 0x0000000, but this seems expected: https://docs.kernel.org/kbuild/modules.html "For a kernel build without CONFIG_MODVERSIONS enabled, the CRC would read 0x00000000."

As a quick sanity check, https://github.com/torvalds/linux/blob/fbafc3e621c3f4ded43720fdb1d6ce1728ec664e/arch/s390/lib/mem.S#L51

```shell
26/12/2023 13:27:27 AEDT❯ grep __memmove symvers-6.6.8-200.fc39.x86_64 
13181:0x00000000        __memmove       vmlinux EXPORT_SYMBOL
```

neat.

### /boot/System.map-6.6.8-200.fc39.x86_64

https://en.wikipedia.org/wiki/System.map

```shell
26/12/2023 13:33:02 AEDT❯ cp /boot/System.map-6.6.8-200.fc39.x86_64 .
cp: cannot open '/boot/System.map-6.6.8-200.fc39.x86_64' for reading: Permission denied

/tmp/cpio 
26/12/2023 13:33:07 AEDT❯ sudo cp /boot/System.map-6.6.8-200.fc39.x86_64 .

/tmp/cpio 
26/12/2023 13:33:10 AEDT❯ sudo ls -la ./System.map-6.6.8-200.fc39.x86_64
-rw-------. 1 root root 8784472 Dec 26 13:33 ./System.map-6.6.8-200.fc39.x86_64

/tmp/cpio 
26/12/2023 13:33:38 AEDT❯ sudo chown colin:colin ./System.map-6.6.8-200.fc39.x86_64
```

_This_ is where the kernel has the addresses of its symbols:

```shell
26/12/2023 13:34:04 AEDT❯ file ./System.map-6.6.8-200.fc39.x86_64
./System.map-6.6.8-200.fc39.x86_64: ASCII text

/tmp/cpio 
26/12/2023 13:34:46 AEDT❯ head ./System.map-6.6.8-200.fc39.x86_64 
0000000000000000 D __per_cpu_start
0000000000000000 D fixed_percpu_data
0000000000001000 D cpu_debug_store
0000000000002000 D irq_stack_backing_store
0000000000006000 D cpu_tss_rw
000000000000b000 D gdt_page
000000000000c000 d exception_stacks
0000000000018000 d entry_stack_storage
0000000000019000 D espfix_waddr
0000000000019008 D espfix_stack
```

What's the second column there? `D`. "Symbol Types" apparently:

```
    A for absolute
    B or b for uninitialized data section (called BSS)
    D or d for initialized data section
    G or g for initialized data section for small objects (global)
    i for sections specific to DLLs
    N for debugging symbol
    p for stack unwind section
    R or r for read only data section
    S or s for uninitialized data section for small objects
    T or t for text (code) section
    U for undefined
    V or v for weak object
    W or w for weak objects which have not been tagged so
    - for stabs symbol in an a.out object file
    ? for "symbol type unknown"
```

### /boot/vmlinuz-6.6.8-200.fc39.x86_64

Finally what we actually wanted - this is the linux kernel to boot.

```shell
26/12/2023 13:53:04 AEDT❯ file ./vmlinuz-6.6.8-200.fc39.x86_64 
./vmlinuz-6.6.8-200.fc39.x86_64: Linux kernel x86 boot executable bzImage, version 6.6.8-200.fc39.x86_64 (mockbuild@f2936e05dca94a129acf79933fec484d) #1 SMP PREEMPT_DYNAMIC Thu Dec 21 04:01:49 UTC 2023, RO-rootFS, swap_dev 0XD, Normal VGA
```

## Actually booting something

Just for familiarities sake more than anything, I'm gonna use qemu:

```shell
qemu-system-x86_64 -kernel ./vmlinuz-6.6.8-200.fc39.x86_64 -display none -serial stdio -append "console=ttyAMA0 console=ttyS0" --enable-kvm
```

Breaking that down a bit:

 - `-kernel ./vmlinuz-6.6.8-200.fc39.x86_64` - boot the kernel we have
 - `-display none -serial stdio -append "console=ttyAMA0 console=ttyS0"` - redirect the output to our terminal instead of qemus window (https://stackoverflow.com/questions/18098455/redirect-qemu-console-to-a-file-or-the-host-terminal/18100781#18100781)
 - `--enable-kvm`, optional, but makes things a lot faster

 Running that, we get a kernel panic:

```log
[    3.381524] Kernel panic - not syncing: VFS: Unable to mount root fs on unknown-block(0,0)
[    3.385943] CPU: 0 PID: 1 Comm: swapper/0 Not tainted 6.6.8-200.fc39.x86_64 #1
[    3.386341] Hardware name: QEMU Standard PC (i440FX + PIIX, 1996), BIOS 1.16.3-1.fc39 04/01/2014
[    3.386887] Call Trace:
[    3.387091]  <TASK>
[    3.387590]  dump_stack_lvl+0x47/0x60
[    3.388081]  panic+0x193/0x350
[    3.388269]  mount_root_generic+0x1ac/0x340
[    3.388513]  prepare_namespace+0x69/0x280
[    3.388696]  kernel_init_freeable+0x41c/0x470
[    3.388934]  ? __pfx_kernel_init+0x10/0x10
[    3.389115]  kernel_init+0x1a/0x1c0
[    3.389269]  ret_from_fork+0x34/0x50
[    3.389430]  ? __pfx_kernel_init+0x10/0x10
[    3.389607]  ret_from_fork_asm+0x1b/0x30
[    3.389866]  </TASK>
[    3.390998] Kernel Offset: 0x7000000 from 0xffffffff81000000 (relocation range: 0xffffffff80000000-0xffffffffbfffffff)
[    3.392013] ---[ end Kernel panic - not syncing: VFS: Unable to mount root fs on unknown-block(0,0) ]---
```

Nice. We have a kernel that can panic. That means we booted something!

## Footnotes

 - [^1] Types of hypervisors: https://www.vmware.com/au/topics/glossary/content/hypervisor.html
 - [^2] https://www.debugpoint.com/install-use-gnome-boxes/
 - [^3] https://www.oracle.com/technical-resources/articles/it-infrastructure/admin-manage-vbox-cli.html
 - [^4] Linux Kernel Configuration: https://tldp.org/HOWTO/SCSI-2.4-HOWTO/kconfig.html
 - [^5] CPIO: https://en.wikipedia.org/wiki/Cpio