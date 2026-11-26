# The PHP oracle: the PHP version the recorded vectors come from, plus the
# extensions the recorders need. Recording and comparison only; never
# deployed.
FROM php:8.4.27-cli-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends libicu-dev \
    && docker-php-ext-install intl \
    && rm -rf /var/lib/apt/lists/*

# /work is this repository (read-only).
WORKDIR /work
ENTRYPOINT ["php", "-d", "display_errors=stderr"]
