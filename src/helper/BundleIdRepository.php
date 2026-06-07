<?php

class BundleIdRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function listByDeveloperId(string $developerId): array
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id, developer_id, app_name, status, created_at
             FROM bundle_ids
             WHERE developer_id = :developer_id
             ORDER BY created_at DESC'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findByBundleId(string $bundleId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id, developer_id, app_name, status, created_at
             FROM bundle_ids
             WHERE bundle_id = :bundle_id
             LIMIT 1'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
        ]);

        $bundle = $stmt->fetch(PDO::FETCH_ASSOC);

        return $bundle === false ? null : $bundle;
    }

    public function isOwnedByDeveloper(string $bundleId, string $developerId): bool
    {
        $bundle = $this->findByBundleId($bundleId);

        return $bundle !== null && $bundle['developer_id'] === $developerId;
    }

    public function create(string $developerId, string $bundleId, string $appName): array
    {
        $bundle = [
            'bundle_id' => $bundleId,
            'developer_id' => $developerId,
            'app_name' => $appName,
            'status' => 'reserved',
            'created_at' => date('c'),
        ];

        $stmt = $this->db->prepare(
            'INSERT INTO bundle_ids (
                bundle_id,
                developer_id,
                app_name,
                status,
                created_at
            ) VALUES (
                :bundle_id,
                :developer_id,
                :app_name,
                :status,
                :created_at
            )'
        );

        $stmt->execute([
            ':bundle_id' => $bundle['bundle_id'],
            ':developer_id' => $bundle['developer_id'],
            ':app_name' => $bundle['app_name'],
            ':status' => $bundle['status'],
            ':created_at' => $bundle['created_at'],
        ]);

        return $bundle;
    }
}