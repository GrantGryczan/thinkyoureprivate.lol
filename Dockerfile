# syntax=docker/dockerfile:1

FROM rust:1.86-alpine AS build

WORKDIR /app

RUN apk add --no-cache clang lld musl-dev git file openssl-dev \
    openssl-libs-static

COPY . .

RUN --mount=type=cache,target=target \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry \
    <<END
set -eu
cargo build --locked --release --package web
cp ./target/release/web /bin/app
END

FROM alpine:3 AS final

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home /nonexistent \
    --shell /sbin/nologin \
    --no-create-home \
    --uid 10001 \
    app

USER app

COPY --from=build /bin/app /bin/app

ENTRYPOINT [ "/bin/app" ]
