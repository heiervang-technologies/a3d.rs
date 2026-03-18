PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin

.PHONY: all build release install uninstall test clean

all: install

build:
	cargo build

release:
	cargo build --release

install: release
	install -Dm755 target/release/a3d $(DESTDIR)$(BINDIR)/a3d

uninstall:
	rm -f $(DESTDIR)$(BINDIR)/a3d

test:
	cargo test

clean:
	cargo clean
