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
export INSTALL_DIR  := $(HOME)/microvm/bin

#===================================================================================================
# Toolchain Configuration
#===================================================================================================

export CARGO = $(HOME)/.cargo/bin/cargo
export CC := gcc
export LD := gcc

# Cargo Options
export CARGO_FEATURES ?= --no-default-features
ifeq ($(RELEASE),no)
export CARGO_FLAGS :=
else
export CARGO_FLAGS := --release
endif

#===================================================================================================
# Build Artifacts
#===================================================================================================

# Binary file
export BIN := microvm

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
	$(CARGO) build --all $(CARGO_FLAGS) $(CARGO_FEATURES)
ifeq ($(RELEASE),no)
	cp -f --preserve target/debug/$(BIN) $(BINARIES_DIR)/$(BIN)
else
	cp -f --preserve target/release/$(BIN) $(BINARIES_DIR)/$(BIN)
endif

# Cleans microvm build
clean-microvm:
	rm -f $(BINARIES_DIR)/$(BIN)
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
	$(CARGO) run $(CARGO_FLAGS) $(CARGO_FEATURES) -- -kernel $(BINARIES_DIR)/hello-world.elf

install: all-microvm
	mkdir -p $(INSTALL_DIR)
ifeq ($(RELEASE),no)
	cp -f --preserve target/debug/$(BIN) $(INSTALL_DIR)/$(BIN)
else
	cp -f --preserve target/release/$(BIN) $(INSTALL_DIR)/$(BIN)
endif
