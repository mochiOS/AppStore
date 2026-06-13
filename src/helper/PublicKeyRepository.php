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

    public function findByPublicKey(string $publicKey): ?array
    {
        $decodedPublicKey = self::decodeEd25519PublicKey($publicKey);

        if ($decodedPublicKey === null) {
            return null;
        }

        $fingerprint = hash('sha256', $decodedPublicKey);

        $stmt = $this->db->prepare(
            'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
         FROM public_keys
         WHERE fingerprint = :fingerprint
         LIMIT 1'
        );

        $stmt->execute([
            ':fingerprint' => $fingerprint,
        ]);

        $key = $stmt->fetch(PDO::FETCH_ASSOC);

        return $key === false ? null : $key;
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
        $decodedPublicKey = self::decodeEd25519PublicKey($publicKey);

        if ($decodedPublicKey === null) {
            throw new InvalidArgumentException('public_key must be a base64-encoded 32-byte Ed25519 public key');
        }

        $publicKey = base64_encode($decodedPublicKey);

        $key = [
            'key_id' => $keyId,
            'developer_id' => $developerId,
            'public_key' => $publicKey,
            'fingerprint' => hash('sha256', $decodedPublicKey),
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

    public static function normalizePublicKey(string $publicKey): string
    {
        return trim($publicKey);
    }

    public static function isValidEd25519PublicKey(string $publicKey): bool
    {
        return self::decodeEd25519PublicKey($publicKey) !== null;
    }

    public static function publicKeyMaterialEquals(string $left, string $right): bool
    {
        $leftDecoded = self::decodeEd25519PublicKey($left);
        $rightDecoded = self::decodeEd25519PublicKey($right);

        if ($leftDecoded !== null && $rightDecoded !== null) {
            return hash_equals($leftDecoded, $rightDecoded);
        }

        return hash_equals(self::normalizePublicKey($left), self::normalizePublicKey($right));
    }

    private static function decodeEd25519PublicKey(string $publicKey): ?string
    {
        $publicKey = preg_replace('/\s+/', '', self::normalizePublicKey($publicKey)) ?? '';

        if ($publicKey === '') {
            return null;
        }

        $padding = strlen($publicKey) % 4;
        if ($padding !== 0) {
            $publicKey .= str_repeat('=', 4 - $padding);
        }

        $decoded = base64_decode($publicKey, true);

        return $decoded !== false && strlen($decoded) === 32 ? $decoded : null;
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
