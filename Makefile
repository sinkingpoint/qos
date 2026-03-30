KERNEL_RELEASE := $(shell uname -r)
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
		-display gtk,grab-on-hover=on \
		-device virtio-vga \
		-append "root=/dev/sda" \
		-drive format=raw,file=./target/filesystem.ext4 \
		-netdev user,id=net0,dhcpstart=10.0.2.15 \
		-device e1000,netdev=net0,mac=00:12:34:56:78:9b \
		-object filter-dump,id=f1,netdev=net0,file=./target/dhcp.pcap \
		--enable-kvm
