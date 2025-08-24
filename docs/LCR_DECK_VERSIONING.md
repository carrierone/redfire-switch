# LCR Rate Deck Versioning & Time-Based Management

## Overview

The Redfire Switch LCR engine now supports comprehensive rate deck versioning with automatic cutover management. This allows carriers to:

- Load future rate decks ahead of time
- Automatic version management and end-date setting
- Lazy preloading before cutover time
- Zero-downtime rate transitions
- Historical rate tracking and auditing

## Key Features

### 1. Automatic Versioning

When loading a new deck with the same name for the same vendor/client:
- System automatically increments the `deck_version`
- Previous version's `end_date` is set to 1 second before new deck's `effective_date`
- No manual version tracking required

### 2. Effective Date & Time Control

Rate decks support precise timing:
- **effective_date**: Date and time when deck becomes active
- **effective_time**: Time of day for cutover (default: 00:00:00 GMT)
- **end_date**: Automatically managed or manually set
- Support for non-standard cutover times

### 3. Lazy Loading & Preloading

System automatically manages deck loading:
- **preload_minutes**: Configure how early to load deck into cache (default: 30)
- Background task monitors upcoming cutovers
- Decks are preloaded into memory before effective time
- Seamless cutover without service interruption

## Database Schema Changes

### New Columns

```sql
-- Rate deck tables now include:
deck_version INTEGER NOT NULL DEFAULT 1
end_date TIMESTAMP WITH TIME ZONE
parent_deck_id INTEGER  -- References previous version
effective_time TIME DEFAULT '00:00:00'
preload_minutes INTEGER DEFAULT 30
loaded_at TIMESTAMP WITH TIME ZONE
is_staged BOOLEAN DEFAULT false
```

### New Tables

```sql
-- Track deck loading history
deck_load_history

-- Manage cutover scheduling
deck_cutover_schedule
```

## CLI Usage

### Load Future Vendor Deck

```bash
# Load deck effective at midnight GMT on Jan 15, 2025
./lcr_cli load-vendor \
  --name "Carrier-A-NANPA" \
  --vendor-id 1 \
  --rate-type DNIS \
  --effective-date "2025-01-15" \
  --csv-file rates.csv

# Load deck with custom time (3 PM GMT)
./lcr_cli load-vendor \
  --name "Carrier-A-NANPA" \
  --vendor-id 1 \
  --effective-date "2025-01-15" \
  --effective-time "15:00:00" \
  --preload-minutes 60 \
  --csv-file rates.csv
```

### View Deck Versions

```bash
# Show all versions of a deck
./lcr_cli show-versions \
  --name "Carrier-A-NANPA" \
  --owner-id 1 \
  --deck-type vendor

# Output:
Version | Effective Date        | End Date             | Status
v3      | 2025-01-15 00:00:00  | Never                | FUTURE
v2      | 2025-01-01 00:00:00  | 2025-01-14 23:59:59  | ACTIVE
v1      | 2024-12-01 00:00:00  | 2024-12-31 23:59:59  | EXPIRED
```

### Monitor Cutovers

```bash
# Show upcoming cutovers in next 48 hours
./lcr_cli show-cutovers --hours 48

# Force preload a deck
./lcr_cli preload-deck --deck-id 123 --deck-type vendor

# Cancel scheduled cutover
./lcr_cli cancel-cutover --schedule-id 45
```

### Test Routing at Future Time

```bash
# Test what routing would be at specific time
./lcr_cli test-routing \
  --ani 12125551234 \
  --dnis 14155555678 \
  --trunk-id 1 \
  --at-time "2025-01-15 00:00:00" \
  --compare  # Compare with current routing
```

## API Usage

### Load Vendor Deck

```bash
curl -X POST http://localhost:8080/decks/vendor \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Carrier-A-NANPA",
    "owner_id": 1,
    "rate_type": "DNIS",
    "effective_date": "2025-01-15T00:00:00Z",
    "effective_time": "00:00:00",
    "preload_minutes": 30,
    "rates": [
      {
        "code": "1212",
        "inter_rate": 0.0035,
        "intra_rate": 0.0040,
        "ij_rate": 0.0038,
        "local_rate": 0.0020
      }
    ]
  }'
```

### Get Deck Versions

```bash
# Get all versions of a deck
curl http://localhost:8080/decks/vendor/123/versions

# Response:
{
  "versions": [
    {
      "id": 123,
      "version": 3,
      "effective_date": "2025-01-15T00:00:00Z",
      "end_date": null,
      "status": "future",
      "rate_count": 50000
    },
    {
      "id": 122,
      "version": 2,
      "effective_date": "2025-01-01T00:00:00Z",
      "end_date": "2025-01-14T23:59:59Z",
      "status": "active",
      "rate_count": 49500
    }
  ]
}
```

### View Upcoming Cutovers

```bash
curl http://localhost:8080/decks/cutovers?hours=24

# Response:
{
  "schedules": [
    {
      "id": 1,
      "deck_type": "vendor",
      "current_deck_id": 122,
      "new_deck_id": 123,
      "cutover_date": "2025-01-15T00:00:00Z",
      "preload_at": "2025-01-14T23:30:00Z",
      "status": "scheduled"
    }
  ]
}
```

### Test Routing at Specific Time

```bash
curl -X POST http://localhost:8080/decks/test-routing \
  -H "Content-Type: application/json" \
  -d '{
    "ani": "12125551234",
    "dnis": "14155555678",
    "trunk_id": 1,
    "test_time": "2025-01-15T00:00:00Z",
    "compare_with_current": true
  }'
```

## Background Processing

### Deck Manager Task

A background task runs every minute to:
1. Check for decks that need preloading
2. Preload decks approaching their cutover time
3. Activate decks that have reached effective date
4. Clean up expired cache entries

### Database Notifications

PostgreSQL NOTIFY/LISTEN used for:
- Real-time deck change notifications
- Cluster synchronization
- Cache invalidation

## Best Practices

### 1. Loading Schedule

- Load new decks at least 24 hours before effective date
- Use 30-60 minute preload window for large decks
- Schedule major updates during low-traffic periods

### 2. Version Management

- Always load new version rather than updating existing
- Keep historical versions for audit trail
- Use descriptive deck names with date suffixes

### 3. Testing

- Always test routing at future time before cutover
- Compare current vs future routing
- Verify profit margins maintained

### 4. Monitoring

- Monitor deck loading status
- Set alerts for failed preloads
- Track cutover completion

## Example Workflow

### Typical Monthly Rate Update

1. **Day -3**: Receive new rates from vendor
```bash
./lcr_cli load-vendor \
  --name "Vendor-A-Jan2025" \
  --vendor-id 1 \
  --effective-date "2025-01-01" \
  --csv-file vendor_a_jan_rates.csv
```

2. **Day -2**: Verify deck loaded correctly
```bash
./lcr_cli show-versions --name "Vendor-A-Jan2025" --owner-id 1 --deck-type vendor
```

3. **Day -1**: Test routing changes
```bash
./lcr_cli test-routing \
  --ani 12125551234 \
  --dnis 14155555678 \
  --trunk-id 1 \
  --at-time "2025-01-01 00:00:00" \
  --compare
```

4. **Day 0**: Monitor cutover
```bash
./lcr_cli show-cutovers --hours 1
# System automatically handles cutover at midnight
```

5. **Day +1**: Verify successful transition
```bash
# Check active deck
curl http://localhost:8080/decks/vendor/124

# Verify routing using new rates
./lcr_cli simulate --ani 12125551234 --dnis 14155555678
```

## Troubleshooting

### Deck Not Loading

- Check `deck_cutover_schedule` status
- Verify `preload_at` time hasn't passed
- Check system logs for errors

### Wrong Rates Applied

- Verify effective_date and end_date ranges
- Check deck_version ordering
- Ensure cache reloaded after changes

### Performance Issues

- Increase preload_minutes for large decks
- Monitor cache memory usage
- Consider splitting very large decks

## Migration from Legacy System

For existing deployments:

1. Run migration script:
```bash
psql -d lcr_db -f migrations/lcr_deck_versioning.sql
```

2. Existing decks get version 1
3. End dates remain NULL (never expire)
4. New loads will increment versions

## API Rate Limiting

When loading decks via API:
- Max 10 deck loads per hour per vendor
- Max 100,000 rates per deck load
- Use batch loading for large updates