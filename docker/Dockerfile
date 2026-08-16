# Imagem publica que serve o instalador do NukaBoost, no mesmo desenho do
# `arrowremove` ja em producao no homelab: nginx somente leitura, sem
# volume, sem porta publicada no host, conteudo embutido na propria
# imagem. Ver docker/nginx.conf para o contrato de rotas/headers e
# tmp/relatorio.txt para o contrato completo com o lado do homelab.
#
# Build (contexto = este diretorio; NukaBoost.msi precisa existir aqui
# antes do build -- ver docs/RELEASE.md):
#
#   docker build -t nukaboost-installer docker
#
# O ENTRYPOINT abaixo ignora deliberadamente o entrypoint padrao da
# imagem base (que tenta ajustar permissoes/templates em runtime): tudo
# que ele faria ja foi resolvido em build time, e um filesystem raiz
# somente leitura nao deixaria esses ajustes acontecerem de qualquer
# forma.
FROM nginx:1.27-alpine

COPY nginx.conf /etc/nginx/nginx.conf
COPY install.ps1 /usr/share/nginx/html/install.ps1
COPY NukaBoost.msi /usr/share/nginx/html/NukaBoost.msi

RUN chown -R nginx:nginx /usr/share/nginx/html /etc/nginx/nginx.conf && \
    chmod -R a-w /usr/share/nginx/html

USER nginx
EXPOSE 8080

# O compose do homelab precisa montar um tmpfs em /tmp (root filesystem
# read_only: true) -- e onde nginx.conf redireciona pid, logs e buffers
# temporarios, ja que /var/cache e /var/run nao sao graváveis aqui.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q -O- http://127.0.0.1:8080/healthz | grep -q ok || exit 1

ENTRYPOINT ["nginx", "-g", "daemon off;"]
