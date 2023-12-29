# assemble-initramfs

assemble-initramfs constructs a CPIO archive file that can be passed into qemu as an initramfs.

## Config

Config files look like this:

```yaml
init_file: /bin/sh
libraries: []
binaries: []
output_file: ./initramfs.cpio
```

### init_file

`init_file` gets copied in as the `/init` binary that will get called as the entrypoint by the kernel when the initramfs is loaded.

### libraries

Files in `libraries` are copied into the `/lib64` directory of the initramfs.

### binaries

Files in `binaries` are copied into the `/bin` directory of the initramfs.

### output_file

`output_file` is the file that the initramfs will be outputted to.
