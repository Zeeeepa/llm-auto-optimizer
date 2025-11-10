-- Enhanced PostgreSQL features migration
-- Adds version column, partitioning support, and advanced indexes

-- Add version column for optimistic locking
ALTER TABLE state_entries ADD COLUMN IF NOT EXISTS version BIGINT NOT NULL DEFAULT 1;

-- Create index on version for optimistic locking queries
CREATE INDEX IF NOT EXISTS idx_state_entries_version ON state_entries(key, version);

-- Add advisory lock tracking table
CREATE TABLE IF NOT EXISTS state_advisory_locks (
    lock_id BIGINT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    metadata JSONB
);

CREATE INDEX IF NOT EXISTS idx_advisory_locks_expires
    ON state_advisory_locks(expires_at)
    WHERE expires_at IS NOT NULL;

-- Add replication lag monitoring view
CREATE OR REPLACE VIEW replication_lag AS
SELECT
    CASE WHEN pg_is_in_recovery() THEN
        EXTRACT(EPOCH FROM (NOW() - pg_last_xact_replay_timestamp())) * 1000
    ELSE 0 END as lag_ms,
    pg_is_in_recovery() as is_replica,
    pg_last_xact_replay_timestamp() as last_replay_timestamp;

-- Add table bloat monitoring function
CREATE OR REPLACE FUNCTION get_table_bloat()
RETURNS TABLE (
    table_name TEXT,
    bloat_size BIGINT,
    bloat_ratio NUMERIC
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        schemaname || '.' || tablename as table_name,
        pg_total_relation_size(schemaname || '.' || tablename) -
        pg_relation_size(schemaname || '.' || tablename) as bloat_size,
        CASE WHEN pg_relation_size(schemaname || '.' || tablename) > 0 THEN
            ROUND((pg_total_relation_size(schemaname || '.' || tablename)::numeric /
                   pg_relation_size(schemaname || '.' || tablename)::numeric - 1) * 100, 2)
        ELSE 0
        END as bloat_ratio
    FROM pg_tables
    WHERE schemaname = 'public'
    AND tablename LIKE '%state%';
END;
$$ LANGUAGE plpgsql;

-- Add batch cleanup function with rate limiting
CREATE OR REPLACE FUNCTION cleanup_expired_state_entries_batch(batch_size INTEGER DEFAULT 1000)
RETURNS BIGINT AS $$
DECLARE
    deleted_count BIGINT := 0;
    batch_deleted BIGINT;
BEGIN
    LOOP
        DELETE FROM state_entries
        WHERE ctid IN (
            SELECT ctid FROM state_entries
            WHERE expires_at IS NOT NULL
              AND expires_at <= NOW()
            LIMIT batch_size
        );

        GET DIAGNOSTICS batch_deleted = ROW_COUNT;
        deleted_count := deleted_count + batch_deleted;

        EXIT WHEN batch_deleted < batch_size;

        -- Small delay to avoid overwhelming the system
        PERFORM pg_sleep(0.1);
    END LOOP;

    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Add function to get detailed table statistics
CREATE OR REPLACE FUNCTION get_state_table_stats()
RETURNS TABLE (
    total_rows BIGINT,
    active_rows BIGINT,
    expired_rows BIGINT,
    total_size_bytes BIGINT,
    table_size_bytes BIGINT,
    indexes_size_bytes BIGINT,
    toast_size_bytes BIGINT,
    avg_row_size BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        (SELECT count(*) FROM state_entries)::BIGINT,
        (SELECT count(*) FROM state_entries WHERE expires_at IS NULL OR expires_at > NOW())::BIGINT,
        (SELECT count(*) FROM state_entries WHERE expires_at IS NOT NULL AND expires_at <= NOW())::BIGINT,
        pg_total_relation_size('state_entries')::BIGINT,
        pg_relation_size('state_entries')::BIGINT,
        pg_indexes_size('state_entries')::BIGINT,
        (pg_total_relation_size('state_entries') - pg_relation_size('state_entries'))::BIGINT,
        CASE WHEN (SELECT count(*) FROM state_entries) > 0 THEN
            (pg_relation_size('state_entries') / (SELECT count(*) FROM state_entries))::BIGINT
        ELSE 0
        END;
END;
$$ LANGUAGE plpgsql;

-- Add notification channel for state changes
CREATE OR REPLACE FUNCTION notify_state_change()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM pg_notify('state_change', json_build_object(
            'operation', 'INSERT',
            'key', encode(NEW.key, 'hex'),
            'timestamp', extract(epoch from NOW())
        )::text);
    ELSIF TG_OP = 'UPDATE' THEN
        PERFORM pg_notify('state_change', json_build_object(
            'operation', 'UPDATE',
            'key', encode(NEW.key, 'hex'),
            'timestamp', extract(epoch from NOW())
        )::text);
    ELSIF TG_OP = 'DELETE' THEN
        PERFORM pg_notify('state_change', json_build_object(
            'operation', 'DELETE',
            'key', encode(OLD.key, 'hex'),
            'timestamp', extract(epoch from NOW())
        )::text);
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

-- Optional: Create trigger for state change notifications (commented out by default)
-- Uncomment to enable real-time notifications
-- CREATE TRIGGER state_change_notify
--     AFTER INSERT OR UPDATE OR DELETE ON state_entries
--     FOR EACH ROW EXECUTE FUNCTION notify_state_change();

-- Add archival table for historical data
CREATE TABLE IF NOT EXISTS state_entries_archive (
    key BYTEA,
    value BYTEA,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    metadata JSONB,
    version BIGINT
);

CREATE INDEX IF NOT EXISTS idx_state_archive_archived_at
    ON state_entries_archive(archived_at);

CREATE INDEX IF NOT EXISTS idx_state_archive_key
    ON state_entries_archive(key, archived_at);

-- Add function for automatic archival
CREATE OR REPLACE FUNCTION archive_old_state_entries(age_days INTEGER DEFAULT 30)
RETURNS BIGINT AS $$
DECLARE
    archived_count BIGINT;
BEGIN
    INSERT INTO state_entries_archive
    SELECT *, NOW() as archived_at
    FROM state_entries
    WHERE created_at < NOW() - (age_days || ' days')::INTERVAL;

    GET DIAGNOSTICS archived_count = ROW_COUNT;

    DELETE FROM state_entries
    WHERE created_at < NOW() - (age_days || ' days')::INTERVAL;

    RETURN archived_count;
END;
$$ LANGUAGE plpgsql;

-- Add partition maintenance function (for future use)
CREATE OR REPLACE FUNCTION create_time_partition(
    table_name TEXT,
    start_date DATE,
    end_date DATE
) RETURNS TEXT AS $$
DECLARE
    partition_name TEXT;
BEGIN
    partition_name := table_name || '_' || to_char(start_date, 'YYYY_MM_DD');

    EXECUTE format(
        'CREATE TABLE IF NOT EXISTS %I PARTITION OF %I
         FOR VALUES FROM (%L) TO (%L)',
        partition_name,
        table_name,
        start_date,
        end_date
    );

    RETURN partition_name;
END;
$$ LANGUAGE plpgsql;

-- Enable pg_stat_statements extension for query monitoring (if not already enabled)
-- This requires superuser privileges, so it may fail in some environments
DO $$
BEGIN
    CREATE EXTENSION IF NOT EXISTS pg_stat_statements;
EXCEPTION
    WHEN insufficient_privilege THEN
        RAISE NOTICE 'Could not create pg_stat_statements extension (requires superuser). Query statistics will be limited.';
    WHEN duplicate_object THEN
        NULL; -- Extension already exists
END $$;

-- Add comments for documentation
COMMENT ON COLUMN state_entries.version IS 'Version number for optimistic locking';
COMMENT ON TABLE state_advisory_locks IS 'Tracks advisory lock ownership';
COMMENT ON TABLE state_entries_archive IS 'Historical archive of state entries';
COMMENT ON FUNCTION cleanup_expired_state_entries_batch IS 'Batch cleanup of expired entries with rate limiting';
COMMENT ON FUNCTION get_state_table_stats IS 'Get comprehensive table statistics';
COMMENT ON FUNCTION archive_old_state_entries IS 'Archive and purge old state entries';

-- Grant necessary permissions (adjust as needed for your environment)
-- GRANT SELECT, INSERT, UPDATE, DELETE ON state_entries TO your_app_user;
-- GRANT SELECT ON replication_lag TO your_app_user;
-- GRANT EXECUTE ON FUNCTION get_state_table_stats() TO your_app_user;
