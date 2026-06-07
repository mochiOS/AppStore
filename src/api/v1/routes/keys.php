<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/keys') {
        $developerId = requireDeveloperId();

        if ($ctx->method === 'GET') {
            $stmt = $ctx->db->prepare(
                'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
                 FROM public_keys
                 WHERE developer_id = :developer_id
                 ORDER BY created_at DESC'
            );

            $stmt->execute([
                ':developer_id' => $developerId,
            ]);

            ApiResponse::json([
                'keys' => $stmt->fetchAll(PDO::FETCH_ASSOC),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $payload = readJsonBody();
        $publicKey = trim((string) ($payload['public_key'] ?? ''));

        if ($publicKey === '') {
            ApiResponse::error('VALIDATION_ERROR', 'public_key is required', 422);
            return true;
        }

        $key = [
            'key_id' => 'key_' . bin2hex(random_bytes(16)),
            'developer_id' => $developerId,
            'public_key' => $publicKey,
            'fingerprint' => hash('sha256', $publicKey),
            'created_at' => date('c'),
            'revoked_at' => null,
        ];

        $stmt = $ctx->db->prepare(
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

        try {
            $stmt->execute([
                ':key_id' => $key['key_id'],
                ':developer_id' => $key['developer_id'],
                ':public_key' => $key['public_key'],
                ':fingerprint' => $key['fingerprint'],
                ':created_at' => $key['created_at'],
                ':revoked_at' => $key['revoked_at'],
            ]);
        } catch (PDOException $e) {
            if ($e->getCode() === '23000') {
                ApiResponse::error('KEY_ALREADY_EXISTS', 'Public key already exists', 409);
                return true;
            }

            throw $e;
        }

        ApiResponse::json([
            'key' => $key,
        ], 201);
        return true;
    }

    if (preg_match('#^/keys/([^/]+)/revoke$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $developerId = requireDeveloperId();
        $keyId = $matches[1];

        $stmt = $ctx->db->prepare(
            'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
             FROM public_keys
             WHERE key_id = :key_id
             LIMIT 1'
        );

        $stmt->execute([
            ':key_id' => $keyId,
        ]);

        $key = $stmt->fetch(PDO::FETCH_ASSOC);

        if ($key === false || $key['developer_id'] !== $developerId) {
            ApiResponse::error('KEY_NOT_FOUND', 'Key not found', 404);
            return true;
        }

        if ($key['revoked_at'] === null) {
            $key['revoked_at'] = date('c');

            $update = $ctx->db->prepare(
                'UPDATE public_keys
                 SET revoked_at = :revoked_at
                 WHERE key_id = :key_id'
            );

            $update->execute([
                ':revoked_at' => $key['revoked_at'],
                ':key_id' => $keyId,
            ]);
        }

        ApiResponse::json([
            'key' => $key,
        ]);
        return true;
    }

    return false;
};