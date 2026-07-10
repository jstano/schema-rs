set shell := ["bash", "-euo", "pipefail", "-c"]

# SSH host alias for remote build machine (from ~/.ssh/config)
remote_host := env_var_or_default("CROSS_BUILD_HOST", "54.89.164.24")

# Working directory on remote machine
remote_dir := env_var_or_default("CROSS_BUILD_DIR", "~/schema-rs-build")

# Extract version from Cargo.toml
version := `grep -m1 'version = ' Cargo.toml | sed -E 's/.*version = "([^"]+)".*/\1/'`

# Binaries to package
binaries := "schema-installer schema-diagram-generator schema-sql-generator"

# Sync local repo to remote machine
sync-remote:
	rsync -az --delete \
		--exclude='target/' \
		--exclude='.git/' \
		--exclude='release/' \
		--exclude='.idea/' \
		--exclude='.DS_Store' \
		--exclude='*.swp' \
		--exclude='*.swo' \
		./ {{remote_host}}:{{remote_dir}}/

# Build a single target on the remote machine
_build-remote target:
	ssh {{remote_host}} "bash -lc 'cd {{remote_dir}} && cross build --release --target {{target}}'"

# Fetch built artifacts from remote machine
_fetch target:
	mkdir -p target/{{target}}/release
	rsync -az {{remote_host}}:{{remote_dir}}/target/{{target}}/release/ target/{{target}}/release/

# Zip built binaries + docs for a target into release/{name}.zip, staged under schema-{version}/
_zip target name ext="":
	@echo "Packaging {{name}}.zip..."
	rm -rf release/schema-{{version}}
	mkdir -p release/schema-{{version}}
	cp target/{{target}}/release/schema-installer{{ext}} release/schema-{{version}}/
	cp target/{{target}}/release/schema-diagram-generator{{ext}} release/schema-{{version}}/
	cp target/{{target}}/release/schema-sql-generator{{ext}} release/schema-{{version}}/
	cp README.md release/schema-{{version}}/
	cp LICENSE release/schema-{{version}}/
	rm -f release/{{name}}.zip
	(cd release && zip -r {{name}}.zip schema-{{version}})
	rm -rf release/schema-{{version}}
	@echo "✓ Created release/{{name}}.zip"

# Build for Linux aarch64 (aarch64-unknown-linux-gnu) on remote machine
build-linux-aarch64: sync-remote
	@echo "Building aarch64-unknown-linux-gnu on {{remote_host}}..."
	just _build-remote aarch64-unknown-linux-gnu
	@echo "Fetching aarch64-unknown-linux-gnu artifacts..."
	just _fetch aarch64-unknown-linux-gnu
	just _zip aarch64-unknown-linux-gnu linux-aarch64

# Build for Linux x86_64 (x86_64-unknown-linux-gnu) on remote machine
build-linux-x86_64: sync-remote
	@echo "Building x86_64-unknown-linux-gnu on {{remote_host}}..."
	just _build-remote x86_64-unknown-linux-gnu
	@echo "Fetching x86_64-unknown-linux-gnu artifacts..."
	just _fetch x86_64-unknown-linux-gnu
	just _zip x86_64-unknown-linux-gnu linux-x86_64

# Build for Windows x86_64 (x86_64-pc-windows-gnu) on remote machine
build-windows-x86_64: sync-remote
	@echo "Building x86_64-pc-windows-gnu on {{remote_host}}..."
	just _build-remote x86_64-pc-windows-gnu
	@echo "Fetching x86_64-pc-windows-gnu artifacts..."
	just _fetch x86_64-pc-windows-gnu
	just _zip x86_64-pc-windows-gnu windows-x86_64 .exe

# Build all cross-compile targets on remote machine
build-all-remote: sync-remote
	@echo "Building all targets on {{remote_host}}..."
	just _build-remote aarch64-unknown-linux-gnu
	just _build-remote x86_64-unknown-linux-gnu
	just _build-remote x86_64-pc-windows-gnu
	@echo "Fetching all artifacts..."
	just _fetch aarch64-unknown-linux-gnu
	just _fetch x86_64-unknown-linux-gnu
	just _fetch x86_64-pc-windows-gnu
	@echo "Zipping all artifacts..."
	just _zip aarch64-unknown-linux-gnu linux-aarch64
	just _zip x86_64-unknown-linux-gnu linux-x86_64
	just _zip x86_64-pc-windows-gnu windows-x86_64 .exe
	@echo "✓ All remote builds and packaging complete!"

# Build for macOS AARCH64 (aarch64-apple-darwin) natively on macOS
build-macos-aarch64:
	@echo "Building aarch64-apple-darwin locally..."
	rustup target add aarch64-apple-darwin
	cargo build --release --target aarch64-apple-darwin
	just _zip aarch64-apple-darwin macos-aarch64
	@echo "✓ macOS build and packaging complete!"

# Show current configuration
@show-config:
	echo "Remote SSH host: {{remote_host}}"
	echo "Remote directory: {{remote_dir}}"
	echo ""
	echo "Override with environment variables:"
	echo "  CROSS_BUILD_HOST=myhost CROSS_BUILD_DIR=~/builds just build-all-remote"
