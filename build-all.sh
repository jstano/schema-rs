#!/bin/bash
set -e
  
# Color output for clarity
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Extract version from Cargo.toml
VERSION=$(grep -m1 'version = ' Cargo.toml | sed -E 's/.*version = "([^"]+)".*/\1/')

if [ -z "$VERSION" ]; then
    echo -e "${RED}Failed to extract version from Cargo.toml${NC}"
    exit 1
fi

echo -e "${BLUE}Building schema-rs v${VERSION}${NC}"

# Define binaries to package
BINARIES=("schema-installer" "schema-diagram-generator" "schema-sql-generator")

# Define targets
TARGETS=("aarch64-unknown-linux-gnu" "x86_64-unknown-linux-gnu" "x86_64-pc-windows-gnu" "aarch64-apple-darwin")

# Clean, build, and test
echo -e "${BLUE}Cleaning and building...${NC}"
cargo clean && cargo build && cargo test

# Cross-compile for Linux and Windows
echo -e "${BLUE}Cross-compiling for Linux (aarch64)...${NC}"
cross build --release --target aarch64-unknown-linux-gnu

echo -e "${BLUE}Cross-compiling for Linux (x86_64)...${NC}"
cross build --release --target x86_64-unknown-linux-gnu

echo -e "${BLUE}Cross-compiling for Windows...${NC}"
cross build --release --target x86_64-pc-windows-gnu

# Native build for macOS
echo -e "${BLUE}Building for macOS (aarch64)...${NC}"
rustup target add aarch64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Clean up old releases
echo -e "${BLUE}Preparing release directory...${NC}"
rm -rf release/
mkdir -p release

# Package each target
for target in "${TARGETS[@]}"; do
    echo -e "${BLUE}Packaging ${target}...${NC}"

    staging_dir="release/schema-rs-${VERSION}-${target}"
    mkdir -p "$staging_dir"

    # Determine binary extension
    if [[ "$target" == "x86_64-pc-windows-gnu" ]]; then
        bin_ext=".exe"
    else
        bin_ext=""
    fi

    # Copy binaries
    for binary in "${BINARIES[@]}"; do
        src="target/${target}/release/${binary}${bin_ext}"
        if [ ! -f "$src" ]; then
            echo -e "${RED}Error: Binary not found at ${src}${NC}"
            exit 1
        fi
        cp "$src" "$staging_dir/${binary}${bin_ext}"
        chmod +x "$staging_dir/${binary}${bin_ext}"
    done

    # Copy documentation
    cp README.md "$staging_dir/"
    cp LICENSE "$staging_dir/"

    # Create archive
    if [[ "$target" == "x86_64-pc-windows-gnu" ]]; then
        # Use zip for Windows
        if command -v zip &> /dev/null; then
            (cd release && zip -r "schema-rs-${VERSION}-${target}.zip" "schema-rs-${VERSION}-${target}")
            echo -e "${GREEN}Created release/schema-rs-${VERSION}-${target}.zip${NC}"
        else
            # Fallback to tar.gz if zip not available
            tar -czf "release/schema-rs-${VERSION}-${target}.tar.gz" -C release "schema-rs-${VERSION}-${target}"
            echo -e "${GREEN}Created release/schema-rs-${VERSION}-${target}.tar.gz${NC}"
        fi
    else
        # Use tar.gz for Linux and macOS
        tar -czf "release/schema-rs-${VERSION}-${target}.tar.gz" -C release "schema-rs-${VERSION}-${target}"
        echo -e "${GREEN}Created release/schema-rs-${VERSION}-${target}.tar.gz${NC}"
    fi

    # Remove staging directory
    rm -rf "$staging_dir"
done

# Generate checksums
echo -e "${BLUE}Generating checksums...${NC}"
cd release
shasum -a 256 *.{tar.gz,zip} 2>/dev/null | grep -v 'shasum:' > SHA256SUMS.txt || shasum -a 256 *.tar.gz > SHA256SUMS.txt
cd ..

echo -e "${GREEN}✓ Release complete!${NC}"
echo -e "${BLUE}Contents of release/ directory:${NC}"
ls -lh release/

echo ""
echo -e "${BLUE}To verify checksums:${NC}"
echo "  cd release && shasum -a 256 -c SHA256SUMS.txt"
