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

