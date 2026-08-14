# Build the static site, then serve it from a tiny Caddy image.
# The cluster's Caddy terminates TLS and routes by hostname, so this one
# only ever speaks plain HTTP on :80.

FROM node:22-alpine AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM caddy:2-alpine
COPY Caddyfile /etc/caddy/Caddyfile
COPY --from=build /app/dist /srv
EXPOSE 80
