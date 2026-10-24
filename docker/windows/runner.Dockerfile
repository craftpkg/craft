# Runs Windows executables under Wine, with the Windows builds of Node.js and Git on PATH.
FROM --platform=linux/amd64 ubuntu:24.04
ARG NODE_VERSION=22.20.0
ARG MINGIT_VERSION=2.51.0
ENV DEBIAN_FRONTEND=noninteractive WINEDEBUG=-all WINEPREFIX=/wine
RUN dpkg --add-architecture i386 \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl gnupg unzip xvfb \
    && mkdir -pm755 /etc/apt/keyrings \
    && curl -fsSL https://dl.winehq.org/wine-builds/winehq.key | gpg --dearmor -o /etc/apt/keyrings/winehq.gpg \
    && echo "deb [signed-by=/etc/apt/keyrings/winehq.gpg] https://dl.winehq.org/wine-builds/ubuntu/ noble main" > /etc/apt/sources.list.d/winehq.list \
    && apt-get update \
    && apt-get install -y --install-recommends winehq-staging \
    && rm -rf /var/lib/apt/lists/*
RUN wineboot --init && wineserver --wait \
    && curl -fsSL -o /tmp/node.zip https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-win-x64.zip \
    && unzip -q /tmp/node.zip -d /tmp/node \
    && mv /tmp/node/node-v${NODE_VERSION}-win-x64 "$WINEPREFIX/drive_c/node" \
    && curl -fsSL -o /tmp/mingit.zip https://github.com/git-for-windows/git/releases/download/v${MINGIT_VERSION}.windows.1/MinGit-${MINGIT_VERSION}-64-bit.zip \
    && unzip -q /tmp/mingit.zip -d "$WINEPREFIX/drive_c/git" \
    && rm -rf /tmp/node.zip /tmp/node /tmp/mingit.zip
ENV WINEPATH="C:\\node;C:\\git\\cmd"
WORKDIR /work
