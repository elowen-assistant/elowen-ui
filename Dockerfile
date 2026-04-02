FROM rust:1.88-bookworm AS build
WORKDIR /app

ARG TRUNK_VERSION=0.21.14

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz" \
    | tar -xz -C /usr/local/bin trunk
RUN rustup target add wasm32-unknown-unknown

COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY index.html index.html
COPY src src
COPY public public

RUN trunk build --release

FROM nginx:1.29-alpine
COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /app/dist /usr/share/nginx/html

EXPOSE 3000

CMD ["nginx", "-g", "daemon off;"]
