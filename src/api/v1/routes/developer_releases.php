<?php

return function (ApiContext $ctx): bool {
    if (preg_match('#^/developer/apps/([^/]+)/releases$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $bundleId = urldecode($matches[1]);

        $app = $ctx->developerAppRepo->findOwnedByBundleId($bundleId, $developerId);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        if ($ctx->method === 'GET') {
            ApiResponse::json([
                'bundle_id' => $bundleId,
                'releases' => $ctx->developerReleaseRepo->listByBundleIdForDeveloper(
                    $developerId,
                    $bundleId
                ),
            ]);
            return true;
        }

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        if (!isset($_FILES['package'])) {
            ApiResponse::error('VALIDATION_ERROR', 'package is required', 422);
            return true;
        }

        $changelog = isset($_POST['changelog'])
            ? trim((string) $_POST['changelog'])
            : null;

        if ($changelog === '') {
            $changelog = null;
        }

        try {
            $inspection = $ctx->packageInspectService->inspect($_FILES['package']['tmp_name']);

            $about = $inspection['about'];
            $packageBundleId = (string) $about['bundle_id'];
            $version = (string) $about['version'];

            if ($packageBundleId !== $bundleId) {
                ApiResponse::error(
                    'BUNDLE_ID_MISMATCH',
                    'Package bundle_id does not match URL bundle_id',
                    422
                );
                return true;
            }

            if ($ctx->developerReleaseRepo->versionExists($bundleId, $version)) {
                ApiResponse::error('RELEASE_ALREADY_EXISTS', 'Release version already exists', 409);
                return true;
            }

            $stored = $ctx->packageUploadService->storeUploadedPackage(
                $_FILES['package'],
                $bundleId,
                $version
            );

            $signature = null;
            $certificateId = null;

            if (isset($inspection['signature']) && is_array($inspection['signature'])) {
                $signature = isset($inspection['signature']['signature'])
                    ? (string) $inspection['signature']['signature']
                    : null;

                $certificateId = isset($inspection['signature']['certificate_id'])
                    ? (string) $inspection['signature']['certificate_id']
                    : null;
            }

            $release = $ctx->developerReleaseRepo->createDraft(
                developerId: $developerId,
                bundleId: $bundleId,
                version: $version,
                manifestHash: $inspection['hashes']['manifest_hash'],
                packageHash: $inspection['hashes']['content_hash'],
                signature: $signature,
                certificateId: $certificateId,
                packagePath: $stored['relative_path'],
                packageSize: (int) $stored['size'],
                changelog: $changelog
            );

            if ($release === null) {
                ApiResponse::error(
                    'BUNDLE_ID_NOT_OWNED',
                    'Bundle ID is not owned by this developer',
                    403
                );
                return true;
            }

            ApiResponse::json([
                'release' => $release,
                'package' => [
                    'path' => $stored['relative_path'],
                    'size' => $stored['size'],
                    'sha256' => $stored['sha256'],
                ],
                'inspection' => [
                    'about' => $inspection['about'],
                    'hashes' => $inspection['hashes'],
                ],
            ], 201);
            return true;
        } catch (RuntimeException $e) {
            ApiResponse::error('PACKAGE_INVALID', $e->getMessage(), 422);
            return true;
        }
    }

    if (preg_match('#^/developer/releases/([^/]+)$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $releaseId = urldecode($matches[1]);

        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $release = $ctx->developerReleaseRepo->findOwnedById($releaseId, $developerId);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        ApiResponse::json([
            'release' => $release,
        ]);
        return true;
    }

    if (preg_match('#^/developer/releases/([^/]+)/submit$#', $ctx->path, $matches) === 1) {
        $developerId = requireDeveloperId();
        $releaseId = urldecode($matches[1]);

        if ($ctx->method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $current = $ctx->developerReleaseRepo->findOwnedById($releaseId, $developerId);
        if ($current === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        if (!in_array($current['status'], ['draft', 'rejected'], true)) {
            ApiResponse::error(
                'INVALID_RELEASE_STATUS',
                'Only draft or rejected releases can be submitted',
                409
            );
            return true;
        }

        $release = $ctx->developerReleaseRepo->submitOwned($releaseId, $developerId);

        ApiResponse::json([
            'release' => $release,
        ]);
        return true;
    }

    return false;
};