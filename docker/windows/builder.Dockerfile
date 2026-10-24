# Cross-compiles craft and its test executables for x86_64-pc-windows-gnu.
FROM rust:1-bookworm
RUN apt-get update \
    && apt-get install -y --no-install-recommends gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 \
    && rm -rf /var/lib/apt/lists/* \
    && rustup component add clippy \
    && rustup target add x86_64-pc-windows-gnu
ENV CARGO_TARGET_DIR=/work/target/windows
WORKDIR /work
