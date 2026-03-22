FROM rust:1.88-bookworm
WORKDIR /app

RUN cargo install trunk --locked
RUN rustup target add wasm32-unknown-unknown

COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY index.html index.html
COPY src src
COPY public public

EXPOSE 3000

CMD ["trunk", "serve", "--release", "--address", "0.0.0.0", "--port", "3000"]
