# Развёртывание сервера

Сервер (`server/`) и Caddy для HTTPS запускаются через Docker Compose. Спецификация сервера: `server-doc.md`.

## Что нужно

- VPS с Linux, Docker и плагином Compose. Память: не меньше 2,5 ГБ при `MAX_PARALLEL_PARSES=1` (один разбор полного матча даёт пик около 0,9 ГБ), около 3,5 ГБ при двух. Данные по замерам: `docs/measurements.md`.
- Диск: демка весит 60-410 МБ, результат 1-4 МБ; пока демка разбирается или если `KEEP_DEM=true`, она лежит на диске.
- Домен, A/AAAA-запись которого указывает на VPS. Порты 80 и 443 открыты (Caddy получает сертификат сам; порт 80 нужен для проверки домена).
- Сборка образа требует сети (Cargo и генерация protobuf-кода из demoparser2) и занимает несколько минут.

## Запуск

```bash
cd deploy
cp .env.example .env
# в .env: SITE_ADDRESS (домен) и API_TOKEN (openssl rand -hex 32)
docker compose up -d --build
docker compose ps          # server должен стать healthy
curl https://ВАШ_ДОМЕН/health
```

Проверка загрузки (токен тот же, что в `.env`):

```bash
curl -H "Authorization: Bearer $API_TOKEN" -H "Content-Type: application/octet-stream" \
     -H "X-File-Name: match.dem" --data-binary @match.dem https://ВАШ_ДОМЕН/matches
curl -H "Authorization: Bearer $API_TOKEN" https://ВАШ_ДОМЕН/matches/ID
```

## Обновление

```bash
git pull
cd deploy && docker compose up -d --build
```

Данные лежат в томе `data` и переживают обновление. Если сервер остановили во время разбора, матч доразбирается при следующем запуске (при сохранённой демке).

## Что где

- `Dockerfile` (в корне): многоступенчатая сборка; итоговый образ на `debian:bookworm-slim`, процесс идёт не от root, проверка живости `server healthcheck` (в образе нет curl).
- `deploy/docker-compose.yml`: сервер (наружу порт не публикуется, виден только Caddy) и Caddy. Лимит памяти контейнера `SERVER_MEMORY_LIMIT`, время на остановку 60 с (идущий разбор успевает закончиться).
- `deploy/Caddyfile`: HTTPS, лимит тела запроса 1100 МБ (чуть больше `MAX_UPLOAD_BYTES`), таймаут чтения тела 30 минут.
- `deploy/.env.example`: все переменные. Описание каждой: `server-doc.md`, разд. 12.

Образец `PROTO_COMMIT` в `Dockerfile` фиксирует версию protobuf-определений Steam, с которой проверен парсер: сборка demoparser2 иначе берёт самую свежую и результат зависел бы от дня сборки. Менять его нужно вместе с обновлением demoparser2.

## Резервные копии и безопасность

- Резервные копии не нужны: всё восстанавливается из демок. Единственный секрет это `API_TOKEN` в `.env`; файл не коммитить.
- Токен отправляется в заголовке, поэтому только HTTPS; порт сервера (8080) наружу не открывать.
- Посмотреть логи: `docker compose logs -f server`.

## Локальная проверка без домена

```bash
cd deploy
SITE_ADDRESS=http://localhost HTTP_PORT=18080 HTTPS_PORT=18443 API_TOKEN=$(openssl rand -hex 16) docker compose up -d --build
curl http://localhost:18080/health
docker compose down -v
```
