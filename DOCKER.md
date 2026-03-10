# Docker Setup for Reading Group Bot

This project includes a lightweight Docker setup using Alpine Linux for deployment.

## Quick Start

### Build and Run with Docker Compose (Recommended)

```bash
docker-compose up --build
```

The service will be available at `http://localhost:3001`

### Manual Docker Build and Run

**Build the image:**
```bash
docker build -t reading-group-bot:latest .
```

**Run the container:**
```bash
docker run -d \
  --name reading-group-bot \
  -p 3001:3001 \
  -v reading-group-bot-db:/app/data \
  -v reading-group-bot-pdfs:/app/static/pdfs \
  -v reading-group-bot-thumbnails:/app/static/thumbnails \
  reading-group-bot:latest
```

## Image Details

- **Base Image**: Alpine Linux (lightweight, ~5MB)
- **Total Image Size**: ~200-300MB (depends on build)
- **Multi-stage Build**: Reduces final image size by building in a temporary builder stage

### Directories

- `/app/data/db.sqlite` - SQLite database (persisted via volume)
- `/app/static/pdfs` - PDF files storage
- `/app/static/thumbnails` - Thumbnail images storage
- `/app/templates` - Tera templates
- `/app/migrations` - Database migrations

## Volumes

Three named volumes are created for persistence:

1. `reading-group-bot-db` - Database file
2. `reading-group-bot-pdfs` - PDF storage
3. `reading-group-bot-thumbnails` - Thumbnail storage

## Environment Variables

Custom environment variables can be passed to the container:

```bash
docker run -e ROCKET_ENV=production -e ROCKET_LOG_LEVEL=normal ...
```

Available variables (defaults in Rocket.toml):
- `ROCKET_ADDRESS` - Server address (default: 0.0.0.0)
- `ROCKET_PROFILE` - Rocket profile (for release containers, set to `release`)
- `ROCKET_PORT` - Server port (set to `3001` in this setup)
- `ROCKET_LOG_LEVEL` - Logging level (default: normal)

`ROCKET_SECRET_KEY` is required by Rocket in production mode when `secrets` is enabled.
If not provided, the container entrypoint generates an ephemeral key at startup.
For stable sessions across restarts, set `ROCKET_SECRET_KEY` explicitly.

## Health Checks

The container includes a health check that verifies the server is responding.

## Production Considerations

1. **Database Backup**: Mount the db volume to a backed-up location
2. **Reverse Proxy**: Use nginx or similar to handle SSL/TLS
3. **Resource Limits**: Set CPU and memory limits in docker-compose or docker run
4. **Logging**: Consider mounting a logging volume or using Docker logging drivers

Example with resource limits:

```yaml
services:
  reading-group-bot:
    deploy:
      resources:
        limits:
          cpus: '1'
          memory: 512M
        reservations:
          cpus: '0.5'
          memory: 256M
```

## Cleanup

Remove containers and volumes:

```bash
docker-compose down -v  # -v removes volumes
```

Or manually:

```bash
docker stop reading-group-bot
docker rm reading-group-bot
docker volume rm reading-group-bot-db reading-group-bot-pdfs reading-group-bot-thumbnails
```

## Troubleshooting

### View logs:
```bash
docker-compose logs -f reading-group-bot
```

### Access container shell:
```bash
docker exec -it reading-group-bot /bin/sh
```

### Check container status:
```bash
docker ps -a
```
