set shell := ["bash", "-euo", "pipefail", "-c"]

# SSH host alias for remote build machine (from ~/.ssh/config)
remote_host := env_var_or_default("CROSS_BUILD_HOST", "cicd-01.stano.com")

# Working directory on remote machine
remote_dir := env_var_or_default("CROSS_BUILD_DIR", "~/schema-rs-build")

# Extract version from Cargo.toml
version := `grep -m1 'version = ' Cargo.toml | sed -E 's/.*version = "([^"]+)".*/\1/'`

# Binaries to package
binaries := "schema-installer schema-diagram-generator schema-sql-generator schema-reverse-engineer"

# Docker Hub repository to publish images to
docker_repo := env_var_or_default("DOCKER_REPO", "jstano/schema-rs")

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
	for bin in {{binaries}}; do \
		cp target/{{target}}/release/$bin{{ext}} release/schema-{{version}}/; \
	done
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

# Build every release artifact: macOS locally, Linux + Windows on the remote machine
build-all-releases: build-macos-aarch64 build-all-remote
	@echo "✓ All release artifacts built (macOS local + remote)!"

# Stage binaries + docs for one build target into release/docker/{target}/ — used as a
# minimal Docker build context so we don't send the whole target/ dir to the daemon
_docker-stage target:
	rm -rf release/docker/{{target}}
	mkdir -p release/docker/{{target}}
	for bin in {{binaries}}; do \
		cp target/{{target}}/release/$bin release/docker/{{target}}/; \
	done
	cp README.md LICENSE release/docker/{{target}}/

# Build & push a single-arch image for one (cross target, docker platform, tag suffix)
_docker-build-push target platform suffix: (_docker-stage target)
	docker buildx build \
		--platform {{platform}} \
		-f docker/Dockerfile \
		-t {{docker_repo}}:{{version}}-{{suffix}} \
		--push \
		release/docker/{{target}}

# Build & push the linux/amd64 image (builds x86_64 binaries first if needed)
docker-build-push-amd64: build-linux-x86_64
	just _docker-build-push x86_64-unknown-linux-gnu linux/amd64 amd64

# Build & push the linux/arm64 image (builds aarch64 binaries first if needed)
docker-build-push-arm64: build-linux-aarch64
	just _docker-build-push aarch64-unknown-linux-gnu linux/arm64 arm64

# Build+push both arch images, then stitch them into one multi-arch manifest tagged
# {{version}} and latest
docker-publish: docker-build-push-amd64 docker-build-push-arm64
	docker buildx imagetools create \
		-t {{docker_repo}}:{{version}} \
		-t {{docker_repo}}:latest \
		{{docker_repo}}:{{version}}-amd64 \
		{{docker_repo}}:{{version}}-arm64
	@echo "✓ Published {{docker_repo}}:{{version}} (linux/amd64, linux/arm64) and :latest"

# Build every release artifact and publish the Docker images: the full release
release-all: build-all-releases docker-publish
	@echo "✓ Full release complete: zips packaged + Docker images published!"

# Show current configuration
@show-config:
	echo "Remote SSH host: {{remote_host}}"
	echo "Remote directory: {{remote_dir}}"
	echo "Docker repository: {{docker_repo}}"
	echo ""
	echo "Override with environment variables:"
	echo "  CROSS_BUILD_HOST=myhost CROSS_BUILD_DIR=~/builds just build-all-remote"
	echo "  DOCKER_REPO=myuser/schema-rs just docker-publish"
