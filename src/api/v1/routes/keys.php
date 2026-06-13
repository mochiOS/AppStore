<?php

return function (ApiContext $ctx): bool {
    if (preg_match('#^/keys/([^/]+)$#', $ctx->path, $matches) === 1) {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $keyId = rawurldecode($matches[1]);

        if (!preg_match('/^[A-Za-z0-9._:-]+$/', $keyId)) {
            ApiResponse::error('VALIDATION_ERROR', 'key_id is invalid', 422);
            return true;
        }

        $key = $ctx->publicKeyRepo->findByKeyId($keyId);

        if ($key === null) {
            ApiResponse::error('KEY_NOT_FOUND', 'Key not found', 404);
            return true;
        }

        ApiResponse::json([
            'key' => [
                'key_id' => $key['key_id'],
                'public_key' => $key['public_key'],
                'revoked_at' => $key['revoked_at'],
            ],
        ]);

        return true;
    }

    if ($ctx->path === '/keys') {
        $developerId = requireDeveloperId();

        if ($ctx->method === 'GET') {
            ApiResponse::json([
                'keys' => $ctx->publicKeyRepo->listByDeveloperId($developerId),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $payload = readJsonBody();

        $keyId = trim((string) ($payload['key_id'] ?? ''));
        $publicKey = trim((string) ($payload['public_key'] ?? ''));

        if ($keyId === '') {
            ApiResponse::error('VALIDATION_ERROR', 'key_id is required', 422);
            return true;
        }

        if (!preg_match('/^[A-Za-z0-9._:-]+$/', $keyId)) {
            ApiResponse::error('VALIDATION_ERROR', 'key_id is invalid', 422);
            return true;
        }

        if ($publicKey === '') {
            ApiResponse::error('VALIDATION_ERROR', 'public_key is required', 422);
            return true;
        }

        if (!PublicKeyRepository::isValidEd25519PublicKey($publicKey)) {
            ApiResponse::error(
                'VALIDATION_ERROR',
                'public_key must be a base64-encoded 32-byte Ed25519 public key',
                422
            );
            return true;
        }

        try {
            $key = $ctx->publicKeyRepo->create($developerId, $keyId, $publicKey);
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
        $keyId = rawurldecode($matches[1]);

        $key = $ctx->publicKeyRepo->revokeForDeveloper($keyId, $developerId);

        if ($key === null) {
            ApiResponse::error('KEY_NOT_FOUND', 'Key not found', 404);
            return true;
        }

        ApiResponse::json([
            'key' => $key,
        ]);
        return true;
    }

    return false;
};