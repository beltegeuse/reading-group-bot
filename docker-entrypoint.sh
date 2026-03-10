#!/bin/sh
set -eu

if [ -z "${ROCKET_SECRET_KEY:-}" ]; then
  export ROCKET_SECRET_KEY="$(openssl rand -base64 32)"
  echo "ROCKET_SECRET_KEY not provided; generated ephemeral key for this container start."
fi

exec /app/reading-group-bot
