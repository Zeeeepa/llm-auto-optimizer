-- Performance optimization indexes
-- These indexes improve query performance for common access patterns

-- Index for expiration-based queries (TTL cleanup)
CREATE INDEX IF NOT EXISTS idx_state_entries_expires_at
    ON state_entries(expires_at)
    WHERE expires_at IS NOT NULL;

-- Partial index for non-expired entries (most common queries)
CREATE INDEX IF NOT EXISTS idx_state_entries_active
    ON state_entries(key)
    WHERE expires_at IS NULL OR expires_at > NOW();

-- GIN index for JSONB metadata queries
CREATE INDEX IF NOT EXISTS idx_state_entries_metadata
    ON state_entries USING GIN (metadata jsonb_path_ops);

-- Index for timestamp-based queries
CREATE INDEX IF NOT EXISTS idx_state_entries_created_at
    ON state_entries(created_at);

CREATE INDEX IF NOT EXISTS idx_state_entries_updated_at
    ON state_entries(updated_at);

-- Index for checkpoint queries
CREATE INDEX IF NOT EXISTS idx_state_checkpoints_created_at
    ON state_checkpoints(created_at DESC);

-- Add statistics for better query planning
ALTER TABLE state_entries ALTER COLUMN key SET STATISTICS 1000;
ALTER TABLE state_entries ALTER COLUMN expires_at SET STATISTICS 1000;

-- Create a function to cleanup expired entries
CREATE OR REPLACE FUNCTION cleanup_expired_state_entries()
RETURNS BIGINT AS $$
DECLARE
    deleted_count BIGINT;
BEGIN
    DELETE FROM state_entries
    WHERE expires_at IS NOT NULL
      AND expires_at <= NOW();

    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

COMMENT ON FUNCTION cleanup_expired_state_entries() IS 'Remove expired state entries and return count deleted';
