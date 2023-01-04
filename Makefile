# Scripts adapted from https://github.com/rust-embedded/rust-raspberrypi-OS-tutorials

include format.mk
include operating_system.mk

# RPI3 info
TARGET            = aarch64-unknown-none-softfloat
KERNEL_BIN        = kernel8.img
KERNEL_DEBUG_BIN  = kernel8_debug.img
QEMU_BINARY       = qemu-system-aarch64
QEMU_MACHINE_TYPE = raspi3
QEMU_ARGS 		  = -serial stdio -display none -smp 4 -semihosting
QEMU_DEBUG_ARGS   = -serial stdio -display none -smp 4 -semihosting -s -S
OBJDUMP_BINARY    = aarch64-none-elf-objdump
NM_BINARY         = aarch64-none-elf-nm
READELF_BINARY    = aarch64-none-elf-readelf
LD_SCRIPT_PATH    = $(shell pwd)/src/board

VERBOSE ?= 0

# Export for build.rs.
export LD_SCRIPT_PATH

# Dependencies
KERNEL_MANIFEST      = Cargo.toml
KERNEL_LINKER_SCRIPT = kernel.ld
KERNEL_ELF      	 ?= target/$(TARGET)/release/kernel
KERNEL_DEBUG_ELF     ?= target/$(TARGET)/debug/kernel
KERNEL_ELF_DEPS = $(shell find src -type f) $(KERNEL_MANIFEST)

# Rust + other build things
RUSTFLAGS = $(RUSTC_MISC_ARGS)                   \
    -C link-arg=--library-path=$(LD_SCRIPT_PATH) \
    -C link-arg=--script=$(KERNEL_LINKER_SCRIPT) \
		-C target-cpu=cortex-a53
RUSTFLAGS_DEBUG = -g
RUSTFLAGS_NODEBUG = --release

RUSTFLAGS_PEDANTIC = $(RUSTFLAGS) \
    # -D warnings                   \
    -D missing_docs

COMPILER_ARGS = --target=$(TARGET) --manifest-path $(KERNEL_MANIFEST) --features=verbose

ifeq ($(VERBOSE), 1)
COMPILER_ARGS += --features=verbose
endif

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

tmp:
	$(shell echo $(KERNEL_ELF_DEPS))

# Compile the kernel
$(KERNEL_ELF): $(KERNEL_ELF_DEPS)
	$(call color_header, "Compiling kernel ELF")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(RUSTC_CMD) $(RUSTFLAGS_NODEBUG)

$(KERNEL_DEBUG_ELF): $(KERNEL_ELF_DEPS)
	$(call color_header, "Compiling kernel ELF")
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC) $(RUSTFLAGS_DEBUG)" $(RUSTC_CMD)

# Generate binary
$(KERNEL_BIN): $(KERNEL_ELF)
	$(call color_header, "Generating stripped binary")
	@$(OBJCOPY_CMD) $(KERNEL_ELF) $(KERNEL_BIN)
	$(call color_progress_prefix, "Name")
	@echo $(KERNEL_BIN)
	$(call color_progress_prefix, "Size")
	$(call disk_usage_KiB, $(KERNEL_BIN))

$(KERNEL_DEBUG_BIN): $(KERNEL_DEBUG_ELF)
	$(call color_header, "Generating stripped binary")
	@$(OBJCOPY_CMD) $(KERNEL_DEBUG_ELF) $(KERNEL_DEBUG_BIN)
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
	docker exec -it $(shell docker ps | grep $(DOCKER_IMAGE) | head -c 12) gdb-multiarch $(DOCKER_FOLDER)/$(KERNEL_DEBUG_ELF) -ex "target remote localhost:1234"

# Clippy
clippy:
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(CLIPPY_CMD)

# Cleans all build stuff
clean:
	rm -rf target $(KERNEL_BIN)

##------------------------------------------------------------------------------
## Run readelf
##------------------------------------------------------------------------------
readelf: $(KERNEL_ELF)
	$(call color_header, "Launching readelf")
	@$(DOCKER_TOOLS) $(READELF_BINARY) --headers $(KERNEL_ELF)

##------------------------------------------------------------------------------
## Run objdump
##------------------------------------------------------------------------------
objdump: $(KERNEL_ELF)
	$(call color_header, "Launching objdump")
	@$(DOCKER_TOOLS) $(OBJDUMP_BINARY) --disassemble --demangle \
                --section .text   \
                $(KERNEL_ELF) | rustfilt

##------------------------------------------------------------------------------
## Run nm
##------------------------------------------------------------------------------
nm: $(KERNEL_ELF)
	$(call color_header, "Launching nm")
	@$(DOCKER_TOOLS) $(NM_BINARY) --demangle --print-size $(KERNEL_ELF) | sort | rustfilt

##------------------------------------------------------------------------------
## Helpers for unit and integration test targets
##------------------------------------------------------------------------------
define KERNEL_TEST_RUNNER
#!/usr/bin/env bash

    # The cargo test runner seems to change into the crate under test's directory. Therefore, ensure
    # this script executes from the root.
    cd $(shell pwd)

    TEST_ELF=$$(echo $$1 | sed -e 's/.*target/target/g')
    TEST_BINARY=$$(echo $$1.img | sed -e 's/.*target/target/g')

    $(OBJCOPY_CMD) $$TEST_ELF $$TEST_BINARY
    $(DOCKER_TEST) $(EXEC_TEST_DISPATCH) $(EXEC_QEMU) $(QEMU_ARGS) -kernel $$TEST_BINARY
endef

export KERNEL_TEST_RUNNER

define test_prepare
    @mkdir -p target
    @echo "$$KERNEL_TEST_RUNNER" > target/kernel_test_runner.sh
    @chmod +x target/kernel_test_runner.sh
endef

test:
	$(call color_header, "Compiling tests")
	$(call test_prepare)
	@RUSTFLAGS="$(RUSTFLAGS_PEDANTIC)" $(TEST_CMD) $(if $(TEST), --test $(TEST))
