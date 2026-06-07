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