<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path !== '/bundle-ids') {
        return false;
    }

    $developerId = requireDeveloperId();

    if ($ctx->method === 'GET') {
        ApiResponse::json([
            'bundle_ids' => $ctx->bundleIdRepo->listByDeveloperId($developerId),
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

    try {
        $bundle = $ctx->bundleIdRepo->create($developerId, $bundleId, $appName);
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