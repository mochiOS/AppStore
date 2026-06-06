<?php

class ReleaseRepository
{
    public function __construct(
        private readonly PDO $db
    ) {}

    public function findAllByBundleId(string $bundleId): array
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id, version, size, sha256, changelog, download_path, created_at
             FROM releases
             WHERE bundle_id = :bundle_id
             ORDER BY created_at DESC, version DESC'
        );
        $stmt->execute([
            ':bundle_id' => $bundleId,
        ]);

        $rows = $stmt->fetchAll(PDO::FETCH_ASSOC);

        return array_map(
            fn(array $row): array => $this->toApiRelease($row),
            $rows
        );
    }

    public function findByBundleIdAndVersion(string $bundleId, string $version): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id, version, size, sha256, changelog, download_path, created_at
             FROM releases
             WHERE bundle_id = :bundle_id
               AND version = :version
             LIMIT 1'
        );
        $stmt->execute([
            ':bundle_id' => $bundleId,
            ':version' => $version,
        ]);

        $row = $stmt->fetch(PDO::FETCH_ASSOC);

        return $row === false ? null : $row;
    }

    public function findLatestByBundleId(string $bundleId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id, version, size, sha256, changelog, download_path, created_at
             FROM releases
             WHERE bundle_id = :bundle_id
             ORDER BY created_at DESC, version DESC
             LIMIT 1'
        );
        $stmt->execute([
            ':bundle_id' => $bundleId,
        ]);

        $row = $stmt->fetch(PDO::FETCH_ASSOC);

        return $row === false ? null : $row;
    }

    public function toApiRelease(array $row): array
    {
        return [
            'version' => $row['version'],
            'size' => (int) $row['size'],
            'sha256' => $row['sha256'],
            'changelog' => $row['changelog'],
            'download_url' => '/apps/' . rawurlencode($row['bundle_id']) . '/download?version=' . rawurlencode($row['version']),
            'created_at' => $row['created_at'],
        ];
    }

    public function downloadPath(array $row): string
    {
        return $row['download_path'];
    }
}


