# Copyright(c) The Maintainers of Nanvix.
# Licensed under the MIT License.

#===================================================================================================
# Build Artifacts
#===================================================================================================

# C source files.
SRC=$(wildcard *.c)

# Object files.
OBJ = $(SRC:.c=.o)

# Binary file.
BIN = $(BINARIES_DIR)/microvm.elf

#===================================================================================================
# Toolchain Configuration
#===================================================================================================

export CFLAGS := -Wall -Wextra -Werror -O3 -std=gnu17 -I$(INCLUDE_DIR)

#===================================================================================================
# Build Targets
#===================================================================================================

# Buidls everything.
all: $(OBJ)
	$(LD) -o $(BIN) $^

# Cleans everything.
clean:
	rm -f $(OBJ)
	rm -f $(BIN)

#===================================================================================================

# Builds a C source file.
%.o: %.c
	$(CC) $(CFLAGS) $< -c -o $@

# Builds an assembly source file.
%.o: %.S
	$(CC) $(CFLAGS) $< -c -o $@
