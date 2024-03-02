.PHONY: build
build:
	cargo build --target=x86_64-unknown-linux-musl

.PHONY: initramfs
initramfs: build
	cargo run -p assemble-fs -- --config ./assemble-fs/initramfs-config.yaml

.PHONY: run
run: initramfs
	qemu-system-x86_64 \
		-m 2G \
		-kernel /boot/vmlinuz-6.7.6-200.fc39.x86_64 \
		-initrd ./initramfs.cpio \
		-display none \
		-serial stdio -append "console=ttyS0 root=/dev/sda" \
		-drive format=raw,file=filesystem.ext4 \
		--enable-kvm
