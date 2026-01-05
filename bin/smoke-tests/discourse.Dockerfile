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

# Use the Ruby version from the image (3.3.8)
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci -q
