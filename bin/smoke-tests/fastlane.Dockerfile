# Test rv ci with Fastlane
# Two-stage build: compile rv, then test with Fastlane

# Stage 1: Build rv
FROM rust:slim-bookworm AS builder
WORKDIR /rv
COPY . .
RUN ./bin/build-rv

# Stage 2: Test with Fastlane
FROM debian:bookworm-slim

# Install dependencies for rv and native extensions
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    curl \
    ca-certificates \
    git \
    libssl-dev \
    libreadline-dev \
    zlib1g-dev \
    libyaml-dev \
    libffi-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary from builder
COPY --from=builder /rv/target/release/rv /usr/local/bin/rv

WORKDIR /root

# Clone fastlane
RUN git clone --depth 1 https://github.com/fastlane/fastlane.git

WORKDIR /root/fastlane

# Fastlane requires Ruby >= 2.6 but has no .ruby-version file
RUN echo "3.3" > .ruby-version

# Install Ruby and run rv ci
# Need to source rv's shell env to set PATH for the installed Ruby
RUN rv ruby install && \
    eval "$(rv shell env bash)" && \
    rv ci -q
