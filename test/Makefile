# Copyright(c) The Maintainers of Nanvix.
# Licensed under the MIT License.

# Builds everything.
all: all-fibonacci all-hello-world all-noop

# Cleans everything.
clean: clean-fibonacci clean-hello-world clean-noop

# Builds fibonacci.
all-fibonacci:
	$(MAKE) -C fibonacci all

# Cleans fibonacci.
clean-fibonacci:
	$(MAKE) -C fibonacci clean

# Builds hello-world.
all-hello-world:
	$(MAKE) -C hello-world all

# Cleans hello-world.
clean-hello-world:
	$(MAKE) -C hello-world clean

# Builds noop.
all-noop:
	$(MAKE) -C noop all

# Cleans noop.
clean-noop:
	$(MAKE) -C noop clean
