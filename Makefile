# Copyright(c) The Maintainers of Nanvix.
# Licensed under the MIT License.

#===================================================================================================
# Directories
#===================================================================================================

export ROOT_DIR     := $(CURDIR)
export BINARIES_DIR ?= $(ROOT_DIR)/bin
export BUILD_DIR    := $(ROOT_DIR)/build
export SOURCES_DIR  := $(ROOT_DIR)/src
export TESTS_DIR    := $(ROOT_DIR)/test

#===================================================================================================
# Toolchain Configuration
#===================================================================================================

export CARGO = $(HOME)/.cargo/bin/cargo
export CC := gcc
export LD := gcc

#===================================================================================================
# Build Targets
#===================================================================================================

# Buidls everything.
all: make-dirs | all-microvm all-tests

# Creates output directories.
make-dirs:
	mkdir -p $(BINARIES_DIR)

# Cleans everything.
clean: clean-tests clean-microvm

# Builds microvm.
all-microvm:
	$(CARGO) build --release

# Cleans microvm build
clean-microvm:
	$(CARGO) clean
	rm -rf Cargo.lock target

# Builds tests.
all-tests:
	$(MAKE) -C $(TESTS_DIR) all

# Cleans tests build.
clean-tests:
	$(MAKE) -C $(TESTS_DIR) clean

# Runs tests.
run: all
	$(CARGO) run --release -- -kernel $(BINARIES_DIR)/hello-world.elf
