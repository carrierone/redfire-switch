#!/bin/bash

# Setup test database for RedFire Switch
set -e

echo "Setting up test database for RedFire Switch..."

# Stop and remove existing container if it exists
docker stop redfire_test_db 2>/dev/null || true
docker rm redfire_test_db 2>/dev/null || true

# Start PostgreSQL container
echo "Starting PostgreSQL container..."
docker run -d \
  --name redfire_test_db \
  -e POSTGRES_PASSWORD=test \
  -e POSTGRES_USER=test \
  -e POSTGRES_DB=redfire_test \
  -p 5432:5432 \
  postgres:15

# Wait for database to be ready
echo "Waiting for database to be ready..."
sleep 10

# Check if database is ready
for i in {1..30}; do
  if docker exec redfire_test_db pg_isready -U test -d redfire_test; then
    echo "Database is ready!"
    break
  fi
  echo "Waiting for database... ($i/30)"
  sleep 1
done

# Run migrations
echo "Running database migrations..."
docker exec -i redfire_test_db psql -U test -d redfire_test < migrations/001_initial_schema.sql

echo "Test database setup complete!"
echo "Database URL: postgresql://test:test@localhost:5432/redfire_test"