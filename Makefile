include format.mk
include operating_system.mk

# RPI3 info
TARGET            = aarch64-unknown-none-softfloat
KERNEL_BIN        = kernel8.img
KERNEL_DEBUG_BIN  = kernel8_debug.img
QEMU_BINARY       = qemu-system-aarch64
QEMU_MACHINE_TYPE = raspi3
QEMU_ARGS 		    = -serial stdio -display none -semihosting -drive file=img.dmg,if=sd,format=raw -s -S
QEMU_DEBUG_ARGS   = $(QEMU_ARGS)

# Dependencies
KERNEL_MANIFEST      = Cargo.toml
KERNEL_LINKER_SCRIPT = src/bin/kernel/boot/kernel.ld
KERNEL_ELF      	 ?= target/$(TARGET)/release/kernel
FS_ELF      	 ?= target/$(TARGET)/release/fs
FS_DEBUG_ELF      	 ?= target/$(TARGET)/debug/fs
KERNEL_DEBUG_ELF     ?= target/$(TARGET)/debug/kernel
KERNEL_DEBUG_INFO ?= target/$(TARGET)/debug/kernel.dwp
KERNEL_ELF_DEPS = $(shell find src -type f) $(KERNEL_MANIFEST)

RUSTC_CMD   = cargo rustc $(COMPILER_ARGS)
DOC_CMD     = cargo doc $(COMPILER_ARGS)
CLIPPY_CMD  = cargo clippy $(COMPILER_ARGS)
TEST_CMD    = cargo test $(COMPILER_ARGS) --release
OBJCOPY_CMD = rust-objcopy \
    --strip-all            \
    -O binary

EXEC_QEMU = $(QEMU_BINARY) -M $(QEMU_MACHINE_TYPE)

# Docker
DOCKER_IMAGE 				= rustembedded/osdev-utils:2021.12
DOCKER_FOLDER = /work/tutorial
DOCKER_CMD          = docker run -t --rm -v $(shell pwd):$(DOCKER_FOLDER) -w $(DOCKER_FOLDER)
DOCKER_CMD_INTERACT = $(DOCKER_CMD) -i
DOCKER_QEMU  = $(DOCKER_CMD_INTERACT) $(DOCKER_IMAGE)
DOCKER_TOOLS = $(DOCKER_CMD) $(DOCKER_IMAGE)
DOCKER_TEST  = $(DOCKER_CMD) $(DOCKER_ARG_DIR_COMMON) $(DOCKER_IMAGE)

.PHONY: all doc qemu clippy clean readelf objdump nm check tmp test

all: $(KERNEL_BIN)

# Compile the kernel
$(KERNEL_ELF): $(KERNEL_ELF_DEPS)
	$(call color_header, "Compiling kernel ELF")
	cargo build --release

$(KERNEL_DEBUG_ELF): $(KERNEL_ELF_DEPS)
	$(call color_header, "Compiling kernel ELF")
	cargo build

# Generate binary
$(KERNEL_BIN): $(KERNEL_ELF)
	$(call color_header, "Generating stripped binary")
	@cp $(KERNEL_ELF) $(KERNEL_BIN)
	@python3 append_size.py $(KERNEL_BIN) $(FS_ELF)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_BIN))

$(KERNEL_DEBUG_BIN): $(KERNEL_DEBUG_ELF)
	$(call color_header, "Generating binary")
	@cpy $(KERNEL_DEBUG_ELF) $(KERNEL_DEBUG_BIN)
	@python3 append_size.py $(KERNEL_DEBUG_BIN) $(FS_DEBUG_ELF)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_DEBUG_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_DEBUG_BIN))

# Running in QEMU
qemu: $(KERNEL_BIN)
	$(call color_header, "Launching QEMU")
	@$(DOCKER_QEMU) $(EXEC_QEMU) $(QEMU_ARGS) -kernel $(KERNEL_BIN)

qemu_debug: $(KERNEL_DEBUG_BIN)
	$(call color_header, "Launching QEMU with debugging...")
	@$(DOCKER_QEMU) $(EXEC_QEMU) $(QEMU_DEBUG_ARGS) -kernel $(KERNEL_DEBUG_BIN)

gdb:
	docker exec -it $(shell docker ps | grep $(DOCKER_IMAGE) | head -c 12) gdb-multiarch $(DOCKER_FOLDER)/$(KERNEL_DEBUG_INFO) -ex "target remote localhost:1234"

in_docker:
	docker run -it -v $(shell pwd):$(DOCKER_FOLDER) -w $(DOCKER_FOLDER) $(DOCKER_IMAGE) /bin/bash
