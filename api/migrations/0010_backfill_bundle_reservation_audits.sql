INSERT INTO audit_logs (
  audit_id,
  actor_id,
  action,
  target_type,
  target_id,
  metadata_json,
  created_at
)
SELECT
  'audit_bundle_reserve_backfill_' || bundle_id,
  developer_id,
  'bundle.reserve',
  'bundle_id',
  bundle_id,
  '{"source":"migration-backfill"}',
  created_at
FROM bundle_ids
WHERE NOT EXISTS (
  SELECT 1
  FROM audit_logs
  WHERE action = 'bundle.reserve'
    AND target_type = 'bundle_id'
    AND target_id = bundle_ids.bundle_id
);
