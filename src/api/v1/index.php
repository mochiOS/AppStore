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
require_once ROOT . 'helper/DeveloperCertificateRepository.php';
require_once ROOT . 'helper/CertificateAuthority.php';
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
$certificateRepo = new DeveloperCertificateRepository($db);
$certificateAuthority = CertificateAuthority::fromAppConfig($appConfig);
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

if (!function_exists('requireAdminToken')) {
    function requireAdminToken(array $appConfig): string
    {
        $configuredToken = (string) (getenv('APPSTORE_ADMIN_API_TOKEN') ?: ($appConfig['admin_api_token'] ?? ''));
        $providedToken = (string) ($_SERVER['HTTP_X_ADMIN_TOKEN'] ?? '');

        if ($configuredToken === '' || !hash_equals($configuredToken, $providedToken)) {
            ApiResponse::error('FORBIDDEN', 'Admin token is required', 403);
            throw new ApiAbortException('Forbidden');
        }

        return 'admin';
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

        case $path === '/developer-verifications/me':
            if ($method !== 'GET') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $developerId = requireDeveloperId();
            ApiResponse::json([
                'verification' => $certificateRepo->findVerificationByDeveloperId($developerId),
            ]);
            return;

        case $path === '/developer-verifications/request':
            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $developerId = requireDeveloperId();
            $payload = readJsonBody();
            $note = isset($payload['note']) ? trim((string) $payload['note']) : null;

            ApiResponse::json([
                'verification' => $certificateRepo->requestVerification($developerId, $note),
            ], 201);
            return;

        case $path === '/certificate-requests':
            $developerId = requireDeveloperId();

            if ($method === 'GET') {
                ApiResponse::json([
                    'certificate_requests' => $certificateRepo->listCertificateSigningRequestsByDeveloperId($developerId),
                ]);
                return;
            }

            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $verification = $certificateRepo->findVerificationByDeveloperId($developerId);
            if ($verification === null || $verification['verification_status'] !== 'verified') {
                ApiResponse::error('DEVELOPER_NOT_VERIFIED', 'Developer verification is required', 403);
                return;
            }

            $payload = readJsonBody();
            $csrPem = trim((string) ($payload['csr_pem'] ?? ''));
            if ($csrPem === '') {
                ApiResponse::error('VALIDATION_ERROR', 'csr_pem is required', 422);
                return;
            }

            $csrInfo = $certificateAuthority->parseCsr($csrPem);
            $csr = $certificateRepo->createCertificateSigningRequest($developerId, $csrPem, $csrInfo);

            ApiResponse::json([
                'certificate_request' => $csr,
            ], 201);
            return;

        case $path === '/certificates':
            if ($method !== 'GET') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $developerId = requireDeveloperId();
            ApiResponse::json([
                'certificates' => $certificateRepo->listCertificatesByDeveloperId($developerId),
            ]);
            return;

        case preg_match('#^/certificates/([^/]+)$#', $path, $matches) === 1:
            if ($method !== 'GET') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $developerId = requireDeveloperId();
            $certificate = $certificateRepo->findCertificateById($matches[1]);
            if ($certificate === null || $certificate['developer_id'] !== $developerId) {
                ApiResponse::error('CERTIFICATE_NOT_FOUND', 'Certificate not found', 404);
                return;
            }

            ApiResponse::json([
                'certificate' => $certificate,
            ]);
            return;

        case $path === '/ca/root':
            if ($method !== 'GET') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            if (!$certificateAuthority->isConfigured()) {
                ApiResponse::error('CA_NOT_CONFIGURED', 'Certificate authority is not configured', 503);
                return;
            }

            ApiResponse::json([
                'ca' => [
                    'configured' => true,
                    'fingerprint' => $certificateAuthority->rootFingerprint(),
                    'certificate_pem' => $certificateAuthority->rootCertificatePem(),
                ],
            ]);
            return;

        case preg_match('#^/admin/developers/([^/]+)/verification$#', $path, $matches) === 1:
            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $adminId = requireAdminToken($appConfig);
            $developerId = $matches[1];
            $developer = $developerRepo->findById($developerId);
            if ($developer === null) {
                ApiResponse::error('DEVELOPER_NOT_FOUND', 'Developer not found', 404);
                return;
            }

            $payload = readJsonBody();
            $status = (string) ($payload['verification_status'] ?? '');
            $note = isset($payload['note']) ? trim((string) $payload['note']) : null;
            if (!in_array($status, ['verified', 'rejected'], true)) {
                ApiResponse::error('VALIDATION_ERROR', 'verification_status must be verified or rejected', 422);
                return;
            }

            ApiResponse::json([
                'verification' => $certificateRepo->updateVerification($developerId, $status, $note, $adminId),
            ]);
            return;

        case preg_match('#^/admin/certificate-requests/([^/]+)/issue$#', $path, $matches) === 1:
            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $adminId = requireAdminToken($appConfig);
            if (!$certificateAuthority->isConfigured()) {
                ApiResponse::error('CA_NOT_CONFIGURED', 'Certificate authority is not configured', 503);
                return;
            }

            $csr = $certificateRepo->findCertificateSigningRequestById($matches[1]);
            if ($csr === null) {
                ApiResponse::error('CSR_NOT_FOUND', 'Certificate signing request not found', 404);
                return;
            }

            $verification = $certificateRepo->findVerificationByDeveloperId($csr['developer_id']);
            if ($verification === null || $verification['verification_status'] !== 'verified') {
                ApiResponse::error('DEVELOPER_NOT_VERIFIED', 'Developer verification is required', 403);
                return;
            }

            $issued = $certificateAuthority->issueCertificate($csr['csr_pem']);
            $certificate = $certificateRepo->issueCertificate($csr['csr_id'], $adminId, $issued);

            ApiResponse::json([
                'certificate' => $certificate,
            ], 201);
            return;

        case preg_match('#^/admin/certificate-requests/([^/]+)/reject$#', $path, $matches) === 1:
            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            $adminId = requireAdminToken($appConfig);
            $payload = readJsonBody();
            $reason = isset($payload['reason']) ? trim((string) $payload['reason']) : null;
            $csr = $certificateRepo->rejectCertificateSigningRequest($matches[1], $adminId, $reason);
            if ($csr === null) {
                ApiResponse::error('CSR_NOT_FOUND', 'Certificate signing request not found', 404);
                return;
            }

            ApiResponse::json([
                'certificate_request' => $csr,
            ]);
            return;

        case preg_match('#^/admin/certificates/([^/]+)/revoke$#', $path, $matches) === 1:
            if ($method !== 'POST') {
                ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
                return;
            }

            requireAdminToken($appConfig);
            $payload = readJsonBody();
            $reason = trim((string) ($payload['reason'] ?? ''));
            if ($reason === '') {
                ApiResponse::error('VALIDATION_ERROR', 'reason is required', 422);
                return;
            }

            $certificate = $certificateRepo->revokeCertificate($matches[1], $reason);
            if ($certificate === null) {
                ApiResponse::error('CERTIFICATE_NOT_FOUND', 'Certificate not found', 404);
                return;
            }

            ApiResponse::json([
                'certificate' => $certificate,
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
