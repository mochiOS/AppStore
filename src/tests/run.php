<?php

use Random\RandomException;

require_once __DIR__ . '/../helper/Paths.php';
require_once __DIR__ . '/../helper/Database.php';
require_once __DIR__ . '/../helper/AppRepository.php';
require_once __DIR__ . '/../helper/ReleaseRepository.php';
require_once __DIR__ . '/../helper/AppCatalog.php';
require_once __DIR__ . '/../helper/ApiRequest.php';
require_once __DIR__ . '/../helper/ApiResponse.php';
require_once __DIR__ . '/../helper/PackageStorage.php';
require_once __DIR__ . '/../helper/AppConfig.php';
require_once __DIR__ . '/../helper/DeveloperRepository.php';
require_once __DIR__ . '/../helper/DeveloperCertificateRepository.php';
require_once __DIR__ . '/../helper/CertificateAuthority.php';

require_once __DIR__ . '/Support.php';

try {
    $tempDataDir = sys_get_temp_dir() . '/appstore-tests-' . getmypid() . '-' . bin2hex(random_bytes(4));
} catch (RandomException $e) {
    http_response_code(500);
    echo 'Failed to generate temporary data directory.';
    exit;
}
if (!mkdir($tempDataDir, 0777, true) && !is_dir($tempDataDir)) {
    throw new RuntimeException('Failed to create temp data dir: ' . $tempDataDir);
}

$tempSessionDir = $tempDataDir . '/sessions';
if (!mkdir($tempSessionDir, 0777, true) && !is_dir($tempSessionDir)) {
    throw new RuntimeException('Failed to create temp session dir: ' . $tempSessionDir);
}

$tempCaDir = $tempDataDir . '/ca';
if (!mkdir($tempCaDir, 0777, true) && !is_dir($tempCaDir)) {
    throw new RuntimeException('Failed to create temp ca dir: ' . $tempCaDir);
}

session_save_path($tempSessionDir);

$originalDataDir = getenv('APPSTORE_DATA_DIR');
$originalAdminToken = getenv('APPSTORE_ADMIN_API_TOKEN');
$originalCaCertPath = getenv('APPSTORE_CA_CERT_PATH');
$originalCaKeyPath = getenv('APPSTORE_CA_KEY_PATH');
$originalCaDays = getenv('APPSTORE_CA_CERTIFICATE_DAYS');
putenv('APPSTORE_DATA_DIR=' . $tempDataDir);
Database::reset();

$caPrivateKey = openssl_pkey_new([
    'private_key_type' => OPENSSL_KEYTYPE_RSA,
    'private_key_bits' => 2048,
]);
if ($caPrivateKey === false) {
    throw new RuntimeException('Failed to generate CA private key.');
}

$caCsr = openssl_csr_new([
    'commonName' => 'mochiOS Test Root CA',
], $caPrivateKey, [
    'digest_alg' => 'sha256',
]);
if ($caCsr === false) {
    throw new RuntimeException('Failed to generate CA CSR.');
}

$caCert = openssl_csr_sign($caCsr, null, $caPrivateKey, 3650, [
    'digest_alg' => 'sha256',
], 1);
if ($caCert === false) {
    throw new RuntimeException('Failed to self-sign CA certificate.');
}

$caCertPath = $tempCaDir . '/root-ca.pem';
$caKeyPath = $tempCaDir . '/root-ca.key';

if (!openssl_x509_export_to_file($caCert, $caCertPath)) {
    throw new RuntimeException('Failed to write CA certificate.');
}

if (!openssl_pkey_export_to_file($caPrivateKey, $caKeyPath)) {
    throw new RuntimeException('Failed to write CA private key.');
}

putenv('APPSTORE_ADMIN_API_TOKEN=test-admin-token');
putenv('APPSTORE_CA_CERT_PATH=' . $caCertPath);
putenv('APPSTORE_CA_KEY_PATH=' . $caKeyPath);
putenv('APPSTORE_CA_CERTIFICATE_DAYS=365');

ob_start();
require __DIR__ . '/../cli/migrate.php';
ob_end_clean();

require_once __DIR__ . '/PathsTest.php';
require_once __DIR__ . '/DeveloperRepositoryTest.php';
require_once __DIR__ . '/DeveloperCaTest.php';
require_once __DIR__ . '/ApiTest.php';

$failures = 0;
$results = [];
foreach ($GLOBALS['APPSTORE_TESTS'] as $test) {
    try {
        ($test['fn'])();
        $results[] = [
            'ok' => true,
            'name' => $test['name'],
            'message' => '',
        ];
    } catch (Throwable $e) {
        $failures++;
        $results[] = [
            'ok' => false,
            'name' => $test['name'],
            'message' => $e->getMessage(),
        ];
    }
}

if ($originalDataDir === false) {
    putenv('APPSTORE_DATA_DIR');
} else {
    putenv('APPSTORE_DATA_DIR=' . $originalDataDir);
}

if ($originalAdminToken === false) {
    putenv('APPSTORE_ADMIN_API_TOKEN');
} else {
    putenv('APPSTORE_ADMIN_API_TOKEN=' . $originalAdminToken);
}

if ($originalCaCertPath === false) {
    putenv('APPSTORE_CA_CERT_PATH');
} else {
    putenv('APPSTORE_CA_CERT_PATH=' . $originalCaCertPath);
}

if ($originalCaKeyPath === false) {
    putenv('APPSTORE_CA_KEY_PATH');
} else {
    putenv('APPSTORE_CA_KEY_PATH=' . $originalCaKeyPath);
}

if ($originalCaDays === false) {
    putenv('APPSTORE_CA_CERTIFICATE_DAYS');
} else {
    putenv('APPSTORE_CA_CERTIFICATE_DAYS=' . $originalCaDays);
}

if ($failures > 0) {
    foreach ($results as $result) {
        if ($result['ok']) {
            echo 'PASS ' . $result['name'] . PHP_EOL;
            continue;
        }

        fwrite(STDERR, 'FAIL ' . $result['name'] . ': ' . $result['message'] . PHP_EOL);
    }
    exit(1);
}

foreach ($results as $result) {
    echo 'PASS ' . $result['name'] . PHP_EOL;
}

echo 'All tests passed' . PHP_EOL;
