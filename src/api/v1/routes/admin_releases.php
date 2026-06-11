<?php


return function (ApiContext $ctx): bool {
    if ($ctx->path === '/admin/releases') {
        $admin = requireAdminDeveloper($ctx);
        $adminId = $admin['developer_id'];

        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $status = ApiRequest::queryString('status', 'submitted');

        if (!in_array($status, ['draft', 'submitted', 'published', 'rejected'], true)) {
            ApiResponse::error('VALIDATION_ERROR', 'status is invalid', 422);
            return true;
        }

        ApiResponse::json([
            'admin' => $adminId,
            'status' => $status,
            'releases' => $ctx->developerReleaseRepo->listByStatus(
                $status,
                $ctx->limit,
                $ctx->offset
            ),
        ]);
        return true;
    }

    if (preg_match('#^/admin/releases/([^/]+)/approve$#', $ctx->path, $matches) === 1) {
        $admin = requireAdminDeveloper($ctx);
        $adminId = $admin['developer_id'];

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $releaseId = urldecode($matches[1]);
        $current = $ctx->developerReleaseRepo->findById($releaseId);

        if ($current === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        if ($current['status'] !== 'submitted') {
            ApiResponse::error(
                'INVALID_RELEASE_STATUS',
                'Only submitted releases can be approved',
                409
            );
            return true;
        }

        $releaseDeveloperId = $ctx->developerReleaseRepo->findDeveloperIdByReleaseId($releaseId);
        $keyId = (string) ($current['certificate_id'] ?? '');
        if (
            $releaseDeveloperId === null
            || $keyId === ''
            || $ctx->publicKeyRepo->findActiveOwnedByKeyId($keyId, $releaseDeveloperId) === null
        ) {
            ApiResponse::error(
                'SIGNING_KEY_NOT_ACTIVE',
                'Release signing key is not active',
                409
            );
            return true;
        }

        $release = $ctx->developerReleaseRepo->approve($releaseId, $adminId);

        writeAuditLog($ctx, $adminId, 'release.approve', 'release', $releaseId, [
            'bundle_id' => $current['bundle_id'],
            'version' => $current['version'],
        ]);

        ApiResponse::json([
            'release' => $release,
        ]);
        return true;
    }

    if (preg_match('#^/admin/releases/([^/]+)/reject$#', $ctx->path, $matches) === 1) {
        $admin = requireAdminDeveloper($ctx);
        $adminId = $admin['developer_id'];

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $payload = readJsonBody();
        $message = trim((string)($payload['message'] ?? ''));

        if ($message === '') {
            ApiResponse::error('VALIDATION_ERROR', 'message is required', 422);
            return true;
        }

        $releaseId = urldecode($matches[1]);
        $current = $ctx->developerReleaseRepo->findById($releaseId);

        if ($current === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        if ($current['status'] !== 'submitted') {
            ApiResponse::error(
                'INVALID_RELEASE_STATUS',
                'Only submitted releases can be rejected',
                409
            );
            return true;
        }

        $release = $ctx->developerReleaseRepo->reject(
            $releaseId,
            $adminId,
            $message
        );

        writeAuditLog($ctx, $adminId, 'release.reject', 'release', $releaseId, [
            'bundle_id' => $current['bundle_id'],
            'version' => $current['version'],
        ]);

        ApiResponse::json([
            'release' => $release,
        ]);
        return true;
    }

    if (preg_match('#^/admin/releases/([^/]+)/download$#', $ctx->path, $matches) === 1) {
        requireAdminDeveloper($ctx);

        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $releaseId = urldecode($matches[1]);
        $release = $ctx->developerReleaseRepo->findById($releaseId);

        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        $packagePath = (string) ($release['package_path'] ?? '');

        if ($packagePath === '') {
            ApiResponse::error('PACKAGE_NOT_FOUND', 'Package path is missing', 404);
            return true;
        }

        $absolutePath = $ctx->storage->absolutePath($packagePath);

        if (!is_file($absolutePath)) {
            ApiResponse::error('PACKAGE_FILE_MISSING', 'Package file is missing', 404);
            return true;
        }

        $bundleId = (string) ($release['bundle_id'] ?? 'app');
        $version = (string) ($release['version'] ?? 'unknown');

        ApiResponse::streamFile(
            $absolutePath,
            $bundleId . '-' . $version . '.pkg'
        );
        return true;
    }

    return false;
};
