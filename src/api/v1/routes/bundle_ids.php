<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path !== '/bundle-ids') {
        return false;
    }

    $developerId = requireDeveloperId();

    if ($ctx->method === 'GET') {
        $stmt = $ctx->db->prepare(
            'SELECT bundle_id, developer_id, app_name, status, created_at
             FROM bundle_ids
             WHERE developer_id = :developer_id
             ORDER BY created_at DESC'
        );

        $stmt->execute([
            ':developer_id' => $developerId,
        ]);

        ApiResponse::json([
            'bundle_ids' => $stmt->fetchAll(PDO::FETCH_ASSOC),
        ]);
        return true;
    }

    if ($ctx->method !== 'POST') {
        ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
        return true;
    }

    $payload = readJsonBody();
    $bundleId = trim((string) ($payload['bundle_id'] ?? ''));
    $appName = trim((string) ($payload['app_name'] ?? ''));

    if (
        $bundleId === ''
        || !preg_match('/^[a-z0-9.-]+$/', $bundleId)
        || !str_contains($bundleId, '.')
    ) {
        ApiResponse::error('VALIDATION_ERROR', 'bundle_id is invalid', 422);
        return true;
    }

    if ($appName === '') {
        ApiResponse::error('VALIDATION_ERROR', 'app_name is required', 422);
        return true;
    }

    $bundle = [
        'bundle_id' => $bundleId,
        'developer_id' => $developerId,
        'app_name' => $appName,
        'status' => 'reserved',
        'created_at' => date('c'),
    ];

    $stmt = $ctx->db->prepare(
        'INSERT INTO bundle_ids (
            bundle_id,
            developer_id,
            app_name,
            status,
            created_at
        ) VALUES (
            :bundle_id,
            :developer_id,
            :app_name,
            :status,
            :created_at
        )'
    );

    try {
        $stmt->execute([
            ':bundle_id' => $bundle['bundle_id'],
            ':developer_id' => $bundle['developer_id'],
            ':app_name' => $bundle['app_name'],
            ':status' => $bundle['status'],
            ':created_at' => $bundle['created_at'],
        ]);
    } catch (PDOException $e) {
        if ($e->getCode() === '23000') {
            ApiResponse::error('BUNDLE_ID_ALREADY_EXISTS', 'Bundle ID already exists', 409);
            return true;
        }

        throw $e;
    }

    ApiResponse::json([
        'bundle_id' => $bundle,
    ], 201);
    return true;
};