-- Remove note column from endpoints table
-- Note: SQLite doesn't support DROP COLUMN directly in older versions
-- This is a simplified version - in production you might need to recreate the table
ALTER TABLE endpoints DROP COLUMN note;
