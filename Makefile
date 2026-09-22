# Build and verification entry points for Realm of Us.
# Every target delegates to cargo or to a script under toolchain/; no rules live here.

SHELL := /bin/bash
.DEFAULT_GOAL := help

ROOT   := $(CURDIR)
CARGO  := cargo
BLENDER ?= blender
GODOT   ?= godot
PYTHON  ?= python3
VENV    := .venv
VPY     := $(VENV)/bin/python

UNAME := $(shell uname -s)
ifeq ($(UNAME),Darwin)
  PLATFORM := macos
  LIB_PREFIX := lib
  LIB_EXT := dylib
else ifeq ($(UNAME),Linux)
  PLATFORM := linux
  LIB_PREFIX := lib
  LIB_EXT := so
else
  PLATFORM := windows
  LIB_PREFIX :=
  LIB_EXT := dll
endif

TARGET    := rou/target/release
CLI       := build/bin/rou
GODOT_LIB := client/app/bin/$(LIB_PREFIX)rou_godot.$(LIB_EXT)
FFI_LIB   := $(TARGET)/$(LIB_PREFIX)rou.$(LIB_EXT)
ADDON     := dist/rou_blender.zip

export ROU_TOOLCHAIN_ROOT := $(ROOT)

.PHONY: help all rou godot-ext ffi addon viewer examples check test test-rust test-spatial test-git test-addon fuzz clean

help: ## List targets
	@grep -E '^[a-z][a-z-]*:.*## ' $(MAKEFILE_LIST) | awk -F ':.*## ' '{printf "  %-12s %s\n", $$1, $$2}'

all: rou godot-ext ffi addon ## CLI, Godot extension, C library and Blender extension

rou: ## Build the release CLI into build/bin/rou
	cd rou && $(CARGO) build --release -p rou
	mkdir -p build/bin
	cp rou/target/release/rou $(CLI)

godot-ext: ## Build the Godot extension into client/app/bin/
	cd rou && $(CARGO) build --release -p rou-godot
	mkdir -p client/app/bin
	cp $(TARGET)/$(LIB_PREFIX)rou_godot.$(LIB_EXT) $(GODOT_LIB)

ffi: ## Build the C library librou and rou/include/rou.h
	cd rou && $(CARGO) build --release -p rou-ffi

addon: ffi ## Package the Blender extension into dist/rou_blender.zip
	$(PYTHON) toolchain/build/package_addon.py --output $(ADDON)

viewer: godot-ext ## Export the standalone viewer application (opens any browse bundle)
	$(PYTHON) toolchain/build/fetch_templates.py
	$(PYTHON) toolchain/build/package_viewer.py --platform $(PLATFORM) --output-dir dist/viewer

examples: ffi ## Generate the calibration street and the Villa Anahid scene under build/
	$(BLENDER) --background --factory-startup --python-exit-code 1 --python examples/calibration/create_scene.py
	$(BLENDER) --background --factory-startup --python-exit-code 1 --python examples/villa_anahid/create_scene.py

$(VENV)/.ready: requirements-dev.txt
	$(PYTHON) -m venv $(VENV)
	$(VPY) -m pip install --quiet -r requirements-dev.txt
	touch $@

check: $(VENV)/.ready ## Format, lint and doc checks for Rust, Python and GDScript
	$(VPY) toolchain/build/check_code.py

test: test-rust test-spatial test-git ## All automated tests

test-rust: ## Rust unit, integration and doc tests
	cd rou && $(CARGO) test --workspace

test-spatial: godot-ext ## Reviewed spatial cases through the Godot extension
	$(PYTHON) toolchain/build/verify_spatial.py

test-git: rou godot-ext ## Two-author Git/LFS flow with real Blender and headless Godot
	$(PYTHON) toolchain/build/verify_git_realm.py

test-addon: addon ## Blender extension: local preview, authoring session, real install
	rm -rf build/addon-preview build/addon-authoring build/addon-install
	$(BLENDER) --background --factory-startup --python-exit-code 1 --python toolchain/tests/blender/verify_addon.py -- --addon-zip "$(ROOT)/$(ADDON)" --output "$(ROOT)/build/addon-preview"
	$(BLENDER) --background --factory-startup --python-exit-code 1 --python toolchain/tests/blender/verify_git_authoring.py -- --addon-zip "$(ROOT)/$(ADDON)" --output "$(ROOT)/build/addon-authoring"
	mkdir -p build/addon-install/user/config build/addon-install/user/scripts
	BLENDER_USER_RESOURCES="$(ROOT)/build/addon-install/user" BLENDER_USER_CONFIG="$(ROOT)/build/addon-install/user/config" BLENDER_USER_SCRIPTS="$(ROOT)/build/addon-install/user/scripts" \
	  $(BLENDER) --background --factory-startup --python-exit-code 1 --python toolchain/tests/blender/verify_addon_install.py -- --addon-zip "$(ROOT)/$(ADDON)" --output "$(ROOT)/build/addon-install"

fuzz: ## Run the three libFuzzer targets for 60 s each (needs nightly and cargo-fuzz)
	cd rou && for t in json_strict glb_precheck lfs_pointer; do $(CARGO) +nightly fuzz run $$t -- -max_total_time=60; done

clean: ## Remove build/, dist/ and Rust targets (keeps .venv and .cache)
	rm -rf build dist rou/target rou/fuzz/target client/app/bin
