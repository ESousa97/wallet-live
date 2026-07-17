# Build em dois estágios: o builder compila com a toolchain completa; a imagem
# final leva só o binário (templates e migrações já vão embutidos nele pelo
# askama e pelo sqlx::migrate!). SQLX_OFFLINE usa o cache .sqlx versionado, então
# o build não precisa de um banco de dados.

FROM rust:1.95-slim AS builder
WORKDIR /app

# reqwest (native-tls) precisa do OpenSSL para compilar; ca-certificates para o
# update-ca-certificates logo abaixo.
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# CAs extras OPCIONAIS para o build (ambientes atrás de proxy corporativo ou
# antivírus com inspeção TLS, que reassinam o tráfego com um CA próprio — sem
# confiá-lo, o cargo não baixa dependências). Coloque o(s) .crt (PEM) em
# docker/extra-ca/ — eles ficam fora do git (ver .gitignore) por serem
# específicos de cada máquina. Sem certificados, o passo é um no-op.
COPY docker/extra-ca/ /usr/local/share/ca-certificates/extra/
RUN update-ca-certificates

COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

# ca-certificates + libssl3 para o TLS das cotações; curl para o healthcheck.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/*

# Processo sem privilégios: um eventual comprometimento não ganha root.
RUN useradd --system --uid 10001 wallet
USER wallet

COPY --from=builder /app/target/release/wallet /usr/local/bin/wallet

ENV BIND_ADDR=0.0.0.0:3000
EXPOSE 3000

ENTRYPOINT ["wallet"]
