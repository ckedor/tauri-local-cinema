.DEFAULT_GOAL := help
.PHONY: help setup deps dev build

# Omarchy/Arch packages: WebKitGTK (Tauri's webview), libmpv to link against
# (Windows player, Linux fallback), GStreamer demuxers/codecs for the <video>
# player on Linux, and gst-plugin-va for hardware decoding (H.264/HEVC on GPU).
PACMAN_DEPS := base-devel webkit2gtk-4.1 mpv gst-plugins-good gst-plugins-bad gst-libav gst-plugin-va

help: ## List the targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F ':.*## ' '{printf "  %-8s %s\n", $$1, $$2}'

setup: deps ## Everything after a fresh clone: system deps, Rust, node_modules, crates
	npm ci
	cd src-tauri && cargo fetch

# Asks for sudo only when something is missing, and only in a terminal: run
# without one (from the projects panel), it says what to run instead of hanging.
deps: ## System packages and Rust, only what is missing
	@command -v cargo >/dev/null || mise use -g rust
	@missing=$$(pacman -T $(PACMAN_DEPS) || true); \
	if [ -z "$$missing" ]; then echo "system deps ok"; \
	elif [ -t 0 ]; then sudo pacman -S --needed $$missing; \
	else echo "missing: $$missing -> run 'make deps' in a terminal (needs sudo)"; exit 1; fi

dev: ## Run with hot reload
	npm run tauri:dev

build: ## Standalone binary in src-tauri/target/release/ (no installer: the config has no icons)
	npx tauri build --no-bundle
