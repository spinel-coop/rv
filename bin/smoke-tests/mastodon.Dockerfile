# Test rv ci with Mastodon
# https://github.com/mastodon/mastodon

FROM ruby:3.4-slim

# Install build dependencies for native gems
# Based on Mastodon's official Dockerfile build stage
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    libssl-dev \
    zlib1g-dev \
    libffi-dev \
    libyaml-dev \
    libpq-dev \
    libicu-dev \
    libidn-dev \
    libpam0g-dev \
    libgdbm-dev \
    libgmp-dev \
    libncurses-dev \
    libreadline-dev \
    libxml2-dev \
    libxslt1-dev \
    libjemalloc-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Mastodon
RUN git clone --depth 1 https://github.com/mastodon/mastodon.git .

# Use the Ruby version from the image
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci
