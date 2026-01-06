# Test rv ci with Homebrew
# https://github.com/Homebrew/brew

FROM ruby:3.4-slim

# Install build dependencies for native gems
# Homebrew's gems include profilers (ruby-prof, stackprof) and pycall
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    git \
    libffi-dev \
    libyaml-dev \
    python3-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /app

# Clone Homebrew
RUN git clone --depth 1 https://github.com/Homebrew/brew.git .

# Homebrew's Gemfile is in Library/Homebrew/
WORKDIR /app/Library/Homebrew

# Use the Ruby version from the image
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci
