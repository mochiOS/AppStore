<?php

class DeveloperReleaseRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function listByBundleIdForDeveloper(string $developerId, string $bundleId): array
    {
        $stmt = $this->db->prepare(
            'SELECT
                dr.release_id,
                dr.bundle_id,
                dr.version,
                dr.manifest_hash,
                dr.package_hash,
                dr.signature,
                dr.certificate_id,
                dr.status,
                dr.created_at
             FROM developer_releases dr
             INNER JOIN bundle_ids bi
                ON bi.bundle_id = dr.bundle_id
             WHERE dr.bundle_id = :bundle_id
               AND bi.developer_id = :developer_id
             ORDER BY dr.created_at DESC, dr.version DESC'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findById(string $releaseId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT
                release_id,
                bundle_id,
                version,
                manifest_hash,
                package_hash,
                signature,
                certificate_id,
                status,
                created_at
             FROM developer_releases
             WHERE release_id = :release_id
             LIMIT 1'
        );

        $stmt->execute([
            ':release_id' => $releaseId,
        ]);

        $release = $stmt->fetch(PDO::FETCH_ASSOC);

        return $release === false ? null : $release;
    }

    public function findOwnedById(string $releaseId, string $developerId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT
                dr.release_id,
                dr.bundle_id,
                dr.version,
                dr.manifest_hash,
                dr.package_hash,
                dr.signature,
                dr.certificate_id,
                dr.status,
                dr.created_at
             FROM developer_releases dr
             INNER JOIN bundle_ids bi
                ON bi.bundle_id = dr.bundle_id
             WHERE dr.release_id = :release_id
               AND bi.developer_id = :developer_id
             LIMIT 1'
        );

        $stmt->execute([
            ':release_id' => $releaseId,
            ':developer_id' => $developerId,
        ]);

        $release = $stmt->fetch(PDO::FETCH_ASSOC);

        return $release === false ? null : $release;
    }

    public function findByBundleIdAndVersion(string $bundleId, string $version): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT
                release_id,
                bundle_id,
                version,
                manifest_hash,
                package_hash,
                signature,
                certificate_id,
                status,
                created_at
             FROM developer_releases
             WHERE bundle_id = :bundle_id
               AND version = :version
             LIMIT 1'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
            ':version' => $version,
        ]);

        $release = $stmt->fetch(PDO::FETCH_ASSOC);

        return $release === false ? null : $release;
    }

    public function versionExists(string $bundleId, string $version): bool
    {
        return $this->findByBundleIdAndVersion($bundleId, $version) !== null;
    }

    public function createDraft(
        string $developerId,
        string $bundleId,
        string $version,
        ?string $manifestHash,
        string $packageHash,
        ?string $signature,
        ?string $certificateId,
        string $packagePath,
        int $packageSize,
        ?string $changelog
    ): ?array {
        if (!$this->bundleIsOwnedByDeveloper($bundleId, $developerId)) {
            return null;
        }

        $releaseId = 'rel_' . bin2hex(random_bytes(16));
        $createdAt = date('c');

        $stmt = $this->db->prepare(
            'INSERT INTO developer_releases (
            release_id,
            bundle_id,
            version,
            manifest_hash,
            package_hash,
            signature,
            certificate_id,
            status,
            created_at,
            package_path,
            package_size,
            changelog
        ) VALUES (
            :release_id,
            :bundle_id,
            :version,
            :manifest_hash,
            :package_hash,
            :signature,
            :certificate_id,
            :status,
            :created_at,
            :package_path,
            :package_size,
            :changelog
        )'
        );

        $stmt->execute([
            ':release_id' => $releaseId,
            ':bundle_id' => $bundleId,
            ':version' => $version,
            ':manifest_hash' => $manifestHash,
            ':package_hash' => $packageHash,
            ':signature' => $signature,
            ':certificate_id' => $certificateId,
            ':status' => 'draft',
            ':created_at' => $createdAt,
            ':package_path' => $packagePath,
            ':package_size' => $packageSize,
            ':changelog' => $changelog,
        ]);

        return $this->findOwnedById($releaseId, $developerId);
    }

    public function submitOwned(string $releaseId, string $developerId): ?array
    {
        $release = $this->findOwnedById($releaseId, $developerId);

        if ($release === null) {
            return null;
        }

        if (!in_array($release['status'], ['draft', 'rejected'], true)) {
            return $release;
        }

        $stmt = $this->db->prepare(
            'UPDATE developer_releases
        SET status = :status,
        submitted_at = :submitted_at
        WHERE release_id = :release_id'
        );

        $stmt->execute([
            ':status' => 'submitted',
            ':submitted_at' => date('c'),
            ':release_id' => $releaseId,
        ]);

        return $this->findOwnedById($releaseId, $developerId);
    }

    public function updateStatus(string $releaseId, string $status): ?array
    {
        $release = $this->findById($releaseId);

        if ($release === null) {
            return null;
        }

        $stmt = $this->db->prepare(
            'UPDATE developer_releases
             SET status = :status
             WHERE release_id = :release_id'
        );

        $stmt->execute([
            ':status' => $status,
            ':release_id' => $releaseId,
        ]);

        return $this->findById($releaseId);
    }

    private function bundleIsOwnedByDeveloper(string $bundleId, string $developerId): bool
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id
             FROM bundle_ids
             WHERE bundle_id = :bundle_id
               AND developer_id = :developer_id
             LIMIT 1'
        );

        $stmt->execute([
            ':bundle_id' => $bundleId,
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetch(PDO::FETCH_ASSOC) !== false;
    }
}