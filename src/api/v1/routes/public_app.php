<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/apps') {
        ApiResponse::json([
            'apps' => $ctx->appRepo->findAll($ctx->limit, $ctx->offset),
        ]);
        return true;
    }

    if (preg_match('#^/apps/([^/]+)/releases/([^/]+)$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];
        $version = $matches[2];

        $app = $ctx->appRepo->findByBundleId($bundleId);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        $release = $ctx->releaseRepo->findByBundleIdAndVersion($bundleId, $version);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        ApiResponse::json($ctx->releaseRepo->toApiRelease($release));
        return true;
    }

    if (preg_match('#^/apps/([^/]+)/releases$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];

        $app = $ctx->appRepo->findByBundleId($bundleId);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        ApiResponse::json([
            'bundle_id' => $bundleId,
            'releases' => $ctx->releaseRepo->findAllByBundleId($bundleId),
        ]);
        return true;
    }

    if (preg_match('#^/apps/([^/]+)/download$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];

        $app = $ctx->appRepo->findByBundleId($bundleId);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        $version = $_GET['version'] ?? null;
        $release = $version === null || $version === ''
            ? $ctx->releaseRepo->findLatestByBundleId($bundleId)
            : $ctx->releaseRepo->findByBundleIdAndVersion($bundleId, $version);

        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        $packagePath = $ctx->storage->absolutePath($release['download_path']);
        if (!is_file($packagePath)) {
            ApiResponse::error('PACKAGE_FILE_MISSING', 'Package file is missing', 500);
            return true;
        }

        ApiResponse::streamFile($packagePath, $bundleId . '-' . $release['version'] . '.pkg');
        return true;
    }

    if (preg_match('#^/apps/([^/]+)$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];

        $app = $ctx->appRepo->findByBundleId($bundleId);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        $app['releases'] = $ctx->releaseRepo->findAllByBundleId($bundleId);
        ApiResponse::json($app);
        return true;
    }

    if ($ctx->path === '/search') {
        $query = trim((string) ($_GET['q'] ?? ''));

        if ($query === '') {
            ApiResponse::json([
                'query' => '',
                'results' => [],
            ]);
            return true;
        }

        ApiResponse::json([
            'query' => $query,
            'results' => $ctx->appRepo->search($query, $ctx->limit, $ctx->offset),
        ]);
        return true;
    }

    return false;
};