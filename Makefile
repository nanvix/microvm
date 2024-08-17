# Copyright(c) The Maintainers of Nanvix.
# Licensed under the MIT License.

#===================================================================================================
# Directories
#===================================================================================================

export ROOT_DIR     := $(CURDIR)
export BINARIES_DIR ?= $(ROOT_DIR)/bin
export BUILD_DIR    := $(ROOT_DIR)/build
export INCLUDE_DIR  := $(ROOT_DIR)/include
export SOURCES_DIR  := $(ROOT_DIR)/src
export TESTS_DIR    := $(ROOT_DIR)/test

#===================================================================================================
# Toolchain Configuration
#===================================================================================================

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
	$(MAKE) -C $(SOURCES_DIR) all

# Cleans microvm build
clean-microvm:
	$(MAKE) -C $(SOURCES_DIR) clean

# Builds tests.
all-tests:
	$(MAKE) -C $(TESTS_DIR) all

# Cleans tests build.
clean-tests:
	$(MAKE) -C $(TESTS_DIR) clean

# Runs tests.
run: all
	sudo -E $(BINARIES_DIR)/microvm.elf -kernel $(BINARIES_DIR)/hello-world.elf -memory 4MB
