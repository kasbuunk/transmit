# Use the official Rust image as the base image.
FROM rust:latest

# Install Protocol Buffers compiler.
RUN apt-get update && apt-get install -y protobuf-compiler

# Create a working directory for the application.
WORKDIR /usr/src/transmit

# Copy the Cargo.toml and Cargo.lock files to leverage Docker layer caching.
COPY Cargo.toml Cargo.lock ./

# Build and cache dependencies by running a dummy build.
RUN mkdir src && echo "fn main() {}" > src/main.rs && touch src/lib.rs && cargo build --release && rm -rf src

# Copy the entire application source code to the container.
COPY . .

# Turn off sqlx compile-time verification.
ENV SQLX_OFFLINE true

# Build the application.
RUN cargo build --release

# Set the entry point for the container.
CMD ["./target/release/message_scheduler", "config.ron"]
