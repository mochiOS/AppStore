<?php

if (!class_exists('ApiAbortException')) {
    class ApiAbortException extends RuntimeException
    {
    }
}

function requireDeveloperId(): string
{
    $developerId = $_SESSION['developer_id'] ?? null;

    if (!is_string($developerId) || $developerId === '') {
        ApiResponse::error('UNAUTHORIZED', 'Not logged in', 401);
        throw new ApiAbortException('Unauthorized');
    }

    return $developerId;
}

function readJsonBody(): array
{
    $raw = $GLOBALS['APPSTORE_TEST_INPUT'] ?? null;
    if (!is_string($raw)) {
        $raw = file_get_contents('php://input');
    }

    if ($raw === false || trim($raw) === '') {
        return [];
    }

    $payload = json_decode($raw, true);
    if (!is_array($payload)) {
        ApiResponse::error('INVALID_JSON', 'Request body must be valid JSON', 400);
        throw new ApiAbortException('Invalid JSON');
    }

    return $payload;
}

function isStateChangingMethod(string $method): bool
{
    return in_array($method, ['POST', 'PUT', 'PATCH', 'DELETE'], true);
}

function requireValidCsrfToken(array $appConfig, string $method): void
{
    if (!isStateChangingMethod($method)) {
        return;
    }

    $origin = $_SERVER['HTTP_ORIGIN'] ?? '';
    if ($origin !== '' && !appstoreIsAllowedOrigin($appConfig, $origin)) {
        ApiResponse::error('CSRF_ORIGIN_INVALID', 'Request origin is not allowed', 403);
        throw new ApiAbortException('Invalid CSRF origin');
    }

    $fetchSite = strtolower((string) ($_SERVER['HTTP_SEC_FETCH_SITE'] ?? ''));
    if ($fetchSite === 'cross-site' && ($origin === '' || !appstoreIsAllowedOrigin($appConfig, $origin))) {
        ApiResponse::error('CSRF_ORIGIN_INVALID', 'Cross-site requests are not allowed', 403);
        throw new ApiAbortException('Invalid fetch site');
    }

    $expected = $_SESSION['csrf_token'] ?? '';
    $actual = $_SERVER['HTTP_X_CSRF_TOKEN'] ?? '';

    if (!is_string($expected) || $expected === '' || !is_string($actual) || !hash_equals($expected, $actual)) {
        ApiResponse::error('CSRF_TOKEN_INVALID', 'CSRF token is missing or invalid', 403);
        throw new ApiAbortException('Invalid CSRF token');
    }
}

function requireAdminDeveloper(ApiContext $ctx): array
{
    $developerId = requireDeveloperId();

    $admin = $ctx->adminRepo->findByDeveloperId($developerId);

    if ($admin === null) {
        ApiResponse::error('FORBIDDEN', 'Admin permission is required', 403);
        throw new ApiAbortException('Forbidden');
    }

    return $admin;
}

function requireOwnerDeveloper(ApiContext $ctx): array
{
    $admin = requireAdminDeveloper($ctx);

    if (($admin['role'] ?? '') !== 'owner') {
        ApiResponse::error('FORBIDDEN', 'Owner permission is required', 403);
        throw new ApiAbortException('Owner permission required');
    }

    return $admin;
}

function writeAuditLog(
    ApiContext $ctx,
    ?string $actorDeveloperId,
    string $action,
    string $targetType,
    string $targetId,
    array $metadata = []
): void {
    $stmt = $ctx->db->prepare(
        'INSERT INTO audit_logs (
            audit_id,
            actor_developer_id,
            action,
            target_type,
            target_id,
            metadata_json,
            created_at
        ) VALUES (
            :audit_id,
            :actor_developer_id,
            :action,
            :target_type,
            :target_id,
            :metadata_json,
            :created_at
        )'
    );

    $stmt->execute([
        ':audit_id' => 'audit_' . bin2hex(random_bytes(16)),
        ':actor_developer_id' => $actorDeveloperId,
        ':action' => $action,
        ':target_type' => $targetType,
        ':target_id' => $targetId,
        ':metadata_json' => $metadata === [] ? null : json_encode($metadata, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE),
        ':created_at' => date('c'),
    ]);
}
