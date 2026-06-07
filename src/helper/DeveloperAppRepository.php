<?php

class DeveloperAppRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function listByDeveloperId(string $developerId): array
    {
        $stmt = $this->db->prepare(
            'SELECT
            da.app_id,
            da.bundle_id,
            da.latest_version,
            da.display_name,
            da.icon_path,
            da.description,
            da.visibility,
            ata.team_id,
            da.created_at
         FROM developer_apps da
         INNER JOIN bundle_ids bi
            ON bi.bundle_id = da.bundle_id
         LEFT JOIN app_team_assignments ata
            ON ata.bundle_id = da.bundle_id
         WHERE bi.developer_id = :developer_id
         ORDER BY da.created_at DESC'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findByBundleId(string $bundleId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT
                da.app_id,
                da.bundle_id,
                da.latest_version,
                da.display_name,
                da.icon_path,
                da.description,
                da.visibility,
                da.created_at,
                bi.developer_id,
                bi.app_name,
                bi.status AS bundle_status
             FROM developer_apps da
             INNER JOIN bundle_ids bi
                ON bi.bundle_id = da.bundle_id
             WHERE da.bundle_id = :bundle_id
             LIMIT 1'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
        ]);

        $app = $stmt->fetch(PDO::FETCH_ASSOC);

        return $app === false ? null : $app;
    }

    public function findOwnedByBundleId(string $bundleId, string $developerId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT
            da.app_id,
            da.bundle_id,
            da.latest_version,
            da.display_name,
            da.icon_path,
            da.description,
            da.visibility,
            ata.team_id,
            da.created_at
         FROM developer_apps da
         INNER JOIN bundle_ids bi
            ON bi.bundle_id = da.bundle_id
         LEFT JOIN app_team_assignments ata
            ON ata.bundle_id = da.bundle_id
         WHERE da.bundle_id = :bundle_id
           AND bi.developer_id = :developer_id
         LIMIT 1'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
            ':developer_id' => $developerId,
        ]);

        $app = $stmt->fetch(PDO::FETCH_ASSOC);

        return $app === false ? null : $app;
    }

    public function bundleIsOwnedByDeveloper(string $bundleId, string $developerId): bool
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id
             FROM bundle_ids
             WHERE bundle_id = :bundle_id
               AND developer_id = :developer_id
               AND status != :blocked_status
             LIMIT 1'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
            ':developer_id' => $developerId,
            ':blocked_status' => 'blocked',
        ]);

        return $stmt->fetch(PDO::FETCH_ASSOC) !== false;
    }

    public function create(
        string $developerId,
        string $bundleId,
        string $displayName,
        ?string $description,
        ?string $iconPath
    ): ?array {
        if (!$this->bundleIsOwnedByDeveloper($bundleId, $developerId)) {
            return null;
        }

        $appId = 'app_' . bin2hex(random_bytes(16));
        $createdAt = date('c');

        $stmt = $this->db->prepare(
            'INSERT INTO developer_apps (
                app_id,
                bundle_id,
                latest_version,
                display_name,
                icon_path,
                description,
                visibility,
                created_at
            ) VALUES (
                :app_id,
                :bundle_id,
                NULL,
                :display_name,
                :icon_path,
                :description,
                :visibility,
                :created_at
            )'
        );

        $stmt->execute([
            ':app_id' => $appId,
            ':bundle_id' => $bundleId,
            ':display_name' => $displayName,
            ':icon_path' => $iconPath,
            ':description' => $description,
            ':visibility' => 'private',
            ':created_at' => $createdAt,
        ]);

        return $this->findOwnedByBundleId($bundleId, $developerId);
    }

    public function setTeam(
        string $developerId,
        string $bundleId,
        ?string $teamId
    ): ?array {
        $app = $this->findOwnedByBundleId($bundleId, $developerId);

        if ($app === null) {
            return null;
        }

        if ($teamId === null) {
            $stmt = $this->db->prepare(
                'DELETE FROM app_team_assignments
             WHERE bundle_id = :bundle_id'
            );

            $stmt->execute([
                ':bundle_id' => $bundleId,
            ]);

            return $this->findOwnedByBundleId($bundleId, $developerId);
        }

        $stmt = $this->db->prepare(
            'INSERT INTO app_team_assignments (
            bundle_id,
            team_id,
            assigned_at
        ) VALUES (
            :bundle_id,
            :team_id,
            :assigned_at
        )
        ON CONFLICT(bundle_id) DO UPDATE SET
            team_id = excluded.team_id,
            assigned_at = excluded.assigned_at'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
            ':team_id' => $teamId,
            ':assigned_at' => date('c'),
        ]);

        return $this->findOwnedByBundleId($bundleId, $developerId);
    }
}