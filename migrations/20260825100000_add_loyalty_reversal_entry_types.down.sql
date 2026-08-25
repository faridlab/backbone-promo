-- Down: documented no-op.
--
-- PostgreSQL cannot drop values from an enum type. The two reversal values are inert residue once
-- no rows carry them (nothing reads them unless the reversal verbs run), so the honest down for
-- this migration is to leave them in place. Restoring the pre-migration byte-state of the enum is
-- impossible without rebuilding every table that references it.
SELECT 1;
