<?php

if (!defined('ROOT')) {
    define('ROOT', __DIR__ . '/../../');
}

require_once ROOT . 'helper/ApiRequest.php';
require_once ROOT . 'helper/Paths.php';
require_once ROOT . 'helper/Database.php';
require_once ROOT . 'helper/AppRepository.php';
require_once ROOT . 'helper/ReleaseRepository.php';
require_once ROOT . 'helper/AppCatalog.php';
require_once ROOT . 'helper/ApiResponse.php';
require_once ROOT . 'helper/PackageStorage.php';

$method = ApiRequest::method();
if ($method !== 'GET') {
    ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
    return;
}

$appRepo = new AppRepository(Database::get());
$releaseRepo = new ReleaseRepository(Database::get());
$catalog = new AppCatalog($appRepo, $releaseRepo);
$storage = new PackageStorage(ROOT);

$path = ApiRequest::path();
$limit = ApiRequest::queryInt('limit', 50, 0);
$offset = ApiRequest::queryInt('offset', 0, 0);

switch (true) {
    case $path === '/apps':
        ApiResponse::json($catalog->listApps($limit, $offset));
        return;

    case preg_match('#^/apps/([^/]+)/releases/([^/]+)$#', $path, $matches) === 1:
        if (!$catalog->hasApp($matches[1])) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        $release = $catalog->findRelease($matches[1], $matches[2]);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return;
        }

        ApiResponse::json($release);
        return;

    case preg_match('#^/apps/([^/]+)/releases$#', $path, $matches) === 1:
        $payload = $catalog->listReleases($matches[1]);
        if ($payload === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        ApiResponse::json($payload);
        return;

    case preg_match('#^/apps/([^/]+)/download$#', $path, $matches) === 1:
        $bundleId = $matches[1];
        $version = ApiRequest::queryString('version');

        if (!$catalog->hasApp($bundleId)) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        $release = $catalog->findDownloadRelease($bundleId, $version);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return;
        }

        $packagePath = $storage->absolutePath($release['download_path']);
        if (!is_file($packagePath)) {
            $storage->ensurePlaceholderPackage(
                $release['download_path'],
                "mochiOS placeholder package for {$bundleId} {$release['version']}\n"
            );
        }

        ApiResponse::streamFile($packagePath, $bundleId . '-' . $release['version'] . '.pkg');
        return;

    case preg_match('#^/apps/([^/]+)$#', $path, $matches) === 1:
        $app = $catalog->findAppDetail($matches[1]);
        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        ApiResponse::json($app);
        return;

    case $path === '/search':
        $query = ApiRequest::queryString('q', '');
        if ($query === '') {
            ApiResponse::json([
                'query' => '',
                'results' => [],
            ]);
            return;
        }

        ApiResponse::json([
            'query' => $query,
            'results' => $appRepo->search($query, $limit, $offset),
        ]);
        return;
}

http_response_code(404);
header('Content-Type: application/json; charset=utf-8');
echo json_encode([
    'error' => [
        'code' => 'NOT_FOUND',
        'message' => 'Not found',
    ],
], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);

?>
