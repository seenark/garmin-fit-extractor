#!/usr/bin/env bash
set -Eeuo pipefail

if ! command -v docker >/dev/null 2>&1; then
  printf '%s\n' 'docker is required for the PostgreSQL integration harness' >&2
  exit 1
fi

container="garmin-pg-test-${RANDOM}-${RANDOM}"
volume="${container}-data"
db="garmin_fit_test"
user="garmin_fit_test"
password="$(od -An -N24 -tx1 /dev/urandom | tr -d ' \n')"

cleanup() {
  docker rm --force "$container" >/dev/null 2>&1 || true
  docker volume rm "$volume" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker volume create "$volume" >/dev/null
docker run --detach --name "$container" \
  --publish 127.0.0.1::5432 \
  --volume "$volume:/var/lib/postgresql" \
  --env POSTGRES_DB="$db" \
  --env POSTGRES_USER="$user" \
  --env POSTGRES_PASSWORD="$password" \
  postgres:18 >/dev/null

for _ in $(seq 1 90); do
  if docker exec "$container" pg_isready -U "$user" -d "$db" >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

docker exec "$container" pg_isready -U "$user" -d "$db" >/dev/null
host_port="$(docker port "$container" 5432/tcp | cut -d: -f2)"
database_url="postgresql://${user}:${password}@127.0.0.1:${host_port}/${db}"

printf '%s\n' 'Preparing PostgreSQL migrations'
cargo run --locked -p legacy-data-migrator -- prepare-target --database-url "$database_url"

printf '%s\n' 'Running PostgreSQL API tests'
TEST_DATABASE_URL="$database_url" DATABASE_URL="$database_url" \
  cargo test --locked -p garmin-fit-extractor-api --tests -- --test-threads=1

printf '%s\n' 'Running migrator integration tests'
TEST_DATABASE_URL="$database_url" DATABASE_URL="$database_url" \
  cargo test --locked -p legacy-data-migrator --tests -- --test-threads=1
