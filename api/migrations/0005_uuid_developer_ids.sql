UPDATE bundle_ids SET developer_id = replace(developer_id, '-', '');
UPDATE apps SET developer_id = replace(developer_id, '-', '');
UPDATE public_keys SET developer_id = replace(developer_id, '-', '');
UPDATE team_members SET developer_id = replace(developer_id, '-', '');
UPDATE teams SET created_by = replace(created_by, '-', '');
UPDATE releases
SET registered_by = replace(registered_by, '-', '')
WHERE registered_by IS NOT NULL;
UPDATE releases
SET developer_certificate_developer_id = replace(
    replace(developer_certificate_developer_id, 'org.mochios.developer.', ''),
    '-',
    ''
)
WHERE developer_certificate_developer_id IS NOT NULL;
