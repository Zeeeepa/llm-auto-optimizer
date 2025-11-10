# Distributed State Deployment Guide

Production deployment guide for Redis and PostgreSQL state backends with Docker, Kubernetes, and cloud providers.

## Table of Contents

1. [Docker Compose Deployments](#docker-compose-deployments)
2. [Kubernetes Deployments](#kubernetes-deployments)
3. [Cloud Provider Guides](#cloud-provider-guides)
4. [Security Hardening](#security-hardening)
5. [Capacity Planning](#capacity-planning)

---

## Docker Compose Deployments

### Redis Standalone

```yaml
# docker-compose.redis.yml
version: '3.8'

services:
  redis:
    image: redis:7.2-alpine
    container_name: optimizer-redis
    ports:
      - "6379:6379"
    volumes:
      - redis-data:/data
      - ./redis.conf:/usr/local/etc/redis/redis.conf
    command: redis-server /usr/local/etc/redis/redis.conf
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3

volumes:
  redis-data:
    driver: local
```

### Redis Sentinel (HA)

```yaml
# docker-compose.redis-sentinel.yml
version: '3.8'

services:
  redis-master:
    image: redis:7.2-alpine
    container_name: redis-master
    command: redis-server --appendonly yes
    volumes:
      - redis-master-data:/data
    networks:
      - redis-net

  redis-replica-1:
    image: redis:7.2-alpine
    container_name: redis-replica-1
    command: redis-server --replicaof redis-master 6379 --appendonly yes
    volumes:
      - redis-replica-1-data:/data
    networks:
      - redis-net
    depends_on:
      - redis-master

  redis-replica-2:
    image: redis:7.2-alpine
    container_name: redis-replica-2
    command: redis-server --replicaof redis-master 6379 --appendonly yes
    volumes:
      - redis-replica-2-data:/data
    networks:
      - redis-net
    depends_on:
      - redis-master

  sentinel-1:
    image: redis:7.2-alpine
    container_name: sentinel-1
    command: redis-sentinel /usr/local/etc/redis/sentinel.conf
    volumes:
      - ./sentinel.conf:/usr/local/etc/redis/sentinel.conf
    networks:
      - redis-net
    depends_on:
      - redis-master
    ports:
      - "26379:26379"

  sentinel-2:
    image: redis:7.2-alpine
    container_name: sentinel-2
    command: redis-sentinel /usr/local/etc/redis/sentinel.conf
    volumes:
      - ./sentinel.conf:/usr/local/etc/redis/sentinel.conf
    networks:
      - redis-net
    depends_on:
      - redis-master
    ports:
      - "26380:26379"

  sentinel-3:
    image: redis:7.2-alpine
    container_name: sentinel-3
    command: redis-sentinel /usr/local/etc/redis/sentinel.conf
    volumes:
      - ./sentinel.conf:/usr/local/etc/redis/sentinel.conf
    networks:
      - redis-net
    depends_on:
      - redis-master
    ports:
      - "26381:26379"

volumes:
  redis-master-data:
  redis-replica-1-data:
  redis-replica-2-data:

networks:
  redis-net:
    driver: bridge
```

**sentinel.conf:**

```conf
port 26379
sentinel monitor mymaster redis-master 6379 2
sentinel down-after-milliseconds mymaster 5000
sentinel parallel-syncs mymaster 1
sentinel failover-timeout mymaster 10000
```

### PostgreSQL with Replication

```yaml
# docker-compose.postgres.yml
version: '3.8'

services:
  postgres-primary:
    image: postgres:16-alpine
    container_name: postgres-primary
    environment:
      POSTGRES_DB: optimizer
      POSTGRES_USER: optimizer
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_INITDB_ARGS: "-E UTF8"
    volumes:
      - postgres-primary-data:/var/lib/postgresql/data
      - ./postgresql.conf:/etc/postgresql/postgresql.conf
      - ./pg_hba.conf:/etc/postgresql/pg_hba.conf
    command: postgres -c config_file=/etc/postgresql/postgresql.conf
    ports:
      - "5432:5432"
    networks:
      - pg-net
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U optimizer"]
      interval: 10s
      timeout: 5s
      retries: 5

  postgres-replica-1:
    image: postgres:16-alpine
    container_name: postgres-replica-1
    environment:
      POSTGRES_USER: optimizer
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
    volumes:
      - postgres-replica-1-data:/var/lib/postgresql/data
    command: |
      bash -c "
      until pg_basebackup -h postgres-primary -D /var/lib/postgresql/data -U replicator -v -P --wal-method=stream; do
        echo 'Waiting for primary to be available...'
        sleep 1s
      done
      echo 'standby_mode = on' > /var/lib/postgresql/data/standby.signal
      echo \"primary_conninfo = 'host=postgres-primary port=5432 user=replicator password=${REPLICATOR_PASSWORD}'\" >> /var/lib/postgresql/data/postgresql.auto.conf
      postgres
      "
    depends_on:
      - postgres-primary
    ports:
      - "5433:5432"
    networks:
      - pg-net

  pgbouncer:
    image: edoburu/pgbouncer:1.21.0
    container_name: pgbouncer
    environment:
      DATABASE_URL: "postgres://optimizer:${POSTGRES_PASSWORD}@postgres-primary:5432/optimizer"
      POOL_MODE: transaction
      MAX_CLIENT_CONN: 1000
      DEFAULT_POOL_SIZE: 20
    ports:
      - "6432:6432"
    networks:
      - pg-net
    depends_on:
      - postgres-primary

volumes:
  postgres-primary-data:
  postgres-replica-1-data:

networks:
  pg-net:
    driver: bridge
```

---

## Kubernetes Deployments

### Redis Cluster on Kubernetes

```yaml
# redis-cluster.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: redis-cluster-config
  namespace: optimizer
data:
  redis.conf: |
    cluster-enabled yes
    cluster-config-file nodes.conf
    cluster-node-timeout 5000
    appendonly yes
    appendfilename "appendonly.aof"
    appendfsync everysec
    maxmemory 4gb
    maxmemory-policy allkeys-lru

---
apiVersion: v1
kind: Service
metadata:
  name: redis-cluster
  namespace: optimizer
spec:
  type: ClusterIP
  ports:
  - port: 6379
    targetPort: 6379
    name: client
  - port: 16379
    targetPort: 16379
    name: gossip
  selector:
    app: redis-cluster

---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: redis-cluster
  namespace: optimizer
spec:
  serviceName: redis-cluster
  replicas: 6
  selector:
    matchLabels:
      app: redis-cluster
  template:
    metadata:
      labels:
        app: redis-cluster
    spec:
      containers:
      - name: redis
        image: redis:7.2-alpine
        ports:
        - containerPort: 6379
          name: client
        - containerPort: 16379
          name: gossip
        command: ["redis-server"]
        args: ["/conf/redis.conf"]
        volumeMounts:
        - name: conf
          mountPath: /conf
        - name: data
          mountPath: /data
        resources:
          requests:
            memory: "4Gi"
            cpu: "1"
          limits:
            memory: "8Gi"
            cpu: "2"
        livenessProbe:
          exec:
            command: ["redis-cli", "ping"]
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          exec:
            command: ["redis-cli", "ping"]
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: conf
        configMap:
          name: redis-cluster-config
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: "fast-ssd"
      resources:
        requests:
          storage: 100Gi

---
apiVersion: batch/v1
kind: Job
metadata:
  name: redis-cluster-init
  namespace: optimizer
spec:
  template:
    spec:
      containers:
      - name: init
        image: redis:7.2-alpine
        command:
        - sh
        - -c
        - |
          redis-cli --cluster create \
            redis-cluster-0.redis-cluster:6379 \
            redis-cluster-1.redis-cluster:6379 \
            redis-cluster-2.redis-cluster:6379 \
            redis-cluster-3.redis-cluster:6379 \
            redis-cluster-4.redis-cluster:6379 \
            redis-cluster-5.redis-cluster:6379 \
            --cluster-replicas 1 --cluster-yes
      restartPolicy: OnFailure
```

### PostgreSQL HA with Patroni

```yaml
# postgres-ha.yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: postgres-config
  namespace: optimizer
data:
  postgresql.conf: |
    max_connections = 200
    shared_buffers = 4GB
    effective_cache_size = 12GB
    maintenance_work_mem = 1GB
    work_mem = 64MB
    wal_level = replica
    max_wal_senders = 10
    wal_keep_size = 1GB

---
apiVersion: v1
kind: Service
metadata:
  name: postgres
  namespace: optimizer
spec:
  type: ClusterIP
  ports:
  - port: 5432
    targetPort: 5432
  selector:
    app: postgres
    role: master

---
apiVersion: v1
kind: Service
metadata:
  name: postgres-replica
  namespace: optimizer
spec:
  type: ClusterIP
  ports:
  - port: 5432
    targetPort: 5432
  selector:
    app: postgres
    role: replica

---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: postgres
  namespace: optimizer
spec:
  serviceName: postgres
  replicas: 3
  selector:
    matchLabels:
      app: postgres
  template:
    metadata:
      labels:
        app: postgres
    spec:
      containers:
      - name: postgres
        image: postgres:16-alpine
        ports:
        - containerPort: 5432
        env:
        - name: POSTGRES_DB
          value: "optimizer"
        - name: POSTGRES_USER
          valueFrom:
            secretKeyRef:
              name: postgres-secret
              key: username
        - name: POSTGRES_PASSWORD
          valueFrom:
            secretKeyRef:
              name: postgres-secret
              key: password
        - name: PGDATA
          value: /var/lib/postgresql/data/pgdata
        volumeMounts:
        - name: data
          mountPath: /var/lib/postgresql/data
        - name: config
          mountPath: /etc/postgresql
        resources:
          requests:
            memory: "8Gi"
            cpu: "2"
          limits:
            memory: "16Gi"
            cpu: "4"
        livenessProbe:
          exec:
            command: ["pg_isready", "-U", "optimizer"]
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          exec:
            command: ["pg_isready", "-U", "optimizer"]
          initialDelaySeconds: 5
          periodSeconds: 5
      volumes:
      - name: config
        configMap:
          name: postgres-config
  volumeClaimTemplates:
  - metadata:
      name: data
    spec:
      accessModes: ["ReadWriteOnce"]
      storageClassName: "fast-ssd"
      resources:
        requests:
          storage: 500Gi
```

---

## Cloud Provider Guides

### AWS Deployment

#### Amazon ElastiCache (Redis)

```hcl
# terraform/elasticache.tf
resource "aws_elasticache_replication_group" "optimizer" {
  replication_group_id       = "optimizer-redis"
  replication_group_description = "Redis cluster for LLM Optimizer"
  engine                     = "redis"
  engine_version             = "7.0"
  node_type                  = "cache.r6g.xlarge"
  number_cache_clusters      = 3
  parameter_group_name       = "default.redis7.cluster.on"
  port                       = 6379
  subnet_group_name          = aws_elasticache_subnet_group.optimizer.name
  security_group_ids         = [aws_security_group.redis.id]
  automatic_failover_enabled = true
  multi_az_enabled           = true
  at_rest_encryption_enabled = true
  transit_encryption_enabled = true
  snapshot_retention_limit   = 5
  snapshot_window            = "03:00-05:00"
  maintenance_window         = "sun:05:00-sun:07:00"

  tags = {
    Name        = "optimizer-redis"
    Environment = "production"
  }
}
```

#### Amazon RDS PostgreSQL

```hcl
# terraform/rds.tf
resource "aws_db_instance" "optimizer" {
  identifier             = "optimizer-postgres"
  engine                 = "postgres"
  engine_version         = "16.1"
  instance_class         = "db.r6g.2xlarge"
  allocated_storage      = 500
  storage_type           = "gp3"
  storage_encrypted      = true

  db_name  = "optimizer"
  username = "optimizer"
  password = random_password.db_password.result

  multi_az               = true
  backup_retention_period = 7
  backup_window          = "03:00-04:00"
  maintenance_window     = "sun:04:00-sun:05:00"

  vpc_security_group_ids = [aws_security_group.postgres.id]
  db_subnet_group_name   = aws_db_subnet_group.optimizer.name

  parameter_group_name   = aws_db_parameter_group.optimizer.name

  enabled_cloudwatch_logs_exports = ["postgresql", "upgrade"]

  tags = {
    Name        = "optimizer-postgres"
    Environment = "production"
  }
}

resource "aws_db_parameter_group" "optimizer" {
  name   = "optimizer-postgres-params"
  family = "postgres16"

  parameter {
    name  = "shared_buffers"
    value = "16777216"  # 16GB in 8KB pages
  }

  parameter {
    name  = "effective_cache_size"
    value = "50331648"  # 48GB
  }

  parameter {
    name  = "work_mem"
    value = "65536"  # 64MB
  }
}
```

### GCP Deployment

#### Google Cloud Memorystore (Redis)

```hcl
# terraform/memorystore.tf
resource "google_redis_instance" "optimizer" {
  name           = "optimizer-redis"
  tier           = "STANDARD_HA"
  memory_size_gb = 10
  region         = "us-central1"

  redis_version     = "REDIS_7_0"
  display_name      = "Optimizer Redis"

  authorized_network = google_compute_network.optimizer.id

  redis_configs = {
    maxmemory-policy = "allkeys-lru"
  }

  maintenance_policy {
    weekly_maintenance_window {
      day = "SUNDAY"
      start_time {
        hours   = 3
        minutes = 0
      }
    }
  }
}
```

#### Google Cloud SQL (PostgreSQL)

```hcl
# terraform/cloudsql.tf
resource "google_sql_database_instance" "optimizer" {
  name             = "optimizer-postgres"
  database_version = "POSTGRES_16"
  region           = "us-central1"

  settings {
    tier              = "db-custom-8-32768"  # 8 vCPU, 32GB RAM
    availability_type = "REGIONAL"  # HA with automatic failover
    disk_type         = "PD_SSD"
    disk_size         = 500
    disk_autoresize   = true

    backup_configuration {
      enabled                        = true
      point_in_time_recovery_enabled = true
      start_time                     = "03:00"
      transaction_log_retention_days = 7
    }

    database_flags {
      name  = "shared_buffers"
      value = "8388608"  # 8GB
    }

    database_flags {
      name  = "effective_cache_size"
      value = "25165824"  # 24GB
    }

    ip_configuration {
      ipv4_enabled    = false
      private_network = google_compute_network.optimizer.id
    }
  }
}
```

### Azure Deployment

#### Azure Cache for Redis

```hcl
# terraform/azure-redis.tf
resource "azurerm_redis_cache" "optimizer" {
  name                = "optimizer-redis"
  location            = azurerm_resource_group.optimizer.location
  resource_group_name = azurerm_resource_group.optimizer.name
  capacity            = 6
  family              = "P"  # Premium
  sku_name            = "Premium"

  enable_non_ssl_port = false
  minimum_tls_version = "1.2"

  redis_configuration {
    maxmemory_policy = "allkeys-lru"
    rdb_backup_enabled = true
    rdb_backup_frequency = 60
    rdb_backup_max_snapshot_count = 1
  }

  patch_schedule {
    day_of_week    = "Sunday"
    start_hour_utc = 3
  }
}
```

#### Azure Database for PostgreSQL

```hcl
# terraform/azure-postgres.tf
resource "azurerm_postgresql_flexible_server" "optimizer" {
  name                = "optimizer-postgres"
  resource_group_name = azurerm_resource_group.optimizer.name
  location            = azurerm_resource_group.optimizer.location
  version             = "16"

  administrator_login    = "optimizer"
  administrator_password = random_password.db_password.result

  storage_mb            = 524288  # 512GB
  sku_name              = "GP_Standard_D8s_v3"  # 8 vCPU, 32GB RAM
  zone                  = "1"

  backup_retention_days = 7
  geo_redundant_backup_enabled = true

  high_availability {
    mode                      = "ZoneRedundant"
    standby_availability_zone = "2"
  }
}
```

---

## Security Hardening

### Network Security

```yaml
# Kubernetes NetworkPolicy
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: redis-network-policy
  namespace: optimizer
spec:
  podSelector:
    matchLabels:
      app: redis-cluster
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - podSelector:
        matchLabels:
          app: optimizer-app
    ports:
    - protocol: TCP
      port: 6379
  egress:
  - to:
    - podSelector:
        matchLabels:
          app: redis-cluster
    ports:
    - protocol: TCP
      port: 6379
    - protocol: TCP
      port: 16379
```

### Secrets Management

```yaml
# Kubernetes Secret
apiVersion: v1
kind: Secret
metadata:
  name: redis-secret
  namespace: optimizer
type: Opaque
data:
  password: <base64-encoded-password>

---
apiVersion: v1
kind: Secret
metadata:
  name: postgres-secret
  namespace: optimizer
type: Opaque
data:
  username: <base64-encoded-username>
  password: <base64-encoded-password>
```

---

## Capacity Planning

### Sizing Guidelines

| Workload | Redis Config | PostgreSQL Config | Estimated Cost (AWS) |
|----------|-------------|-------------------|---------------------|
| **Small** | cache.r6g.large (13GB) | db.r6g.large (16GB) | $400/month |
| **Medium** | cache.r6g.xlarge (26GB) | db.r6g.xlarge (32GB) | $800/month |
| **Large** | cache.r6g.2xlarge (52GB) | db.r6g.2xlarge (64GB) | $1,600/month |
| **XLarge** | cache.r6g.4xlarge (104GB) | db.r6g.4xlarge (128GB) | $3,200/month |

### Performance Benchmarks

| Backend | Throughput | Latency (p99) | Use Case |
|---------|-----------|---------------|----------|
| Redis Standalone | 100K ops/sec | <1ms | High-frequency cache |
| Redis Cluster | 1M+ ops/sec | <2ms | Massive scale |
| PostgreSQL | 10K ops/sec | 1-5ms | Persistent state |
| PostgreSQL + PgBouncer | 50K ops/sec | 2-8ms | Connection pooling |

---

This deployment guide provides production-ready configurations for all major deployment scenarios. Adjust configurations based on your specific requirements and scale.
