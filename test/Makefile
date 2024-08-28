# Copyright(c) The Maintainers of Nanvix.
# Licensed under the MIT License.

# Builds everything.
all: all-hello-world all-noop

# Cleans everything.
clean: clean-hello-world clean-noop

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
