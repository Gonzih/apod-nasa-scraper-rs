BACKTRACE ?= 0
CARGO = cargo --color always
CARGO_ARGS = $(if $(RELEASE),--release) $(if $(STATIC_BINARY), --target=x86_64-unknown-linux-musl)

.PHONY: run-nix
run-nix:
	nix-shell shell.nix --run 'make run'

.PHONY: build-nix
build-nix:
	nix-shell shell.nix --run 'make build'

.PHONY: build
build:
	$(CARGO) build $(CARGO_ARGS)

.PHONY: run
run:
	$(CARGO) run $(CARGO_ARGS) -- --directory "$(DIRECTORY)"

.PHONY: test
test:
	$(CARGO) test $(CARGO_ARGS)

.PHONY: shell
shell:
	nix-shell shell.dev.nix
