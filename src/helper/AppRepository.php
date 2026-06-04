<?php

class AppRepository
{
    public function __construct(
        private PDO $db
    ) {}

    public function findAll(int $limit = 50, int $offset = 0): array
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id, name, version, developer, description, icon
             FROM apps
             ORDER BY name
             LIMIT :limit OFFSET :offset'
        );

        $stmt->bindValue(':limit', $limit, PDO::PARAM_INT);
        $stmt->bindValue(':offset', $offset, PDO::PARAM_INT);
        $stmt->execute();

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findByBundleId(string $bundleId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT bundle_id, name, version, developer, description, icon
             FROM apps
             WHERE bundle_id = :bundle_id
             LIMIT 1'
        );
        $stmt->execute([
            ':bundle_id' => $bundleId,
        ]);

        $app = $stmt->fetch(PDO::FETCH_ASSOC);

        return $app === false ? null : $app;
    }

    public function search(string $query, int $limit = 50, int $offset = 0): array
    {
        $pattern = '%' . str_replace(
            ['\\', '%', '_'],
            ['\\\\', '\%', '\_'],
            $query
        ) . '%';

        $stmt = $this->db->prepare(
            "SELECT bundle_id, name, version, developer, description, icon
             FROM apps
             WHERE name LIKE :query ESCAPE '\\'
                OR bundle_id LIKE :query ESCAPE '\\'
                OR developer LIKE :query ESCAPE '\\'
                OR description LIKE :query ESCAPE '\\'
             ORDER BY name
             LIMIT :limit OFFSET :offset"
        );

        $stmt->bindValue(':query', $pattern, PDO::PARAM_STR);
        $stmt->bindValue(':limit', $limit, PDO::PARAM_INT);
        $stmt->bindValue(':offset', $offset, PDO::PARAM_INT);
        $stmt->execute();

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }
}

?>
