# Test rv ci with Discourse
# Expects rv binary to be passed as build argument

FROM discourse/base:release

# Copy rv binary (passed via build context)
COPY rv /usr/local/bin/rv

WORKDIR /var/www/discourse

# Clear pre-installed gems to test rv ci from scratch
RUN rm -rf vendor/bundle .bundle

# Use the Ruby version from the image
RUN ruby -e 'puts RUBY_VERSION' > .ruby-version

# Run rv ci
RUN rv ci
