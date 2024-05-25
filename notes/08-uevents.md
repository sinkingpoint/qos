# uevents

We want to be able to load kernel modules, and the first step of that is checking which devices we have. How can we do that?

https://www.kernel.org/doc/Documentation/ABI/testing/sysfs-uevent
http://web.archive.org/web/20160127215232/https://www.kernel.org/doc/pending/hotplug.txt


> In theory, transient devices (which are created and removed again almost
instantly, which can be caused by poorly written drivers that fail their device
probe) could have similar "leftover" /dev entries from the /sbin/hotplug
mechanism.  (If two processes are spawned simultaneously, which one completes
first is not guaranteed.)  This is not common, but theoretically possible.

> These sort of races are why the netlink mechanism was created.  To avoid
such potential races when using netlink, instead of reading each "dev" entry,
fake "add" events by writing to each device's "uevent" file in sysfs.  This
filters the sequencing through the kernel, which will not deliver an "add"
event packet to the netlink process for a device that has been removed.

Generae idea:

- Iterate /sys, looking for `uevent` files
- For every `uevent` file, echo "add" into it
