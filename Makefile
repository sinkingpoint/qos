.PHONY: build
build:
	cargo build --target=x86_64-unknown-linux-musl

.PHONY: initramfs
initramfs: build
	cargo run -p assemble-initramfs -- --config ./assemble-initramfs/config.yaml

.PHONY: run
run: initramfs
	qemu-system-x86_64 \
		-m 2G \
		-kernel /boot/vmlinuz-6.6.8-200.fc39.x86_64 \
		-initrd ./initramfs.cpio \
		-display none \
		-serial stdio -append "console=ttyS0" \
		--enable-kvm
