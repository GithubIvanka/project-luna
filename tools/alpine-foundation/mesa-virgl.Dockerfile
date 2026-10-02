FROM alpine:3.24
RUN apk add --no-cache \
    build-base libdrm-dev libxdamage-dev libxshmfence-dev binutils bison cbindgen \
    elfutils-dev eudev-dev expat-dev findutils flex gettext glslang-dev libdisplay-info-dev \
    libtool libva-dev libx11-dev libxfixes-dev libxml2-dev libxrandr-dev libxxf86vm-dev \
    llvm22-dev meson py3-cparser py3-mako py3-packaging py3-ply py3-yaml python3 \
    vulkan-loader-dev wayland-dev wayland-protocols xorgproto zlib-dev zstd-dev \
    clang22-dev libclc-dev~22
ARG MESA_VERSION=26.1.6
ARG MESA_SHA256=5296b88a0f1e012e2cb9ada150a2bbadf728ca81e5a4fb2ab43c83a4d2158606
ADD https://archive.mesa3d.org/mesa-$MESA_VERSION.tar.xz /src/mesa.tar.xz
RUN echo "$MESA_SHA256  /src/mesa.tar.xz" | sha256sum -c -
RUN mkdir -p /src/mesa && tar -xf /src/mesa.tar.xz -C /src/mesa --strip-components=1
WORKDIR /src/mesa
RUN meson setup output \
    -Dbuildtype=release -Db_ndebug=true -Dbuild-tests=false \
    -Dgallium-drivers=virgl,llvmpipe \
    -Dvulkan-drivers= \
    -Dvulkan-layers= \
    -Dllvm=enabled -Dshared-llvm=enabled -Dgbm=enabled \
    -Dplatforms=x11,wayland \
    -Dglx=dri -Dopengl=true -Dgles1=enabled -Dgles2=enabled -Degl=enabled \
    -Ddri-drivers-path=/usr/lib/dri -Dgallium-rusticl=false \
    -Dexpat=enabled -Dzstd=enabled -Dshader-cache=enabled -Dxlib-lease=enabled \
    -Dallow-kcmp=enabled -Damdgpu-virtio=true -Dvideo-codecs=all -Dgallium-va=enabled
RUN meson compile -C output -j 4
RUN DESTDIR=/out meson install --no-rebuild -C output

