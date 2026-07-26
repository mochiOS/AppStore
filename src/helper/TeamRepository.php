<?php

class TeamRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function create(string $creatorDeveloperId, string $name, string $slug): array
    {
        $teamId = 'team_' . bin2hex(random_bytes(16));
        $now = date('c');

        $this->db->beginTransaction();

        try {
            $stmt = $this->db->prepare(
                'INSERT INTO teams (
                    team_id,
                    name,
                    slug,
                    created_by,
                    created_at
                ) VALUES (
                    :team_id,
                    :name,
                    :slug,
                    :created_by,
                    :created_at
                )'
            );

            $stmt->execute([
                ':team_id' => $teamId,
                ':name' => $name,
                ':slug' => $slug,
                ':created_by' => $creatorDeveloperId,
                ':created_at' => $now,
            ]);

            $member = $this->db->prepare(
                'INSERT INTO team_members (
                    team_id,
                    developer_id,
                    role,
                    joined_at
                ) VALUES (
                    :team_id,
                    :developer_id,
                    :role,
                    :joined_at
                )'
            );

            $member->execute([
                ':team_id' => $teamId,
                ':developer_id' => $creatorDeveloperId,
                ':role' => 'owner',
                ':joined_at' => $now,
            ]);

            $this->db->commit();
        } catch (Throwable $e) {
            if ($this->db->inTransaction()) {
                $this->db->rollBack();
            }

            throw $e;
        }

        $team = $this->findByIdForDeveloper($teamId, $creatorDeveloperId);

        if ($team === null) {
            throw new RuntimeException('failed to create team');
        }

        return $team;
    }

    public function listByDeveloperId(string $developerId): array
    {
        $stmt = $this->db->prepare(
            'SELECT
                t.team_id,
                t.name,
                t.slug,
                t.created_by,
                t.created_at,
                tm.role,
                tm.joined_at
             FROM team_members tm
             INNER JOIN teams t
                ON t.team_id = tm.team_id
             WHERE tm.developer_id = :developer_id
             ORDER BY t.created_at DESC'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findById(string $teamId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT team_id, name, slug, created_by, created_at
             FROM teams
             WHERE team_id = :team_id
             LIMIT 1'
        );

        $stmt->execute([
            ':team_id' => $teamId,
        ]);

        $team = $stmt->fetch(PDO::FETCH_ASSOC);

        return $team === false ? null : $team;
    }

    public function findByIdForDeveloper(string $teamId, string $developerId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT
                t.team_id,
                t.name,
                t.slug,
                t.created_by,
                t.created_at,
                tm.role,
                tm.joined_at
             FROM teams t
             INNER JOIN team_members tm
                ON tm.team_id = t.team_id
             WHERE t.team_id = :team_id
               AND tm.developer_id = :developer_id
             LIMIT 1'
        );

        $stmt->execute([
            ':team_id' => $teamId,
            ':developer_id' => $developerId,
        ]);

        $team = $stmt->fetch(PDO::FETCH_ASSOC);

        return $team === false ? null : $team;
    }

    public function listMembers(string $teamId): array
    {
        $stmt = $this->db->prepare(
            'SELECT
                tm.team_id,
                tm.developer_id,
                tm.role,
                tm.joined_at
             FROM team_members tm
             WHERE tm.team_id = :team_id
             ORDER BY
                CASE tm.role
                    WHEN "owner" THEN 0
                    WHEN "admin" THEN 1
                    WHEN "developer" THEN 2
                    ELSE 3
                END,
                tm.joined_at ASC'
        );

        $stmt->execute([
            ':team_id' => $teamId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findMember(string $teamId, string $developerId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT team_id, developer_id, role, joined_at
             FROM team_members
             WHERE team_id = :team_id
               AND developer_id = :developer_id
             LIMIT 1'
        );

        $stmt->execute([
            ':team_id' => $teamId,
            ':developer_id' => $developerId,
        ]);

        $member = $stmt->fetch(PDO::FETCH_ASSOC);

        return $member === false ? null : $member;
    }

    public function canManageMembers(string $teamId, string $developerId): bool
    {
        $member = $this->findMember($teamId, $developerId);

        return $member !== null && in_array($member['role'], ['owner', 'admin'], true);
    }

    public function canChangeOwnerRole(string $teamId, string $developerId): bool
    {
        $member = $this->findMember($teamId, $developerId);

        return $member !== null && $member['role'] === 'owner';
    }

    public function addMember(string $teamId, string $developerId, string $role): array
    {
        $this->assertValidRole($role);

        $stmt = $this->db->prepare(
            'INSERT INTO team_members (
                team_id,
                developer_id,
                role,
                joined_at
            ) VALUES (
                :team_id,
                :developer_id,
                :role,
                :joined_at
            )
            ON CONFLICT(team_id, developer_id) DO UPDATE SET
                role = excluded.role'
        );

        $stmt->execute([
            ':team_id' => $teamId,
            ':developer_id' => $developerId,
            ':role' => $role,
            ':joined_at' => date('c'),
        ]);

        $member = $this->findMember($teamId, $developerId);

        if ($member === null) {
            throw new RuntimeException('failed to add team member');
        }

        return $member;
    }

    public function removeMember(string $teamId, string $developerId): bool
    {
        $member = $this->findMember($teamId, $developerId);

        if ($member === null) {
            return false;
        }

        if ($member['role'] === 'owner' && $this->countOwners($teamId) <= 1) {
            throw new RuntimeException('cannot remove the last owner');
        }

        $stmt = $this->db->prepare(
            'DELETE FROM team_members
             WHERE team_id = :team_id
               AND developer_id = :developer_id'
        );

        $stmt->execute([
            ':team_id' => $teamId,
            ':developer_id' => $developerId,
        ]);

        return $stmt->rowCount() > 0;
    }

    public function updateMemberRole(string $teamId, string $developerId, string $role): ?array
    {
        $this->assertValidRole($role);

        $current = $this->findMember($teamId, $developerId);

        if ($current === null) {
            return null;
        }

        if ($current['role'] === 'owner' && $role !== 'owner' && $this->countOwners($teamId) <= 1) {
            throw new RuntimeException('cannot demote the last owner');
        }

        $stmt = $this->db->prepare(
            'UPDATE team_members
             SET role = :role
             WHERE team_id = :team_id
               AND developer_id = :developer_id'
        );

        $stmt->execute([
            ':role' => $role,
            ':team_id' => $teamId,
            ':developer_id' => $developerId,
        ]);

        return $this->findMember($teamId, $developerId);
    }

    private function countOwners(string $teamId): int
    {
        $stmt = $this->db->prepare(
            'SELECT COUNT(*) AS count
             FROM team_members
             WHERE team_id = :team_id
               AND role = :role'
        );

        $stmt->execute([
            ':team_id' => $teamId,
            ':role' => 'owner',
        ]);

        $row = $stmt->fetch(PDO::FETCH_ASSOC);

        return (int) ($row['count'] ?? 0);
    }

    private function assertValidRole(string $role): void
    {
        if (!in_array($role, ['owner', 'admin', 'developer', 'viewer'], true)) {
            throw new InvalidArgumentException('role is invalid');
        }
    }
}
