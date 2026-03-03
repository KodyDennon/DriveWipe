# SPDX-License-Identifier: GPL-2.0
#
# DriveWipe Kernel Module Makefile
#
# Usage:
#   make                  # Build against running kernel
#   make KDIR=/path/to/kernel/build  # Build against specific kernel
#   make clean            # Remove build artifacts
#   make install          # Install module to /lib/modules/$(uname -r)/extra/

KDIR ?= /lib/modules/$(shell uname -r)/build
PWD  := $(shell pwd)

.PHONY: all clean install modules_install

all:
	$(MAKE) -C $(KDIR) M=$(PWD) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) clean

install: all
	$(MAKE) -C $(KDIR) M=$(PWD) modules_install
	depmod -a

modules_install:
	$(MAKE) -C $(KDIR) M=$(PWD) modules_install
