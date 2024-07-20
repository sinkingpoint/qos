KERNEL_RELEASE := 6.9.7-200.fc40.x86_64
.PHONY: build
build:
	cargo build --target=x86_64-unknown-linux-musl

.PHONY: initramfs
initramfs: build
	cargo run -p assemble-fs -- --kernel-release $(KERNEL_RELEASE) --config ./assemble-fs/initramfs-config.yaml

.PHONY: rootfs
rootfs: build
	cargo run -p assemble-fs -- --kernel-release $(KERNEL_RELEASE) --config ./assemble-fs/rootfs-config.yaml

.PHONY: run
run: initramfs rootfs
	qemu-system-x86_64 \
		-m 2G \
		-kernel /boot/vmlinuz-$(KERNEL_RELEASE) \
		-initrd ./target/initramfs.cpio \
		-echr 2 \
		-display curses \
		-append "console=ttyS0 root=/dev/sda" \
		-drive format=raw,file=./target/filesystem.ext4 \
		--enable-kvm
