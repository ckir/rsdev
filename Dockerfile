# --- Stage 1: C-Library Builder (The "Static" Foundation) ---
FROM rust:1.92-slim AS c-builder

# Install build tools and bzip2 (required for alsa-lib extraction)
RUN apt-get update && apt-get install -y \
    musl-tools perl make wget gcc linux-libc-dev libtool gettext m4 bzip2 \
    && rm -rf /var/lib/apt/lists/*

# Fix kernel header paths so musl-gcc can find linux/version.h
RUN ln -s /usr/include/linux /usr/include/x86_64-linux-musl/linux && \
    ln -s /usr/include/asm-generic /usr/include/x86_64-linux-musl/asm-generic && \
    ln -s /usr/include/x86_64-linux-gnu/asm /usr/include/x86_64-linux-musl/asm

WORKDIR /tmp

# 1. Build OpenSSL 3.6.0 (Static)
RUN wget https://github.com/openssl/openssl/releases/download/openssl-3.6.0/openssl-3.6.0.tar.gz \
    && tar -xf openssl-3.6.0.tar.gz && cd openssl-3.6.0 \
    && CC=musl-gcc ./Configure linux-x86_64 no-shared no-async no-tests --prefix=/usr/local/musl \
    && make -j$(nproc) && make install_sw

# 2. Build ALSA Lib 1.2.12 (Static) - Required for rodio/cpal speaker output
RUN wget https://www.alsa-project.org/files/pub/lib/alsa-lib-1.2.12.tar.bz2 \
    && tar -xjf alsa-lib-1.2.12.tar.bz2 && cd alsa-lib-1.2.12 \
    && CC=musl-gcc ./configure --host=x86_64-linux-musl --prefix=/usr/local/musl \
       --enable-static --disable-shared --disable-python --disable-alisp --disable-topology \
    && make -j$(nproc) && make install

# --- Stage 2: Rust Builder ---
FROM rust:1.92-slim AS builder

# clang and libclang-dev are REQUIRED for alsa-sys to run bindgen
RUN apt-get update && apt-get install -y \
    musl-tools pkg-config clang libclang-dev curl unzip && rm -rf /var/lib/apt/lists/*

RUN curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v33.5/protoc-33.5-linux-x86_64.zip \
    && unzip -o protoc-33.5-linux-x86_64.zip -d /usr/local \
    && rm protoc-33.5-linux-x86_64.zip

ENV PROTOC=/usr/local/bin/protoc

RUN rustup target add x86_64-unknown-linux-musl
COPY --from=c-builder /usr/local/musl /usr/local/musl

# --- Environment Configuration for Static Linking ---
# OpenSSL
ENV OPENSSL_DIR=/usr/local/musl
ENV OPENSSL_STATIC=1
ENV OPENSSL_LIB_DIR=/usr/local/musl/lib64
ENV OPENSSL_INCLUDE_DIR=/usr/local/musl/include

# ALSA & Pkg-Config
ENV PKG_CONFIG_PATH=/usr/local/musl/lib/pkgconfig
ENV PKG_CONFIG_ALLOW_CROSS=1

# Bindgen (Required for alsa-sys to find headers)
ENV LIBCLANG_PATH=/usr/lib/llvm-14/lib
ENV BINDGEN_EXTRA_CLANG_ARGS="-I/usr/local/musl/include"

WORKDIR /usr/src/app
COPY . .

# Build all binaries in the workspace/project
RUN cargo build --release --target x86_64-unknown-linux-musl

# Create a temporary directory and copy only the executables there, then strip them
RUN mkdir -p /tmp/binaries && \
    find target/x86_64-unknown-linux-musl/release/ -maxdepth 1 -type f -executable \
    -exec cp {} /tmp/binaries/ \; && \
    find /tmp/binaries/ -maxdepth 1 -type f -executable -exec strip {} +

# --- Stage 3: Exporter ---
FROM scratch AS exporter
# This captures all stripped binaries and exports them to your host's output folder
COPY --from=builder /tmp/binaries/ /
