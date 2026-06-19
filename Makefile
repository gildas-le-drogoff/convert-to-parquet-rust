PREFIX      ?= /usr/local
BINDIR      ?= $(PREFIX)/bin
MANDIR      ?= $(PREFIX)/share/man/man1
COMPLETEDIR ?= $(PREFIX)/share/bash-completion/completions

CARGO       ?= cargo
INSTALL     ?= sudo install
RM          ?= rm -f

BIN_NAME    := convert_to_parquet
RELEASE_BIN := target/release/$(BIN_NAME)
MAN_PAGE    := $(BIN_NAME).1

.PHONY: all build release debug test check fmt lint clean install uninstall man demo help doc

all: release

help:
	@echo "targets:"
	@echo "  build    alias for release"
	@echo "  release  compile in release mode"
	@echo "  debug    compile in debug mode"
	@echo "  test     run test suite"
	@echo "  check    cargo check (fast type-check)"
	@echo "  fmt      format code (rustfmt)"
	@echo "  lint     clippy lints"
	@echo "  doc      build mdBook documentation (book/)"
	@echo "  man      generate man page"
	@echo "  demo     run demo script"
	@echo "  install  install binary + man page to PREFIX (default /usr/local)"
	@echo "  uninstall remove installed files"
	@echo "  clean    remove build artifacts"
	@echo "  help     this message"

build: release

release:
	$(CARGO) build --release --bin $(BIN_NAME)

debug:
	$(CARGO) build --bin $(BIN_NAME)

test:
	$(CARGO) test

check:
	$(CARGO) check

fmt:
	$(CARGO) fmt

lint:
	$(CARGO) clippy -- -D warnings

man: release
	$(RELEASE_BIN) --man > $(MAN_PAGE)

install: release man
	$(INSTALL) -d $(DESTDIR)$(BINDIR)
	$(INSTALL) -m 755 $(RELEASE_BIN) $(DESTDIR)$(BINDIR)/$(BIN_NAME)
	$(INSTALL) -d $(DESTDIR)$(MANDIR)
	$(INSTALL) -m 644 $(MAN_PAGE) $(DESTDIR)$(MANDIR)/$(MAN_PAGE)

uninstall:
	$(RM) $(DESTDIR)$(BINDIR)/$(BIN_NAME)
	$(RM) $(DESTDIR)$(MANDIR)/$(MAN_PAGE)

demo: release
	@bash demo.sh

clean:
	$(CARGO) clean
	$(RM) $(MAN_PAGE)
	$(RM) -r book/book

doc:
	cd book && mdbook build
	@echo "Documentation built at book/book/"
