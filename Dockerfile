# Stage 1: Build the binary
FROM rust:1.75.0-bookworm AS builder

# Install Protocol Buffers compiler.
RUN apt-get update && apt-get install -y protobuf-compiler

# Create a working directory for the application.
WORKDIR /usr/src/transmit

# Copy the Cargo.toml and Cargo.lock files to leverage Docker layer caching.
COPY Cargo.toml Cargo.lock ./

# Build and cache dependencies by running a dummy build.
RUN mkdir src && echo "fn main() {}" > src/main.rs && touch src/lib.rs && cargo build --release && rm -rf src

# Copy the relevant application source code to the container.
COPY ./src ./src
COPY ./migrations ./migrations
COPY ./build.rs ./build.rs
COPY ./.sqlx ./.sqlx
COPY ./proto ./proto

# Turn off sqlx compile-time verification.
ENV SQLX_OFFLINE true

# Build the application.
RUN cargo build --release

# Stage 2: Create the final image.
FROM debian:bookworm-slim

# Copy the built binary from the previous stage.
COPY --from=builder /usr/src/transmit/target/release/transmit /usr/local/bin/transmit

# In this directory, docker-compose can mount the configuration file.
WORKDIR /usr/src/transmit

# Set the entry point for the container.
ENTRYPOINT ["/usr/local/bin/transmit"]
