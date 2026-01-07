# Test rv ci with Discourse
# https://github.com/discourse/discourse
# Uses Discourse's production base image which has Ruby pre-installed

FROM discourse/base:release

# Copy rv binary
COPY rv /usr/local/bin/rv

WORKDIR /var/www/discourse

# Clear pre-installed gems to test rv ci from scratch
RUN rm -rf vendor/bundle .bundle

# Run rv ci (Ruby already in PATH from base image)
RUN rv ci
