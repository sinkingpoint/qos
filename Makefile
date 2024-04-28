.PHONY: build
build:
	cargo build --target=x86_64-unknown-linux-musl

.PHONY: initramfs
initramfs: build
	cargo run -p assemble-fs -- --config ./assemble-fs/initramfs-config.yaml

.PHONY: rootfs
rootfs: build
	cargo run -p assemble-fs -- --config ./assemble-fs/rootfs-config.yaml

.PHONY: run
run: initramfs rootfs
	qemu-system-x86_64 \
		-m 2G \
		-kernel /boot/vmlinuz-6.8.6-200.fc39.x86_64 \
		-initrd ./target/initramfs.cpio \
		-echr 2 \
		-display curses \
		-append "console=ttyS0 root=/dev/sda" \
		-drive format=raw,file=./target/filesystem.ext4 \
		--enable-kvm
