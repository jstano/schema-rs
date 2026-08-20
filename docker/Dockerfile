# syntax=docker/dockerfile:1
#
# Bundles the 4 schema-rs CLI binaries into a single image. Built per-arch by the Justfile's
# docker-build-push-amd64 / docker-build-push-arm64 recipes against a staged build context
# (release/docker/{target}/) that already contains just that arch's prebuilt binaries, so this
# Dockerfile stays a simple COPY — no cross-compilation happens inside Docker.
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY schema-installer schema-diagram-generator schema-sql-generator schema-reverse-engineer /usr/local/bin/
COPY README.md LICENSE /usr/share/doc/schema-rs/

CMD ["/bin/sh", "-c", "echo 'Available tools: schema-installer, schema-diagram-generator, schema-sql-generator, schema-reverse-engineer'; echo 'Run: docker run --rm jstano/schema-rs <tool> [args...]'"]
