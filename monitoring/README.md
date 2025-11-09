# Monitoring Setup for LLM Auto-Optimizer

This directory contains monitoring and observability configuration for the distributed state backends.

## Services

### Redis Commander
- **Purpose**: Web UI for Redis
- **URL**: http://localhost:8081
- **Credentials**: admin / admin
- **Features**:
  - Browse keys
  - View/edit values
  - Execute commands
  - Monitor memory usage
  - View slow log

### pgAdmin
- **Purpose**: Web UI for PostgreSQL
- **URL**: http://localhost:5050
- **Credentials**: admin@llmdevops.dev / admin
- **Features**:
  - Query editor
  - Database management
  - Performance monitoring
  - Visual explain plans
  - Backup/restore

### Prometheus
- **Purpose**: Metrics collection
- **URL**: http://localhost:9090
- **Features**:
  - Time-series database
  - PromQL query language
  - Metrics scraping
  - Alerting (when configured)

### Grafana
- **Purpose**: Metrics visualization
- **URL**: http://localhost:3000
- **Credentials**: admin / admin
- **Features**:
  - Dashboard creation
  - Alerts
  - Multiple datasources
  - Visualizations

## Quick Start

### Start All Services

```bash
# Start core services + monitoring
docker-compose up -d

# Verify all services are running
docker-compose ps

# View logs
docker-compose logs -f
```

### Start Only Core Services

```bash
# Start just Redis and PostgreSQL
docker-compose up -d redis postgres

# Start monitoring later
docker-compose up -d redis-commander pgadmin prometheus grafana
```

### Stop Services

```bash
# Stop all services
docker-compose down

# Stop and remove volumes (clean slate)
docker-compose down -v
```

## Accessing Services

### Redis Commander

1. Open http://localhost:8081
2. Login with admin / admin
3. Browse keys using the left sidebar
4. Execute Redis commands in the CLI tab

**Useful Features**:
- View all keys matching a pattern
- Inspect key TTL
- Delete keys
- View memory usage by key pattern

### pgAdmin

1. Open http://localhost:5050
2. Login with admin@llmdevops.dev / admin
3. Right-click "Servers" → "Register" → "Server"
4. Configure:
   - General Tab:
     - Name: LLM Optimizer
   - Connection Tab:
     - Host: postgres
     - Port: 5432
     - Database: optimizer
     - Username: optimizer
     - Password: password
5. Click "Save"

**Useful Queries**:

```sql
-- View all state tables
SELECT tablename FROM pg_tables WHERE schemaname = 'public';

-- Count keys in state table
SELECT COUNT(*) FROM processor_state;

-- View recent entries
SELECT * FROM processor_state ORDER BY created_at DESC LIMIT 10;

-- Check table size
SELECT
    pg_size_pretty(pg_total_relation_size('processor_state')) as total_size,
    pg_size_pretty(pg_relation_size('processor_state')) as table_size,
    pg_size_pretty(pg_indexes_size('processor_state')) as indexes_size;

-- View dead tuples
SELECT relname, n_dead_tup, n_live_tup,
       round(n_dead_tup * 100.0 / NULLIF(n_live_tup + n_dead_tup, 0), 2) as dead_pct
FROM pg_stat_user_tables
WHERE n_dead_tup > 0
ORDER BY n_dead_tup DESC;
```

### Prometheus

1. Open http://localhost:9090
2. Go to "Status" → "Targets" to see scraped endpoints
3. Use the query interface to explore metrics

**Useful Queries**:

```promql
# Redis memory usage
redis_memory_used_bytes

# Redis connected clients
redis_connected_clients

# Redis commands per second
rate(redis_commands_processed_total[1m])

# Redis hit rate
rate(redis_keyspace_hits_total[1m]) /
(rate(redis_keyspace_hits_total[1m]) + rate(redis_keyspace_misses_total[1m]))

# PostgreSQL connections
pg_stat_database_numbackends{datname="optimizer"}

# PostgreSQL transaction rate
rate(pg_stat_database_xact_commit{datname="optimizer"}[1m])

# PostgreSQL database size
pg_database_size_bytes{datname="optimizer"}
```

### Grafana

1. Open http://localhost:3000
2. Login with admin / admin
3. Navigate to "Dashboards"
4. Create new dashboard or import existing ones

**Importing Dashboards**:

1. Click "+" → "Import"
2. Enter dashboard ID:
   - Redis: 11835 (Redis Dashboard for Prometheus Redis Exporter)
   - PostgreSQL: 9628 (PostgreSQL Database)
3. Select "Prometheus" as datasource
4. Click "Import"

**Creating Custom Dashboard**:

Example panels:
- Redis hit rate (gauge)
- PostgreSQL connections (graph)
- Redis memory usage (graph)
- Query duration (histogram)

## Metrics Exporters

### Redis Exporter

**Endpoint**: http://localhost:9121/metrics

**Key Metrics**:
- `redis_memory_used_bytes` - Memory usage
- `redis_connected_clients` - Active connections
- `redis_keyspace_hits_total` - Cache hits
- `redis_keyspace_misses_total` - Cache misses
- `redis_commands_processed_total` - Total commands
- `redis_db_keys` - Number of keys per database

### PostgreSQL Exporter

**Endpoint**: http://localhost:9187/metrics

**Key Metrics**:
- `pg_database_size_bytes` - Database size
- `pg_stat_database_numbackends` - Active connections
- `pg_stat_database_xact_commit` - Committed transactions
- `pg_stat_database_xact_rollback` - Rolled back transactions
- `pg_stat_user_tables_n_dead_tup` - Dead tuples
- `pg_stat_user_tables_seq_scan` - Sequential scans

## Alerting (Optional)

To enable alerting, create alert rules in Prometheus:

```yaml
# monitoring/alerts/state_backends.yml
groups:
  - name: state_backends
    interval: 30s
    rules:
      - alert: RedisHighMemoryUsage
        expr: redis_memory_used_bytes / redis_memory_max_bytes > 0.9
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "Redis memory usage is above 90%"

      - alert: RedisDown
        expr: up{job="redis"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Redis is down"

      - alert: PostgreSQLHighConnections
        expr: pg_stat_database_numbackends / pg_settings_max_connections > 0.8
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "PostgreSQL connections above 80%"

      - alert: PostgreSQLDown
        expr: up{job="postgres"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "PostgreSQL is down"
```

Then update `prometheus.yml` to load the rules:

```yaml
rule_files:
  - "alerts/*.yml"
```

## Troubleshooting

### Service Won't Start

```bash
# Check logs
docker-compose logs [service-name]

# Common issues:
# 1. Port already in use
docker-compose down
sudo lsof -i :[port]  # Find process using port
kill [pid]  # Kill the process

# 2. Volume permissions
docker-compose down -v  # Remove volumes
docker-compose up -d    # Recreate
```

### Can't Access Web UI

```bash
# Check if service is running
docker-compose ps

# Check if port is exposed
docker-compose port [service-name] [internal-port]

# Check firewall
sudo ufw status

# Test connection
curl http://localhost:[port]
```

### Metrics Not Appearing

```bash
# Check Prometheus targets
curl http://localhost:9090/api/v1/targets

# Check exporter is running
curl http://localhost:9121/metrics  # Redis
curl http://localhost:9187/metrics  # PostgreSQL

# Check Prometheus config
docker-compose exec prometheus cat /etc/prometheus/prometheus.yml
```

## Cleanup

```bash
# Remove all containers and volumes
docker-compose down -v

# Remove monitoring data only
docker volume rm llm-auto-optimizer_prometheus-data
docker volume rm llm-auto-optimizer_grafana-data
docker volume rm llm-auto-optimizer_pgadmin-data
```

## Production Recommendations

1. **Security**:
   - Change default passwords
   - Enable TLS/SSL
   - Use environment variables for secrets
   - Restrict network access

2. **Persistence**:
   - Configure volume backups
   - Set up retention policies
   - Enable Redis persistence (AOF/RDB)
   - Configure PostgreSQL backups

3. **High Availability**:
   - Use Redis Sentinel/Cluster
   - Set up PostgreSQL replication
   - Configure health checks
   - Implement auto-restart policies

4. **Monitoring**:
   - Set up alerting
   - Configure log aggregation
   - Enable metrics export
   - Create runbooks for alerts

5. **Performance**:
   - Tune Prometheus retention
   - Optimize Grafana queries
   - Configure exporter intervals
   - Set appropriate scrape intervals

## Additional Resources

- [Redis Monitoring](https://redis.io/docs/management/optimization/)
- [PostgreSQL Monitoring](https://www.postgresql.org/docs/current/monitoring.html)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [Grafana Documentation](https://grafana.com/docs/)
- [Redis Exporter](https://github.com/oliver006/redis_exporter)
- [PostgreSQL Exporter](https://github.com/prometheus-community/postgres_exporter)
