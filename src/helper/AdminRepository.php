<?php

class AdminRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function add(string $developerId, string $role): array
    {
        if (!in_array($role, ['admin', 'owner'], true)) {
            throw new InvalidArgumentException('role must be admin or owner');
        }

        $now = date('c');

        $stmt = $this->db->prepare(
            'INSERT INTO admin_developers (
            developer_id,
            role,
            created_at
        ) VALUES (
            :developer_id,
            :role,
            :created_at
        )
        ON CONFLICT(developer_id) DO UPDATE SET
            role = excluded.role'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
            ':role' => $role,
            ':created_at' => $now,
        ]);

        $admin = $this->findByDeveloperId($developerId);

        if ($admin === null) {
            throw new RuntimeException('failed to add admin developer');
        }

        return $admin;
    }

    public function delete(string $developerId): bool
    {
        $stmt = $this->db->prepare(
            'DELETE FROM admin_developers
         WHERE developer_id = :developer_id'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        return $stmt->rowCount() > 0;
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