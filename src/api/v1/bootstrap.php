<?php

if (session_status() !== PHP_SESSION_ACTIVE) {
    session_start();
}

if (!defined('ROOT')) {
    define('ROOT', __DIR__ . '/../../');
}

require_once ROOT . 'helper/Paths.php';
require_once ROOT . 'helper/Database.php';
require_once ROOT . 'helper/ApiRequest.php';
require_once ROOT . 'helper/ApiResponse.php';
require_once ROOT . 'helper/AppConfig.php';
require_once ROOT . 'helper/AppRepository.php';
require_once ROOT . 'helper/ReleaseRepository.php';
require_once ROOT . 'helper/AppCatalog.php';
require_once ROOT . 'helper/PackageStorage.php';
require_once ROOT . 'helper/DeveloperRepository.php';
require_once ROOT . 'helper/DeveloperCertificateRepository.php';
require_once ROOT . 'helper/CertificateAuthority.php';
require_once ROOT . 'helper/PublicKeyRepository.php';
require_once ROOT . 'helper/BundleIdRepository.php';
require_once ROOT . 'helper/OAuth/Provider.php';
require_once ROOT . 'helper/DeveloperAppRepository.php';
require_once ROOT . 'helper/DeveloperReleaseRepository.php';
require_once ROOT . 'helper/PackageUploadService.php';
require_once ROOT . 'helper/PackageInspectService.php';
require_once ROOT . 'helper/AdminRepository.php';

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

$method = ApiRequest::method();

if ($method === 'OPTIONS') {
    http_response_code(204);
    exit;
}

$db = Database::get();

$appRepo = new AppRepository($db);
$releaseRepo = new ReleaseRepository($db);
return new ApiContext(
    path: ApiRequest::path(),
    method: $method,
    db: $db,
    appRepo: $appRepo,
    releaseRepo: $releaseRepo,
    appCatalog: new AppCatalog($appRepo, $releaseRepo),
    developerRepo: new DeveloperRepository($db),
    publicKeyRepo: new PublicKeyRepository($db),
    bundleIdRepo: new BundleIdRepository($db),
    developerAppRepo: new DeveloperAppRepository($db),
    developerReleaseRepo: new DeveloperReleaseRepository($db),
    packageUploadService: new PackageUploadService(),
    packageInspectService: new PackageInspectService(),
    certificateRepo: new DeveloperCertificateRepository($db),
    certificateAuthority: CertificateAuthority::fromAppConfig($appConfig),
    storage: new PackageStorage(ROOT),
    adminRepo: new AdminRepository($db),
    appConfig: $appConfig,
    limit: ApiRequest::queryInt('limit', 50, 0),
    offset: ApiRequest::queryInt('offset', 0, 0),
);