#!/bin/bash
set -e

cross build --release --target aarch64-unknown-linux-gnu
cross build --release --target x86_64-unknown-linux-gnu
cross build --release --target x86_64-pc-windows-gnu


#docker pull rust:1.91.1

#docker run --rm -v "$PWD":/usr/src/myapp -w /usr/src/myapp rust:1.91.1 bash -c "
#    apt-get update && \
#    apt-get install -y build-essential pkg-config libssl-dev && \
#    rustup target add x86_64-unknown-linux-gnu && \
#    cargo build --release --target x86_64-unknown-linux-gnu
#"

#docker run --rm -v "$PWD":/usr/src/myapp -w /usr/src/myapp rust:1.91.1 bash -c "
#    apt-get update && apt-get install -y gcc-aarch64-linux-gnu && \
#    rustup target add aarch64-unknown-linux-gnu && \
#    cargo build --release --target aarch64-unknown-linux-gnu
#"

#docker run --rm -v "$PWD":/usr/src/myapp -w /usr/src/myapp rust:1.91.1 bash -c "
#    apt-get update && apt-get install -y mingw-w64 && \
#    rustup target add x86_64-pc-windows-gnu && \
#    cargo build --release --target x86_64-pc-windows-gnu
#"
