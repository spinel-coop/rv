# Test rv ci with Lobsters
# https://github.com/lobsters/lobsters

FROM ruby:3.3-slim

# Install build dependencies for native gems
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    libssl-dev \
    zlib1g-dev \
    libffi-dev \
    libyaml-dev \
    libmariadb-dev \
    libsqlite3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Lobsters
RUN git clone --depth 1 https://github.com/lobsters/lobsters.git .

# Use the Ruby version from the image
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci
