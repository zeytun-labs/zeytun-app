.DEFAULT_GOAL := help
.NOTPARALLEL:
.DELETE_ON_ERROR:
SHELL := /bin/bash

PNPM ?= pnpm
PYTHON ?= python3
CORE_REPO ?= https://github.com/zeytun-labs/zeytun-core.git
CORE_REF ?= zeytun-port
CORE_SRC ?= .cache/zeytun-core
CORE_VERSION ?= zeytun-custom
CORE_TAGS_BASE := with_quic with_grpc with_dhcp with_wireguard with_utls with_acme with_clash_api with_v2ray_api with_gvisor with_embedded_tor with_tailscale with_ccm with_ocm with_cloudflared badlinkname
CORE_TAGS ?= $(CORE_TAGS_BASE) with_naive_outbound
UNAME_M := $(shell uname -m)
TARGET ?= $(if $(filter arm64 aarch64,$(UNAME_M)),macos-aarch64,macos-x86_64)
export PNPM PYTHON CORE_REPO CORE_REF CORE_SRC CORE_VERSION CORE_TAGS TARGET

.PHONY: help setup setup-web doctor doctor-web install-deps hooks resources core-source \
 build-core build-core-ci geoip resources-check repair-dev dev dev-web build build-ui \
 build-web frontend-release check test fmt fmt-check check-rs ci ci-frontend \
 prepare precommit csp-sync csp-check check-tooling version disk clean

help: ## Show development commands (run all commands from this repository)
	@awk 'BEGIN {FS = ":.*## "; printf "Zeytun development\n\n"} /^[a-zA-Z0-9_-]+:.*## / {printf "  %-20s %s\n", $$1, $$2}' $(firstword $(MAKEFILE_LIST))

setup: setup-web doctor resources ## Install dependencies, hooks, core, and GeoIP for the macOS app

setup-web: doctor-web install-deps hooks ## Prepare frontend development on macOS or Linux

doctor: ## Check native macOS and frontend prerequisites
	@$(PYTHON) scripts/dev.py doctor --native

doctor-web: ## Check Node, pnpm, Python, Git, and Make
	@$(PYTHON) scripts/dev.py doctor

install-deps: ## Install the exact frontend dependencies from the lockfile
	@$(PNPM) install --frozen-lockfile

hooks: ## Install this clone's pre-commit hook
	@$(PYTHON) scripts/dev.py hooks

core-source: ## Clone the public core into .cache/ if it is missing
	@bash scripts/native-resources.sh core-source

build-core: ## Build the production core, including Naive (CGO enabled)
	@MAKE="$(MAKE)" bash scripts/native-resources.sh build-core

build-core-ci: ## Build the CI core without Naive or CGO
	@CORE_TAGS="$(CORE_TAGS_BASE)" CGO_ENABLED=0 bash scripts/native-resources.sh build-core-ci

geoip: ## Download and validate the current DB-IP country database atomically
	@PYTHON="$(PYTHON)" bash scripts/native-resources.sh geoip

resources: ## Prepare missing native resources; keep existing core/data
	@PYTHON="$(PYTHON)" MAKE="$(MAKE)" bash scripts/native-resources.sh resources

resources-check: ## Validate resources before a native build
	@PYTHON="$(PYTHON)" bash scripts/native-resources.sh check

repair-dev: ## Remove generated core copies that the app made root-owned
	@$(PYTHON) scripts/dev.py repair-dev

dev: resources-check repair-dev ## Start the isolated Zeytun Dev app and Vite together
	@$(PNPM) run tauri:dev

dev-web: ## Start only Vite (native API calls require Tauri)
	@$(PNPM) run dev

build: resources-check repair-dev disk ## Build a local app/DMG without updater signing keys
	@$(PNPM) exec tauri build --config '{"build":{"beforeBuildCommand":"make frontend-release"},"bundle":{"createUpdaterArtifacts":false}}'

build-ui: build ## Alias for build

build-web: ## Build the static frontend
	@$(PNPM) run build

frontend-release: build-web csp-sync ## Build frontend and refresh CSP for a local native bundle

check: ## Type-check Svelte and TypeScript
	@$(PNPM) run check

test: ## Run frontend tests
	@$(PNPM) run test

fmt: ## Format Rust sources
	@cd src-tauri && cargo fmt --all

fmt-check: ## Check Rust formatting
	@cd src-tauri && cargo fmt --all --check

check-rs: resources-check repair-dev disk fmt-check ## Run the same Clippy and Rust tests as CI
	@cd src-tauri && cargo clippy --all-targets --all-features -- -D warnings -A clippy::result_large_err
	@cd src-tauri && cargo test --all-targets --features updater-tools

ci-frontend: version check-tooling check test frontend-release csp-check ## Validate frontend and generate CSP for this exact build

ci: ci-frontend check-rs ## Run all local CI checks before opening a PR

prepare: version check-tooling check test build-web csp-sync ## Validate frontend and regenerate CSP before staging

precommit: ## Check staged changes and prepare generated CSP (normally run by Git)
	@MAKE="$(MAKE)" $(PYTHON) scripts/pre-commit.py

csp-sync: ## Update script hashes from an existing build (prefer make prepare)
	@$(PYTHON) scripts/check-csp-hashes.py --write

csp-check: ## Verify CSP hashes against an existing build
	@$(PYTHON) scripts/check-csp-hashes.py

check-tooling: ## Test development scripts and hook safeguards
	@$(PYTHON) -m unittest discover -s scripts/tests -p 'test_*.py'

version: ## Verify package.json, Cargo.toml, and Tauri versions agree
	@$(PYTHON) scripts/dev.py version

disk: ## Check free disk space before compiling native code
	@$(PYTHON) scripts/dev.py disk

clean: ## Remove frontend output and Cargo build artifacts; keep downloaded resources
	@rm -rf build .svelte-kit
	@cd src-tauri && cargo clean
