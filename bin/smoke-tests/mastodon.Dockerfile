# Test rv ci with Mastodon
# https://github.com/mastodon/mastodon

FROM debian:bookworm-slim

# Install dependencies for native gems (precompiled Ruby needs only glibc)
# Based on Mastodon's official Dockerfile build stage
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    ca-certificates \
    libyaml-dev \
    libpq-dev \
    libicu-dev \
    libidn-dev \
    libxml2-dev \
    libxslt1-dev \
    zlib1g-dev \
    libpam0g-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Mastodon
RUN git clone --depth 1 https://github.com/mastodon/mastodon.git .

# Install Ruby (version detected from Gemfile.lock), add to PATH, then run rv ci
RUN rv ruby install && \
    export PATH="$(dirname $(rv ruby find)):$PATH" && \
    rv ci
