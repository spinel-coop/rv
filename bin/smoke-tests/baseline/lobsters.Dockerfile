# Baseline: Traditional Ruby installation for Lobsters
# Uses ruby-build to compile Ruby from source
# https://github.com/lobsters/lobsters

FROM debian:bookworm-slim

# Install Ruby build dependencies + native gem dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    curl \
    ca-certificates \
    libssl-dev \
    libreadline-dev \
    zlib1g-dev \
    libyaml-dev \
    libffi-dev \
    autoconf \
    bison \
    pkg-config \
    libmariadb-dev \
    libsqlite3-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Install ruby-build
RUN git clone --depth 1 https://github.com/rbenv/ruby-build.git /tmp/ruby-build && \
    /tmp/ruby-build/install.sh && \
    rm -rf /tmp/ruby-build

WORKDIR /app

# Clone Lobsters
RUN git clone --depth 1 https://github.com/lobsters/lobsters.git .

# Compile Ruby from source using ruby-build (this is the slow part)
RUN ruby-build "$(cat .ruby-version)" /usr/local

# Install gems
RUN bundle install
