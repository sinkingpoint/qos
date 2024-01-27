# Getting something mounting

https://docs.rs/nix/latest/nix/mount/fn.mount.html

```
pub fn mount<P1: ?Sized + NixPath, P2: ?Sized + NixPath, P3: ?Sized + NixPath, P4: ?Sized + NixPath>(
    source: Option<&P1>,
    target: &P2,
    fstype: Option<&P3>,
    flags: MsFlags,
    data: Option<&P4>
) -> Result<()>

    source - Specifies the file system. e.g. /dev/sd0.
    target - Specifies the destination. e.g. /mnt.
    fstype - The file system type, e.g. ext4.
    flags - Optional flags controlling the mount.
    data - Optional file system specific data.
```

How to get the fstype?

https://github.com/torvalds/linux/blob/master/include/uapi/linux/magic.h

ext2/3/4 super blocks are the same?

---

blkid?

General idea, try read the superblock and validate it.

https://ext4.wiki.kernel.org/index.php/Ext4_Disk_Layout#The_Super_Block

`cat /proc/filesystems` - valid args to mount

```
truncate -s 1000M ./filesystem
mkfs.ext4 -L "test" filesystem
```


```
Found filesystem: ext4 (test)
UUID: ee3b1d02-bfb7-4cf0-b7c5-f912f354c83b
Mounting at: /tmp/fs
mount: Error: ENOTBLK: Block device required
```

womp

Filesystem files aren't block devices. Need a "loopback" device.

losetup

```
[root@fedora qos]# losetup --find --show ./filesystem 
/dev/loop12

[root@fedora qos]# ~colin/.cargo/bin/cargo run -p mount /dev/loop12 /tmp/fs
   Compiling mount v0.1.0 (/home/colin/repos/qos/mount)
    Finished dev [unoptimized + debuginfo] target(s) in 0.59s
     Running `target/debug/mount /dev/loop12 /tmp/fs`
Found filesystem: ext4 (test)
UUID: ee3b1d02-bfb7-4cf0-b7c5-f912f354c83b
Mounting at: /tmp/fs
[root@fedora qos]# ls /tmp/fs
lost+found
```