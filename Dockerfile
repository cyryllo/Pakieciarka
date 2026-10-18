# --- build: Rust -> WASM (trunk) ---
FROM rust:1-slim-bookworm AS build
ARG TRUNK_VERSION=v0.21.14
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && rustup target add wasm32-unknown-unknown \
    && curl -sSL "https://github.com/trunk-rs/trunk/releases/download/${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
       | tar -xz -C /usr/local/bin
WORKDIR /src
COPY . .
RUN cd app && trunk build --release

# --- serwowanie statycznych plików ---
FROM nginx:1.27-alpine
COPY docker/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/app/dist /usr/share/nginx/html
EXPOSE 80
