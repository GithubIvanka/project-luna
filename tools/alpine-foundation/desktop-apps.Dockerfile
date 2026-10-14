# Freeze the resulting image ID with the exported canonical inputs.
ARG ALPINE_IMAGE=alpine:3.24.2
FROM ${ALPINE_IMAGE}
RUN apk add --no-cache \
    build-base pkgconf gtk4.0-dev libadwaita-dev gtk4-layer-shell-dev \
    rust cargo cmake git curl linux-headers blueprint-compiler \
    bash fish ncurses patchelf file
