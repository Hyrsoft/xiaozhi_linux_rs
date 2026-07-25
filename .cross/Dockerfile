# syntax=docker/dockerfile:1.7

FROM ubuntu:20.04

ARG DEBIAN_FRONTEND=noninteractive
ARG TARGET
ARG CROSS_PREFIX
ARG TOOLCHAIN_ASSET=none
ARG TOOLCHAIN_SHA256=none
ARG TOOLCHAIN_DIR=none

ARG ALSA_ASSET=alsa-lib-1.2.12.tar.bz2
ARG ALSA_SHA256=4868cd908627279da5a634f468701625be8cc251d84262c7e5b6a218391ad0d2
ARG OPUS_ASSET=opus-1.5.2.tar.gz
ARG OPUS_SHA256=65c1d2f78b9f2fb20082c38cbe47c951ad5839345876e46941612ee87f9a7ce1
ARG SPEEXDSP_ASSET=speexdsp-1.2.1.tar.gz
ARG SPEEXDSP_SHA256=8c777343e4a6399569c72abc38a95b24db56882c83dbdb6c6424a5f4aeb54d3d

LABEL org.opencontainers.image.source="https://github.com/haoyn231/xiaozhi_linux_rs"
LABEL org.opencontainers.image.description="Cross SDK for xiaozhi_linux_rs"
LABEL org.cross-rs.for-cross-target="${TARGET}"

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        autoconf \
        automake \
        build-essential \
        ca-certificates \
        curl \
        file \
        libtool \
        make \
        pkg-config \
        xz-utils \
        bzip2 \
    && rm -rf /var/lib/apt/lists/*

RUN set -eux; \
    if [ "${TOOLCHAIN_ASSET}" != "none" ]; then \
        url="https://github.com/haoyn231/xiaozhi_linux_rs/releases/download/Source_Mirror/${TOOLCHAIN_ASSET}"; \
        curl --fail --location --retry 3 --output "/tmp/${TOOLCHAIN_ASSET}" "${url}"; \
        echo "${TOOLCHAIN_SHA256}  /tmp/${TOOLCHAIN_ASSET}" | sha256sum --check -; \
        mkdir -p /opt/toolchains; \
        tar -xf "/tmp/${TOOLCHAIN_ASSET}" -C /opt/toolchains; \
        rm "/tmp/${TOOLCHAIN_ASSET}"; \
        root="/opt/toolchains/${TOOLCHAIN_DIR}"; \
        for tool in gcc g++ ar ranlib strip; do \
            test -x "${root}/bin/${CROSS_PREFIX}-${tool}"; \
            ln -s "${root}/bin/${CROSS_PREFIX}-${tool}" "/usr/local/bin/${CROSS_PREFIX}-${tool}"; \
        done; \
    else \
        for tool in gcc g++ ar ranlib strip; do \
            test -x "/usr/bin/${tool}"; \
            ln -s "/usr/bin/${tool}" "/usr/local/bin/${CROSS_PREFIX}-${tool}"; \
        done; \
    fi

RUN set -eux; \
    sdk_root="/opt/xiaozhi-sdk/${TARGET}"; \
    source_root=/tmp/native-sources; \
    mkdir -p "${sdk_root}" "${source_root}"; \
    fetch() { \
        asset="$1"; checksum="$2"; \
        url="https://github.com/haoyn231/xiaozhi_linux_rs/releases/download/Source_Mirror/${asset}"; \
        curl --fail --location --retry 3 --output "${source_root}/${asset}" "${url}"; \
        echo "${checksum}  ${source_root}/${asset}" | sha256sum --check -; \
    }; \
    fetch "${ALSA_ASSET}" "${ALSA_SHA256}"; \
    fetch "${OPUS_ASSET}" "${OPUS_SHA256}"; \
    fetch "${SPEEXDSP_ASSET}" "${SPEEXDSP_SHA256}"; \
    tar -xf "${source_root}/${ALSA_ASSET}" -C "${source_root}"; \
    tar -xf "${source_root}/${OPUS_ASSET}" -C "${source_root}"; \
    tar -xf "${source_root}/${SPEEXDSP_ASSET}" -C "${source_root}"; \
    export CC="${CROSS_PREFIX}-gcc"; \
    export CXX="${CROSS_PREFIX}-g++"; \
    export AR="${CROSS_PREFIX}-ar"; \
    export RANLIB="${CROSS_PREFIX}-ranlib"; \
    export STRIP="${CROSS_PREFIX}-strip"; \
    export CFLAGS="-fPIC"; \
    export CXXFLAGS="-fPIC"; \
    cd "${source_root}/alsa-lib-1.2.12"; \
    ./configure --host="${CROSS_PREFIX}" --prefix=/usr \
        --enable-shared --disable-static --disable-python --disable-alisp \
        --disable-old-symbols --disable-topology; \
    make -j"$(nproc)"; \
    make DESTDIR="${sdk_root}" install; \
    cd "${source_root}/opus-1.5.2"; \
    ./configure --host="${CROSS_PREFIX}" --prefix=/usr \
        --enable-static --disable-shared --disable-doc --disable-extra-programs; \
    make -j"$(nproc)"; \
    make DESTDIR="${sdk_root}" install; \
    cd "${source_root}/speexdsp-1.2.1"; \
    ./configure --host="${CROSS_PREFIX}" --prefix=/usr --enable-static --disable-shared; \
    make -j"$(nproc)"; \
    make DESTDIR="${sdk_root}" install; \
    rm -rf "${source_root}"; \
    test -f "${sdk_root}/usr/lib/libopus.a"; \
    test -f "${sdk_root}/usr/lib/libspeexdsp.a"; \
    test -e "${sdk_root}/usr/lib/libasound.so"

ENV XIAOZHI_SDK_ROOT="/opt/xiaozhi-sdk/${TARGET}"
ENV PKG_CONFIG_ALLOW_CROSS=1
ENV PKG_CONFIG_SYSROOT_DIR="/opt/xiaozhi-sdk/${TARGET}"
ENV PKG_CONFIG_LIBDIR="/opt/xiaozhi-sdk/${TARGET}/usr/lib/pkgconfig:/opt/xiaozhi-sdk/${TARGET}/usr/share/pkgconfig"
ENV LIBOPUS_STATIC=1
ENV SPEEXDSP_STATIC=1
