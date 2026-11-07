# syntax=docker/dockerfile:1
#
# Image of the CS2 replay server. Build from the repository root:
#   docker build -t cs2-server .
#
# The build needs network access: Cargo fetches the dependencies, and the build script of
# demoparser2 (`csgoproto`) generates Rust code from the Steam protobuf definitions with protoc.

FROM rust:1.89-bookworm AS build
RUN apt-get update \
 && apt-get install -y --no-install-recommends protobuf-compiler cmake git \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY parser-core parser-core
COPY server server

# The build script of demoparser2 clones the *current* GameTracking-CS2 repository to read the
# protobuf definitions, so two builds on different days could generate different code. Put the
# commit the server was tested with in place first; the script then finds its directory taken
# and keeps it. Update PROTO_COMMIT when parser-core moves to a newer demoparser2.
ARG PROTO_COMMIT=cce7bf1f190144c1c08fb0fe75fd3c9514f2637f
RUN cargo fetch --locked \
 && proto_dir="$(find "$CARGO_HOME/git/checkouts" -type d -path '*/src/csgoproto' | head -n 1)/GameTracking-CS2" \
 && test -n "$proto_dir" \
 && git init -q "$proto_dir" \
 && git -C "$proto_dir" fetch -q --depth 1 https://github.com/SteamDatabase/GameTracking-CS2.git "$PROTO_COMMIT" \
 && git -C "$proto_dir" checkout -q FETCH_HEAD

RUN cargo build --release --locked -p server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin cs2 \
 && mkdir /data && chown cs2 /data
COPY --from=build /src/target/release/server /usr/local/bin/server
ENV BIND_ADDR=0.0.0.0:8080 \
    DATA_DIR=/data
USER cs2
VOLUME /data
EXPOSE 8080
# No curl in the image: the binary checks itself.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD ["/usr/local/bin/server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/server"]
