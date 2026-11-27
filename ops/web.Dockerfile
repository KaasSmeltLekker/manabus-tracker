FROM node:20-slim AS build
WORKDIR /app
RUN corepack enable
COPY apps/web ./apps/web
COPY pnpm-workspace.yaml package.json pnpm-lock.yaml* ./
RUN pnpm install --filter ./apps/web
RUN pnpm --filter ./apps/web build

FROM nginx:alpine
COPY --from=build /app/apps/web/dist /usr/share/nginx/html
COPY ops/nginx-web.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
