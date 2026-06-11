<?php

if (!defined('ROOT')) {
    define('ROOT', __DIR__ . '/../../');
}

require_once ROOT . 'helper/Paths.php';
require_once ROOT . 'helper/Database.php';
require_once ROOT . 'helper/ApiRequest.php';
require_once ROOT . 'helper/ApiResponse.php';
require_once ROOT . 'helper/AppConfig.php';
require_once __DIR__ . '/../cors.php';

$appConfig = AppConfig::get();

if (session_status() !== PHP_SESSION_ACTIVE) {
    session_name((string) ($appConfig['session_cookie_name'] ?? 'mochios_appstore_session'));

    session_set_cookie_params([
        'lifetime' => 0,
        'path' => '/',
        'domain' => '',
        'secure' => ($appConfig['env'] ?? 'local') === 'production',
        'httponly' => true,
        'samesite' => 'Lax',
    ]);

    session_start();
}

if (empty($_SESSION['csrf_token']) || !is_string($_SESSION['csrf_token'])) {
    $_SESSION['csrf_token'] = bin2hex(random_bytes(32));
}

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
require_once ROOT . 'helper/PackageSignatureVerifier.php';
require_once ROOT . 'helper/TeamRepository.php';

require_once __DIR__ . '/ApiContext.php';
require_once __DIR__ . '/guards.php';

appstoreApplyCors($appConfig);

$method = ApiRequest::method();

if ($method === 'OPTIONS') {
    http_response_code(204);
    exit;
}

requireValidCsrfToken($appConfig, $method);

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
    packageSignatureVerifier: new PackageSignatureVerifier(
        (string) ($appConfig['msign_path'] ?? '/usr/local/bin/msign'),
        (int) ($appConfig['msign_timeout_seconds'] ?? 10),
        (int) ($appConfig['msign_max_output_bytes'] ?? 65536),
    ),
    teamRepo: new TeamRepository($db),
    appConfig: $appConfig,
    limit: ApiRequest::queryInt('limit', 50, 0),
    offset: ApiRequest::queryInt('offset', 0, 0),
);
