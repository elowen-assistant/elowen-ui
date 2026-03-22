FROM rust:1.88-bookworm AS build
WORKDIR /app

RUN cargo install trunk --locked
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
