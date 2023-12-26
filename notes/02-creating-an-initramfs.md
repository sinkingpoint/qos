# Creating an initramfs

When linux boots, it runs an _init process_ as process id (pid) 1. The init process is responsible for actually doing things. If you need to do things _before_ the init system gets run though, you need an `initramfs` - a file system that gets loaded and run that _then_ loads the real rootfs. While they're not strictly required, let's see what it takes to make one.

## Structure

We [previously learnt](01-getting-something-booting.md) that our initramfs is a couple of cpio archives concatenated together - the first contains the structure:

```
early_cpio
kernel/
  x86/
    microcode/
      GenuineIntel.bin
```

And the second contains our actual file system:

```shell
26/12/2023 14:45:43 AEDT❯ ls
bin  dev  etc  init  lib  lib64  proc  root  run  sbin  shutdown  sys  sysroot  tmp  usr  var
```

With /init as the binary that gets run when our initramfs is loaded.

I have an inkling that we don't actually need the microcode part, so my goal here is to:

- Build a CPIO archive
- Chuck /bin/sh into /init for now
- Update our qemu to use the initramfs

Seems simple enough.

Let's write some code to start assembling a file system. Really all we want is a CPIO archive with its /init as /bin/sh, and to kick things off write, let's start in Rust.

[assemble-initramfs](../assemble-initramfs/) will do that

This is new:
```shell
26/12/2023 14:59:38 AEDT❯ cargo run ./assemble-initramfs
warning: virtual workspace defaulting to `resolver = "1"` despite one or more workspace members being on edition 2021 which implies `resolver = "2"`
note: to keep the current resolver, specify `workspace.resolver = "1"` in the workspace root's manifest
note: to use the edition 2021 resolver, specify `workspace.resolver = "2"` in the workspace root's manifest
note: for more details see https://doc.rust-lang.org/cargo/reference/resolver.html#resolver-versions
```

fixes with `resolver = "2"` in the Cargo.toml

Using https://docs.rs/clap/latest/clap/ for command line parsing, as that's what I've used in the past and it seems like it's still maintained.

https://crates.io/crates/slog seems to still be the thing for structured logging, but that recommends `tracing` - we don't need tracing at this point though.

## cpio

https://docs.rs/cpio/latest/cpio/ exists, but seems unmaintained. Let's give it a shot I guess.
https://crates.io/crates/cpio-archive doesn't support writing

### https://docs.rs/cpio/latest/cpio/

Seems to be the only one that supports writing, but there's no examples...

First attempt:

```rust
    let inputs = vec!["init"].iter().map(|&s| {
        let full_path = base_dir.join(s);

        debug!(log, "Adding file to CPIO archive"; "path" => full_path.display().to_string());

        (cpio::NewcBuilder::new(s), File::open(full_path).expect("Failed to open file"))
    }).collect::<Vec<_>>();

    let output_file = File::create(&args.output_file).expect("Failed to open output file");
    cpio::write_cpio(inputs.into_iter(), output_file).expect("Failed to write CPIO archive");
```

Pretends to work, but I get:

```
26/12/2023 16:26:16 AEDT❯ cpio -i < initramfs 
cpio: init: unknown file type
2814 blocks
```

I think I might need to set the values on the builder?

Indeed:

```rust
let builder = NewcBuilder::new(s)
    .uid(1000)
    .gid(1000)
    .mode(0o100644);
```

Will probably need a way to configure that as well.

## Actually booting into our initramfs

Now that we can make an initramfs, let's run it:

```shell
~/repos/qos ± ● main
26/12/2023 16:47:46 AEDT❯ cargo run -p assemble-initramfs
   Compiling assemble-initramfs v0.1.0 (/home/colin/repos/qos/assemble-initramfs)
    Finished dev [unoptimized + debuginfo] target(s) in 0.89s
     Running `target/debug/assemble-initramfs`
{"msg":"Assembling initramfs structure in /tmp/assemble-initramfs3574388197","level":"INFO","ts":"2023-12-26T05:47:49.446663456Z"}
{"msg":"Adding file to CPIO archive","level":"DEBG","ts":"2023-12-26T05:47:49.447504205Z","path":"/tmp/assemble-initramfs3574388197/init"}

~/repos/qos ± ●● main
26/12/2023 16:47:49 AEDT❯ qemu-system-x86_64 -kernel /tmp/cpio/vmlinuz-6.6.8-200.fc39.x86_64 -initrd ./initramfs -display none -serial stdio -append "console=ttyAMA0 console=ttyS0" --enable-kvm
```

Aaand, we get:

```log
[    1.129647] Failed to execute /init (error -13)
[    1.130384] Run /sbin/init as init process
[    1.131080] Run /etc/init as init process
[    1.131733] Run /bin/init as init process
[    1.132405] Run /bin/sh as init process
[    1.133059] Kernel panic - not syncing: No working init found.  Try passing init= option to kernel. See Linux Documentation/admin-guide/init.rst for guidance.
```

But we're further! We found a /init . -13 is EACCES (https://unix.stackexchange.com/questions/326766/what-are-the-standard-error-codes-in-linux). Derp - we didn't set the executable bits on our init binary. Let's do this:

```rust
        let builder = NewcBuilder::new(s)
            .uid(0)
            .gid(0)
            .mode(0o100744);
```

And we get a new error:

```log
[    1.202118] Failed to execute /init (error -2)
```

ENOENT this time - ah ha! shared libraries. Let's see what we need to bring in for /bin/sh:

```
26/12/2023 16:51:44 AEDT❯ ldd /bin/sh
        linux-vdso.so.1 (0x00007fff937eb000)
        libtinfo.so.6 => /lib64/libtinfo.so.6 (0x00007f857fb62000)
        libc.so.6 => /lib64/libc.so.6 (0x00007f857f980000)
        /lib64/ld-linux-x86-64.so.2 (0x00007f857fd1c000)
```

/lib64/libtinfo.so.6, /lib64/libc.so.6, and ld-linux-x86-64.so.2. _fine_. Let's hack that into our assembler.

```
26/12/2023 16:55:30 AEDT❯ cpio -i < ../initramfs
cpio: lib64/libtinfo.so.6: Cannot open: No such file or directory
cpio: lib64/libc.so.6: Cannot open: No such file or directory         
```

/tableflip . I think that means we have to add the lib64 _directory_ as well? I guess let's add support for that.

Aaand https://github.com/jcreekmore/cpio-rs/blob/master/src/lib.rs#L16 doesn't support adding directories - let's copy it and add support.

```
[    1.325888] Run /init as init process
init: cannot set terminal process group (-1): Inappropriate ioctl for device
init: no job control in this shell
init-5.2# 
```

Nice. We boot into /bin/sh and have a prompt. We have nothing else though:

```
init-5.2# ls
init: ls: command not found
init-5.2# clear
init: clear: command not found
```

And if we try to exit, we get a kernel panic lol:

```
init-5.2# exit
[    1.959097] Kernel panic - not syncing: Attempted to kill init! exitcode=0x00000000
```

But it's there!
