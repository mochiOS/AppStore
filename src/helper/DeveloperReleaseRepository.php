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
            dr.created_at,
            dr.package_path,
            dr.package_size,
            dr.changelog,
            dr.review_message,
            dr.submitted_at,
            dr.reviewed_at,
            dr.reviewed_by,
            dr.published_at
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
            created_at,
            package_path,
            package_size,
            changelog,
            review_message,
            submitted_at,
            reviewed_at,
            reviewed_by,
            published_at
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
            created_at,
            package_path,
            package_size,
            changelog,
            review_message,
            submitted_at,
            reviewed_at,
            reviewed_by,
            published_at
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

    public function listByStatus(string $status, int $limit = 50, int $offset = 0): array
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
            dr.created_at,
            dr.package_path,
            dr.package_size,
            dr.changelog,
            dr.review_message,
            dr.submitted_at,
            dr.reviewed_at,
            dr.reviewed_by,
            dr.published_at,
            da.display_name,
            da.icon_path,
            da.description
         FROM developer_releases dr
         LEFT JOIN developer_apps da
            ON da.bundle_id = dr.bundle_id
         WHERE dr.status = :status
         ORDER BY dr.submitted_at DESC, dr.created_at DESC
         LIMIT :limit OFFSET :offset'
        );

        $stmt->bindValue(':status', $status, PDO::PARAM_STR);
        $stmt->bindValue(':limit', $limit, PDO::PARAM_INT);
        $stmt->bindValue(':offset', $offset, PDO::PARAM_INT);
        $stmt->execute();

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    /**
     * @throws Throwable
     */
    public function approve(string $releaseId, string $adminId): ?array
    {
        $release = $this->findById($releaseId);

        if ($release === null) {
            return null;
        }

        if ($release['status'] !== 'submitted') {
            return $release;
        }

        $now = date('c');

        $this->db->beginTransaction();

        try {
            $stmt = $this->db->prepare(
                'UPDATE developer_releases
             SET status = :status,
                 review_message = NULL,
                 reviewed_at = :reviewed_at,
                 reviewed_by = :reviewed_by,
                 published_at = :published_at
             WHERE release_id = :release_id'
            );

            $stmt->execute([
                ':status' => 'published',
                ':reviewed_at' => $now,
                ':reviewed_by' => $adminId,
                ':published_at' => $now,
                ':release_id' => $releaseId,
            ]);

            $appUpdate = $this->db->prepare(
                'UPDATE developer_apps
             SET latest_version = :latest_version,
                 visibility = :visibility
             WHERE bundle_id = :bundle_id'
            );

            $appUpdate->execute([
                ':latest_version' => $release['version'],
                ':visibility' => 'public',
                ':bundle_id' => $release['bundle_id'],
            ]);

            $this->db->commit();
        } catch (Throwable $e) {
            if ($this->db->inTransaction()) {
                $this->db->rollBack();
            }

            throw $e;
        }

        return $this->findById($releaseId);
    }

    public function reject(string $releaseId, string $adminId, string $message): ?array
    {
        $release = $this->findById($releaseId);

        if ($release === null) {
            return null;
        }

        if ($release['status'] !== 'submitted') {
            return $release;
        }

        $stmt = $this->db->prepare(
            'UPDATE developer_releases
         SET status = :status,
             review_message = :review_message,
             reviewed_at = :reviewed_at,
             reviewed_by = :reviewed_by
         WHERE release_id = :release_id'
        );

        $stmt->execute([
            ':status' => 'rejected',
            ':review_message' => $message,
            ':reviewed_at' => date('c'),
            ':reviewed_by' => $adminId,
            ':release_id' => $releaseId,
        ]);

        return $this->findById($releaseId);
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