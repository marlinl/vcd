ARG BASE_IMAGE=debian:trixie-slim
FROM ${BASE_IMAGE}

ARG USERNAME=user
ARG GIT_USERNAME=user
ARG GIT_EMAIL=user@example.com
ARG SSH_KEY=id_rsa
ARG CLAUDE_CODE_PACKAGE=@anthropic-ai/claude-code
ARG CODEX_PACKAGE=@openai/codex
ARG VCD_PROXY_URL=http://host.docker.internal:1087
ARG VCD_NO_PROXY=localhost,127.0.0.1,::1,host.docker.internal,.local

ENV DEBIAN_FRONTEND=noninteractive
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8
ENV TZ=Asia/Shanghai
ENV SHELL=/bin/zsh
ENV HTTP_PROXY=${VCD_PROXY_URL}
ENV HTTPS_PROXY=${VCD_PROXY_URL}
ENV ALL_PROXY=${VCD_PROXY_URL}
ENV http_proxy=${VCD_PROXY_URL}
ENV https_proxy=${VCD_PROXY_URL}
ENV all_proxy=${VCD_PROXY_URL}
ENV NO_PROXY=${VCD_NO_PROXY}
ENV no_proxy=${VCD_NO_PROXY}

COPY optional-packages.txt /tmp/optional-packages.txt

RUN printf '%s\n' \
        'Types: deb' \
        'URIs: http://mirrors.ustc.edu.cn/debian' \
        'Suites: trixie trixie-updates' \
        'Components: main' \
        'Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg' \
        '' \
        'Types: deb' \
        'URIs: http://mirrors.ustc.edu.cn/debian-security' \
        'Suites: trixie-security' \
        'Components: main' \
        'Signed-By: /usr/share/keyrings/debian-archive-keyring.gpg' \
        > /etc/apt/sources.list.d/debian.sources \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        curl \
        git \
        gh \
        glab \
        locales \
        nodejs \
        npm \
        openjdk-21-jdk-headless \
        openssh-client \
        postgresql-client \
        python3 \
        python3-pip \
        python3-venv \
        sudo \
        tzdata \
        vim \
        wget \
        zsh \
    && ln -snf "/usr/share/zoneinfo/${TZ}" /etc/localtime \
    && echo "${TZ}" > /etc/timezone \
    && optional_packages="$(sed -n 's/^\[x\][[:space:]]\+\([^#[:space:]]\+\).*/\1/p' /tmp/optional-packages.txt)" \
    && if [ -n "${optional_packages}" ]; then apt-get install -y --no-install-recommends ${optional_packages}; fi \
    && npm install -g "${CLAUDE_CODE_PACKAGE}" "${CODEX_PACKAGE}" \
    && npm cache clean --force \
    && rm -rf /var/lib/apt/lists/* /tmp/optional-packages.txt

RUN set -eux; \
    useradd --user-group --create-home --shell /bin/zsh "${USERNAME}"; \
    echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" > "/etc/sudoers.d/${USERNAME}"; \
    chmod 0440 "/etc/sudoers.d/${USERNAME}"; \
    ln -s "/Users/${USERNAME}/.codex" "/home/${USERNAME}/.codex"; \
    ln -s "/Users/${USERNAME}/.claude" "/home/${USERNAME}/.claude"; \
    chown -h "${USERNAME}:${USERNAME}" "/home/${USERNAME}/.codex" "/home/${USERNAME}/.claude"

USER ${USERNAME}
WORKDIR /home/${USERNAME}

RUN set -eux; \
    mkdir -p "/home/${USERNAME}/.ssh"; \
    chmod 700 "/home/${USERNAME}/.ssh"; \
    git config --global user.name "${GIT_USERNAME}"; \
    git config --global user.email "${GIT_EMAIL}"; \
    git config --global core.sshCommand "ssh -i /home/${USERNAME}/.ssh/${SSH_KEY} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new"

CMD ["/bin/zsh"]
