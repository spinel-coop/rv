# Test rv ci with Huginn
# Two-stage build: compile rv, then test with Huginn

# Stage 1: Build rv
FROM rust:slim-bookworm AS builder
WORKDIR /rv
COPY . .
RUN ./bin/build-rv

# Stage 2: Test with Huginn
FROM huginn/huginn

# Copy rv binary from builder
COPY --from=builder /rv/target/release/rv /usr/local/bin/rv

WORKDIR /app

# Clear pre-installed gems to test rv ci from scratch
RUN rm -rf vendor/bundle .bundle

# Huginn uses Ruby 3.2
RUN echo "3.2" > .ruby-version

# Install Ruby and run rv ci
# Need to source rv's shell env to set PATH for the installed Ruby
RUN rv ruby install && \
    eval "$(rv shell env bash)" && \
    rv ci -q
