ARG ALPINE_IMAGE=alpine:3.24
FROM ${ALPINE_IMAGE}
RUN apk add --no-cache \
    git meson samurai gcc g++ musl-dev pkgconf \
    wayland-dev wayland-protocols mesa-dev \
    freetype-dev fontconfig-dev cairo-dev pango-dev harfbuzz-dev librsvg-dev \
    libxkbcommon-dev glib-dev libsecret-dev libsodium-dev polkit-dev \
    pipewire-dev wireplumber-dev curl-dev libqalculate-dev libxml2-dev libjxl-dev libsndfile-dev md4c-dev \
    nlohmann-json tomlplusplus-dev libical-dev linux-pam-dev \
    stb sdbus-cpp-dev
RUN mkdir -p /usr/include/stb \
    && ln -s ../stb_image_resize2.h /usr/include/stb/stb_image_resize2.h \
    && ln -s ../stb_image_write.h /usr/include/stb/stb_image_write.h
ARG NOCTALIA_REF=v5.2.0
RUN git clone --depth 1 --branch "$NOCTALIA_REF" https://github.com/noctalia-dev/noctalia.git /src/noctalia
WORKDIR /src/noctalia
RUN meson setup build-release \
    --buildtype=release -Dtests=disabled -Djemalloc=disabled -Dnative_optimizations=false \
    --prefix=/usr/local
RUN meson compile -C build-release -j 4
RUN DESTDIR=/out meson install -C build-release

