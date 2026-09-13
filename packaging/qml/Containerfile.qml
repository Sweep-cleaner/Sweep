# Sweep Qt6/QML shell over the sweep CLI (no cxx-qt, no Rust linkage).
#
#   podman build -t sweep-qml -f packaging/qml/Containerfile.qml .
#   podman run --rm -v "$PWD/out:/out" sweep-qml
#
# The container builds the QML app, runs it under Xvfb, and drops a
# screenshot at /out/shot.png. Needs the `sweep` image built first
# (packaging/podman/Containerfile) — the CLI is copied from it.

FROM docker.io/library/debian:bookworm-slim
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
        qt6-base-dev qt6-declarative-dev qt6-declarative-dev-tools \
        cmake g++ make pkg-config \
        xvfb scrot fonts-dejavu-core fonts-noto-color-emoji ca-certificates \
        qml6-module-qtquick qml6-module-qtquick-window qml6-module-qtquick-shapes \
        qml6-module-qtqml qml6-module-qtqml-models qml6-module-qtqml-workerscript qml6-module-qtcore \
    && rm -rf /var/lib/apt/lists
COPY --from=localhost/sweep:latest /usr/local/bin/sweep /usr/local/bin/sweep
COPY packaging/qml/CMakeLists.txt packaging/qml/main.cpp /src/
COPY packaging/qml/qml /src/qml
WORKDIR /src
RUN for f in qml/*.qml qml/themes/*.qml; do /usr/lib/qt6/bin/qmllint --unqualified info "$f" || exit 1; done
RUN cmake -B build -DCMAKE_BUILD_TYPE=Release . \
    && cmake --build build -j"$(nproc)" \
    && cp build/sweep-qml /app-sweep-qml
COPY packaging/qml/start.sh /start.sh
RUN chmod +x /start.sh
ENTRYPOINT ["/start.sh"]
