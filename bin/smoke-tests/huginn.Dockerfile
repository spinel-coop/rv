# Test rv ci with Huginn
# https://github.com/huginn/huginn

FROM debian:bookworm-slim

# Install dependencies for native gems (precompiled Ruby needs only glibc)
# Huginn uses: mysql2, pg, nokogiri, typhoeus (curl), and various others
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    ca-certificates \
    libyaml-dev \
    libffi-dev \
    libxml2-dev \
    libxslt-dev \
    libpq-dev \
    default-libmysqlclient-dev \
    libcurl4-openssl-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Huginn
RUN git clone --depth 1 https://github.com/huginn/huginn.git .

# Install Ruby (version detected from Gemfile.lock), add to PATH, then run rv ci
RUN rv ruby install && \
    export PATH="$(dirname $(rv ruby find)):$PATH" && \
    rv ci
