<?php

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/apps') {
        ApiResponse::json($ctx->appCatalog->listApps($ctx->limit, $ctx->offset));
        return true;
    }

    if (preg_match('#^/apps/([^/]+)/releases/([^/]+)$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];
        $version = $matches[2];

        if (!$ctx->appCatalog->hasApp($bundleId)) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        $release = $ctx->appCatalog->findRelease($bundleId, $version);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        ApiResponse::json($release);
        return true;
    }

    if (preg_match('#^/apps/([^/]+)/releases$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];

        $releases = $ctx->appCatalog->listReleases($bundleId);
        if ($releases === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        ApiResponse::json($releases);
        return true;
    }

    if (preg_match('#^/apps/([^/]+)/download$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];
        $version = ApiRequest::queryString('version');

        if (!$ctx->appCatalog->hasApp($bundleId)) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        $release = $ctx->appCatalog->findDownloadRelease($bundleId, $version);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return true;
        }

        $downloadPath = $ctx->releaseRepo->downloadPath($release);
        $packagePath = $ctx->storage->absolutePath($downloadPath);

        if (!is_file($packagePath)) {
            ApiResponse::error('PACKAGE_FILE_MISSING', 'Package file is missing', 500);
            return true;
        }

        ApiResponse::streamFile(
            $packagePath,
            $bundleId . '-' . $release['version'] . '.pkg'
        );
        return true;
    }

    if (preg_match('#^/apps/([^/]+)$#', $ctx->path, $matches) === 1) {
        $bundleId = $matches[1];

        $app = $ctx->appCatalog->findAppDetail($bundleId);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return true;
        }

        ApiResponse::json($app);
        return true;
    }

    if ($ctx->path === '/search') {
        $query = ApiRequest::queryString('q', '');

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