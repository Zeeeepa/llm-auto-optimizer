-- Initial state table schema
-- This creates the primary state storage table with JSONB metadata support

CREATE TABLE IF NOT EXISTS state_entries (
    key BYTEA PRIMARY KEY,
    value BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    metadata JSONB
);

-- Add a trigger to automatically update the updated_at timestamp
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ language 'plpgsql';

CREATE TRIGGER update_state_entries_updated_at
    BEFORE UPDATE ON state_entries
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Create checkpoint table for state snapshots
CREATE TABLE IF NOT EXISTS state_checkpoints (
    checkpoint_id UUID PRIMARY KEY,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    checkpoint_data JSONB NOT NULL,
    metadata JSONB,
    row_count BIGINT NOT NULL DEFAULT 0,
    data_size BIGINT NOT NULL DEFAULT 0
);

-- Add comment documentation
COMMENT ON TABLE state_entries IS 'Primary state storage for distributed stream processing';
COMMENT ON COLUMN state_entries.key IS 'State key (binary format)';
COMMENT ON COLUMN state_entries.value IS 'State value (binary format)';
COMMENT ON COLUMN state_entries.expires_at IS 'Optional expiration timestamp for TTL support';
COMMENT ON COLUMN state_entries.metadata IS 'Optional JSON metadata for additional context';

COMMENT ON TABLE state_checkpoints IS 'State checkpoints for backup and recovery';
