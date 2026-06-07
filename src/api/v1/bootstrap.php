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

require_once __DIR__ . '/ApiContext.php';
require_once __DIR__ . '/guards.php';

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

if ($method === 'OPTIONS') {
    http_response_code(204);
    exit;
}

$db = Database::get();

return new ApiContext(
    path: ApiRequest::path(),
    method: $method,
    db: $db,
    appRepo: new AppRepository($db),
    releaseRepo: new ReleaseRepository($db),
    developerRepo: new DeveloperRepository($db),
    certificateRepo: new DeveloperCertificateRepository($db),
    certificateAuthority: CertificateAuthority::fromAppConfig($appConfig),
    storage: new PackageStorage(ROOT),
    appConfig: $appConfig,
    limit: isset($_GET['limit']) ? max(0, (int) $_GET['limit']) : 50,
    offset: isset($_GET['offset']) ? max(0, (int) $_GET['offset']) : 0,
);