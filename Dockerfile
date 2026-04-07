FROM alpine:3.21
COPY target/x86_64-unknown-linux-musl/release/ptaui /ptaui
ENTRYPOINT ["/ptaui"]
