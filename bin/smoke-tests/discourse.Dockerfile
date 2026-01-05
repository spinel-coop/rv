# Test rv ci with Discourse
# Two-stage build: compile rv, then test with Discourse

# Stage 1: Build rv
FROM rust:slim-bookworm AS builder
WORKDIR /rv
COPY . .
RUN ./bin/build-rv

# Stage 2: Test with Discourse
FROM discourse/base:release

# Copy rv binary from builder
COPY --from=builder /rv/target/release/rv /usr/local/bin/rv

WORKDIR /var/www/discourse

# Clear pre-installed gems to test rv ci from scratch
RUN rm -rf vendor/bundle .bundle

# Discourse requires Ruby 3.3+
RUN echo "3.3" > .ruby-version

# Install Ruby and run rv ci
# Need to source rv's shell env to set PATH for the installed Ruby
RUN rv ruby install && \
    eval "$(rv shell env bash)" && \
    rv ci -q
