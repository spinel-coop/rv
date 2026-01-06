# Test rv ci with GitLab
# https://gitlab.com/gitlab-org/gitlab

FROM ruby:3.2-slim

# Install build dependencies for native gems
# Based on GitLab's source installation docs
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    pkg-config \
    cmake \
    libssl-dev \
    zlib1g-dev \
    libyaml-dev \
    libgdbm-dev \
    libre2-dev \
    libreadline-dev \
    libncurses-dev \
    libffi-dev \
    libxml2-dev \
    libxslt-dev \
    libcurl4-openssl-dev \
    libicu-dev \
    libkrb5-dev \
    libpq-dev \
    libpcre2-dev \
    libgpgme-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone GitLab (shallow clone, it's huge)
RUN git clone --depth 1 https://gitlab.com/gitlab-org/gitlab.git .

# Use the Ruby version from the image
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci
