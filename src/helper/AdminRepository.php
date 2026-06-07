<?php

class AdminRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function findByDeveloperId(string $developerId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT developer_id, role, created_at
             FROM admin_developers
             WHERE developer_id = :developer_id
             LIMIT 1'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        $admin = $stmt->fetch(PDO::FETCH_ASSOC);

        return $admin === false ? null : $admin;
    }

    public function isAdmin(string $developerId): bool
    {
        return $this->findByDeveloperId($developerId) !== null;
    }
}