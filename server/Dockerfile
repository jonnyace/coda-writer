# Self-hosted build of the Coda Writer server (Node + a SQLite file).
FROM node:24-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY tsconfig.json ./
COPY src ./src
RUN npm run build

FROM node:24-alpine
WORKDIR /app
COPY --from=build /app/dist/server.mjs ./server.mjs
ENV CODA_DATA_DIR=/data PORT=8787
# The server runs as the unprivileged "node" user, which must own its data.
RUN mkdir /data && chown node:node /data
VOLUME /data
EXPOSE 8787
USER node
CMD ["node", "server.mjs"]
