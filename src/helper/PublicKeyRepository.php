<?php

class PublicKeyRepository
{
    public function __construct(
        private readonly PDO $db
    ) {
    }

    public function listByDeveloperId(string $developerId): array
    {
        $stmt = $this->db->prepare(
            'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
             FROM public_keys
             WHERE developer_id = :developer_id
             ORDER BY created_at DESC'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }

    public function findByKeyId(string $keyId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
             FROM public_keys
             WHERE key_id = :key_id
             LIMIT 1'
        );

        $stmt->execute([
            ':key_id' => $keyId,
        ]);

        $key = $stmt->fetch(PDO::FETCH_ASSOC);

        return $key === false ? null : $key;
    }

    public function create(string $developerId, string $keyId, string $publicKey): array
    {
        $key = [
            'key_id' => $keyId,
            'developer_id' => $developerId,
            'public_key' => $publicKey,
            'fingerprint' => hash('sha256', $publicKey),
            'created_at' => date('c'),
            'revoked_at' => null,
        ];

        $stmt = $this->db->prepare(
            'INSERT INTO public_keys (
            key_id,
            developer_id,
            public_key,
            fingerprint,
            created_at,
            revoked_at
        ) VALUES (
            :key_id,
            :developer_id,
            :public_key,
            :fingerprint,
            :created_at,
            :revoked_at
        )'
        );

        $stmt->execute([
            ':key_id' => $key['key_id'],
            ':developer_id' => $key['developer_id'],
            ':public_key' => $key['public_key'],
            ':fingerprint' => $key['fingerprint'],
            ':created_at' => $key['created_at'],
            ':revoked_at' => $key['revoked_at'],
        ]);

        return $key;
    }

    public function revokeForDeveloper(string $keyId, string $developerId): ?array
    {
        $key = $this->findByKeyId($keyId);

        if ($key === null || $key['developer_id'] !== $developerId) {
            return null;
        }

        if ($key['revoked_at'] !== null) {
            return $key;
        }

        $key['revoked_at'] = date('c');

        $stmt = $this->db->prepare(
            'UPDATE public_keys
             SET revoked_at = :revoked_at
             WHERE key_id = :key_id'
        );

        $stmt->execute([
            ':revoked_at' => $key['revoked_at'],
            ':key_id' => $keyId,
        ]);

        return $key;
    }

    public function findActiveOwnedByKeyId(string $keyId, string $developerId): ?array
    {
        $stmt = $this->db->prepare(
            'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
         FROM public_keys
         WHERE key_id = :key_id
           AND developer_id = :developer_id
           AND revoked_at IS NULL
         LIMIT 1'
        );


        $stmt->execute([
            ':key_id' => $keyId,
            ':developer_id' => $developerId,
        ]);

        $key = $stmt->fetch(PDO::FETCH_ASSOC);

        return $key === false ? null : $key;
    }
}