# Test rv ci with Diaspora
# https://github.com/diaspora/diaspora

FROM debian:bookworm-slim

# Install dependencies for native gems (precompiled Ruby needs only glibc)
# Diaspora uses: mysql2, pg, nokogiri, typhoeus (curl), mini_magick, yajl-ruby, rugged, idn-ruby
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    ca-certificates \
    cmake \
    pkg-config \
    libyaml-dev \
    libffi-dev \
    libxml2-dev \
    libxslt-dev \
    libpq-dev \
    default-libmysqlclient-dev \
    libcurl4-openssl-dev \
    libyajl-dev \
    libmagickwand-dev \
    libgit2-dev \
    libssh2-1-dev \
    libidn-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Diaspora
RUN git clone --depth 1 https://github.com/diaspora/diaspora.git .

# Install Ruby (version detected from Gemfile.lock), add to PATH, then run rv ci
RUN rv ruby install && \
    export PATH="$(dirname $(rv ruby find)):$PATH" && \
    rv ci
