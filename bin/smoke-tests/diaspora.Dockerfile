# Test rv ci with Diaspora
# https://github.com/diaspora/diaspora

FROM ruby:3.3-slim

# Install build dependencies for native gems
# Diaspora uses: mysql2, pg, nokogiri, typhoeus (curl), mini_magick, yajl-ruby, rugged, idn-ruby
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    cmake \
    pkg-config \
    libssl-dev \
    zlib1g-dev \
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

# Use the Ruby version from the image
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci
