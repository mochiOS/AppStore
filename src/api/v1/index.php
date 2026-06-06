<?php

session_start();

if (!defined('ROOT')) {
    define('ROOT', __DIR__ . '/../../');
}

require_once ROOT . 'helper/Database.php';
require_once ROOT . 'helper/AppRepository.php';
require_once ROOT . 'helper/ReleaseRepository.php';
require_once ROOT . 'helper/ApiResponse.php';
require_once ROOT . 'helper/PackageStorage.php';
require_once ROOT . 'helper/AppConfig.php';

$appConfig = AppConfig::get();

$origin = $_SERVER['HTTP_ORIGIN'] ?? '';

if (in_array($origin, $appConfig['allowed_origins'], true)) {
    header('Access-Control-Allow-Origin: ' . $origin);
    header('Access-Control-Allow-Credentials: true');
    header('Access-Control-Allow-Headers: Content-Type');
    header('Access-Control-Allow-Methods: GET, POST, OPTIONS');
}

$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';

if (($_SERVER['REQUEST_METHOD'] ?? 'GET') === 'OPTIONS') {
    http_response_code(204);
    exit;
}

$path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?: '/';
$path = urldecode($path);
$path = preg_replace('#/index\.php$#', '/', $path);

if (str_starts_with($path, '/v1')) {
    $path = substr($path, 3);
    if ($path === '') {
        $path = '/';
    }
}

if ($path !== '/') {
    $path = rtrim($path, '/');
    if ($path === '') {
        $path = '/';
    }
}

$appRepo = new AppRepository(Database::get());
$releaseRepo = new ReleaseRepository(Database::get());
$storage = new PackageStorage(ROOT);

$limit = isset($_GET['limit']) ? max(0, (int) $_GET['limit']) : 50;
$offset = isset($_GET['offset']) ? max(0, (int) $_GET['offset']) : 0;

switch (true) {
    case $path === '/apps':
        ApiResponse::json([
            'apps' => $appRepo->findAll($limit, $offset),
        ]);
        return;

    case preg_match('#^/apps/([^/]+)/releases/([^/]+)$#', $path, $matches) === 1:
        $bundleId = $matches[1];
        $version = $matches[2];
        $app = $appRepo->findByBundleId($bundleId);

        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        $release = $releaseRepo->findByBundleIdAndVersion($bundleId, $version);
        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return;
        }

        ApiResponse::json($releaseRepo->toApiRelease($release));
        return;

    case preg_match('#^/apps/([^/]+)/releases$#', $path, $matches) === 1:
        $bundleId = $matches[1];
        $app = $appRepo->findByBundleId($bundleId);

        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        ApiResponse::json([
            'bundle_id' => $bundleId,
            'releases' => $releaseRepo->findAllByBundleId($bundleId),
        ]);
        return;

    case preg_match('#^/apps/([^/]+)/download$#', $path, $matches) === 1:
        $bundleId = $matches[1];
        $app = $appRepo->findByBundleId($bundleId);

        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        $version = $_GET['version'] ?? null;
        $release = $version === null || $version === ''
            ? $releaseRepo->findLatestByBundleId($bundleId)
            : $releaseRepo->findByBundleIdAndVersion($bundleId, $version);

        if ($release === null) {
            ApiResponse::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return;
        }

        $packagePath = $storage->absolutePath($release['download_path']);
        if (!is_file($packagePath)) {
            $storage->ensurePlaceholderPackage(
                $release['download_path'],
                "mochiOS placeholder package for $bundleId {$release['version']}\n"
            );
        }

        ApiResponse::streamFile($packagePath, $bundleId . '-' . $release['version'] . '.pkg');
        return;

    case preg_match('#^/apps/([^/]+)$#', $path, $matches) === 1:
        $bundleId = $matches[1];
        $app = $appRepo->findByBundleId($bundleId);

        if ($app === null) {
            ApiResponse::error('APP_NOT_FOUND', 'App not found', 404);
            return;
        }

        $app['releases'] = $releaseRepo->findAllByBundleId($bundleId);
        ApiResponse::json($app);
        return;

    case $path === '/search':
        $query = trim((string) ($_GET['q'] ?? ''));
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

    case $path === '/auth/me':
        if ($method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return;
        }

        $userId = $_SESSION['user_id'] ?? null;

        if (!$userId) {
            ApiResponse::error('UNAUTHORIZED', 'Not logged in', 401);
            return;
        }

        $stmt = Database::get()->prepare(
            'SELECT id, provider, provider_user_id, username, display_name, avatar_url
             FROM users
             WHERE id = :id
             LIMIT 1'
        );

        $stmt->execute([
            ':id' => $userId,
        ]);

        $user = $stmt->fetch(PDO::FETCH_ASSOC);

        if (!$user) {
            ApiResponse::error('UNAUTHORIZED', 'User not found', 401);
            return;
        }

        ApiResponse::json([
            'user' => $user,
        ]);
        return;

    case $path === '/auth/logout':
        if ($method !== 'POST') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return;
        }

        $_SESSION = [];

        if (session_status() === PHP_SESSION_ACTIVE) {
            session_destroy();
        }

        ApiResponse::json([
            'ok' => true,
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


