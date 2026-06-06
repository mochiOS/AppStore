<?php

if (session_status() !== PHP_SESSION_ACTIVE) {
    session_start();
}

if (!defined('ROOT')) {
    define('ROOT', __DIR__ . '/../../');
}

require_once ROOT . 'helper/Database.php';
require_once ROOT . 'helper/AppRepository.php';
require_once ROOT . 'helper/ReleaseRepository.php';
require_once ROOT . 'helper/DeveloperRepository.php';
require_once ROOT . 'helper/ApiResponse.php';
require_once ROOT . 'helper/ApiRequest.php';
require_once ROOT . 'helper/PackageStorage.php';
require_once ROOT . 'helper/AppConfig.php';
require_once ROOT . 'helper/Paths.php';

$appConfig = AppConfig::get();

$origin = $_SERVER['HTTP_ORIGIN'] ?? '';

if (in_array($origin, $appConfig['allowed_origins'], true)) {
    header('Access-Control-Allow-Origin: ' . $origin);
    header('Vary: Origin');
    header('Access-Control-Allow-Credentials: true');
    header('Access-Control-Allow-Headers: Content-Type');
    header('Access-Control-Allow-Methods: GET, POST, OPTIONS');
}

$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';

if (($_SERVER['REQUEST_METHOD'] ?? 'GET') === 'OPTIONS') {
    http_response_code(204);
    exit;
}

$path = ApiRequest::path();

$db = Database::get();
$appRepo = new AppRepository($db);
$releaseRepo = new ReleaseRepository($db);
$developerRepo = new DeveloperRepository($db);
$storage = new PackageStorage(ROOT);

$limit = isset($_GET['limit']) ? max(0, (int) $_GET['limit']) : 50;
$offset = isset($_GET['offset']) ? max(0, (int) $_GET['offset']) : 0;

if (!class_exists('ApiAbortException')) {
    class ApiAbortException extends RuntimeException
    {
    }
}

if (!function_exists('requireDeveloperId')) {
    function requireDeveloperId(): string
    {
        $developerId = $_SESSION['developer_id'] ?? null;

        if (!is_string($developerId) || $developerId === '') {
            ApiResponse::error('UNAUTHORIZED', 'Not logged in', 401);
            throw new ApiAbortException('Unauthorized');
        }

        return $developerId;
    }
}

if (!function_exists('readJsonBody')) {
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
}

try {
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
            $developerId = $_SESSION['developer_id'] ?? null;
            $user = null;

            if ($userId) {
                $stmt = $db->prepare(
                    'SELECT id, provider, provider_user_id, username, display_name, avatar_url
                     FROM users
                     WHERE id = :id
                     LIMIT 1'
                );

                $stmt->execute([
                    ':id' => $userId,
                ]);

                $user = $stmt->fetch(PDO::FETCH_ASSOC) ?: null;
            }

            if (!$user && !$developerId) {
                ApiResponse::error('UNAUTHORIZED', 'Not logged in', 401);
                return;
            }

            ApiResponse::json([
                'authenticated' => true,
                'developer_id' => $developerId,
                'user' => $user,
            ]);
            return;

        case $path === '/developers/me':
            if ($method !== 'GET') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $developerId = requireDeveloperId();
            $developer = $developerRepo->findById($developerId);

            if ($developer === null) {
                ApiResponse::error('UNAUTHORIZED', 'Developer not found', 401);
                return;
            }

            ApiResponse::json([
                'developer' => $developer,
            ]);
            return;

        case $path === '/keys':
            $developerId = requireDeveloperId();

            if ($method === 'GET') {
                $stmt = $db->prepare(
                    'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
                     FROM public_keys
                     WHERE developer_id = :developer_id
                     ORDER BY created_at DESC'
                );

                $stmt->execute([
                    ':developer_id' => $developerId,
                ]);

                ApiResponse::json([
                    'keys' => $stmt->fetchAll(PDO::FETCH_ASSOC),
                ]);
                return;
            }

            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $payload = readJsonBody();
            $publicKey = trim((string) ($payload['public_key'] ?? ''));

            if ($publicKey === '') {
                ApiResponse::error('VALIDATION_ERROR', 'public_key is required', 422);
                return;
            }

            $key = [
                'key_id' => 'key_' . bin2hex(random_bytes(16)),
                'developer_id' => $developerId,
                'public_key' => $publicKey,
                'fingerprint' => hash('sha256', $publicKey),
                'created_at' => date('c'),
                'revoked_at' => null,
            ];

            $stmt = $db->prepare(
                'INSERT INTO public_keys (
                    key_id,
                    developer_id,
                    public_key,
                    fingerprint,
                    created_at,
                    revoked_at
                ) VALUES (
                    :key_id,
                    :developer_id,
                    :public_key,
                    :fingerprint,
                    :created_at,
                    :revoked_at
                )'
            );

            try {
                $stmt->execute([
                    ':key_id' => $key['key_id'],
                    ':developer_id' => $key['developer_id'],
                    ':public_key' => $key['public_key'],
                    ':fingerprint' => $key['fingerprint'],
                    ':created_at' => $key['created_at'],
                    ':revoked_at' => $key['revoked_at'],
                ]);
            } catch (PDOException $e) {
                if ($e->getCode() === '23000') {
                    ApiResponse::error('KEY_ALREADY_EXISTS', 'Public key already exists', 409);
                    return;
                }

                throw $e;
            }

            ApiResponse::json([
                'key' => $key,
            ], 201);
            return;

        case preg_match('#^/keys/([^/]+)/revoke$#', $path, $matches) === 1:
            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $developerId = requireDeveloperId();
            $keyId = $matches[1];

            $stmt = $db->prepare(
                'SELECT key_id, developer_id, public_key, fingerprint, created_at, revoked_at
                 FROM public_keys
                 WHERE key_id = :key_id
                 LIMIT 1'
            );

            $stmt->execute([
                ':key_id' => $keyId,
            ]);

            $key = $stmt->fetch(PDO::FETCH_ASSOC);
            if ($key === false || $key['developer_id'] !== $developerId) {
                ApiResponse::error('KEY_NOT_FOUND', 'Key not found', 404);
                return;
            }

            if ($key['revoked_at'] === null) {
                $key['revoked_at'] = date('c');

                $update = $db->prepare(
                    'UPDATE public_keys
                     SET revoked_at = :revoked_at
                     WHERE key_id = :key_id'
                );

                $update->execute([
                    ':revoked_at' => $key['revoked_at'],
                    ':key_id' => $keyId,
                ]);
            }

            ApiResponse::json([
                'key' => $key,
            ]);
            return;

        case $path === '/bundle-ids':
            $developerId = requireDeveloperId();

            if ($method === 'GET') {
                $stmt = $db->prepare(
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
                return;
            }

            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
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
                return;
            }

            if ($appName === '') {
                ApiResponse::error('VALIDATION_ERROR', 'app_name is required', 422);
                return;
            }

            $bundle = [
                'bundle_id' => $bundleId,
                'developer_id' => $developerId,
                'app_name' => $appName,
                'status' => 'reserved',
                'created_at' => date('c'),
            ];

            $stmt = $db->prepare(
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
                    return;
                }

                throw $e;
            }

            ApiResponse::json([
                'bundle_id' => $bundle,
            ], 201);
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
} catch (ApiAbortException $e) {
    return;
} catch (Throwable $e) {
    ApiResponse::error('INTERNAL_ERROR', 'Internal server error', 500);
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
