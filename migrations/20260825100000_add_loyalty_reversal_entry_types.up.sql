-- Add the two return-document entry types to the loyalty ledger's direction enum.
--
-- 'grant_reversed' (negative points, source = the return document) claws back part or all of a
-- grant; 'spend_reversed' (positive points) gives back points a return made whole. They are the
-- compensating-entry vocabulary for per-order reversals — the ledger stays an append-only signed
-- SUM and never rewrites history.
--
-- Standalone on purpose: PostgreSQL tolerates ALTER TYPE ... ADD VALUE inside a migration
-- transaction only when the new value is not USED in the same transaction (PG12+); nothing here
-- uses either value. This file must run before any verb or later migration writes them.

ALTER TYPE loyalty_entry_type ADD VALUE IF NOT EXISTS 'grant_reversed';
ALTER TYPE loyalty_entry_type ADD VALUE IF NOT EXISTS 'spend_reversed';
