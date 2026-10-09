# syntax=docker/dockerfile:1
FROM debian:trixie-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

COPY schema-installer schema-diagram-generator schema-sql-generator schema-reverse-engineer schema-migration-generator /usr/local/bin/
COPY README.md LICENSE /usr/share/doc/schema-rs/

CMD ["/bin/sh", "-c", "echo 'Available tools: schema-installer, schema-diagram-generator, schema-sql-generator, schema-reverse-engineer, schema-migration-generator'; echo 'Run: docker run --rm jstano/schema-rs <tool> [args...]'"]
