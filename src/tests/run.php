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
require_once __DIR__ . '/../api/cors.php';
require_once __DIR__ . '/../helper/PublicKeyRepository.php';
require_once __DIR__ . '/../helper/PackageInspectService.php';
require_once __DIR__ . '/../helper/PackageSignatureVerifier.php';

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

session_save_path($tempSessionDir);

$originalDataDir = getenv('APPSTORE_DATA_DIR');
$originalAdminToken = getenv('APPSTORE_ADMIN_API_TOKEN');
putenv('APPSTORE_DATA_DIR=' . $tempDataDir);
Database::reset();

putenv('APPSTORE_ADMIN_API_TOKEN=test-admin-token');

ob_start();
require __DIR__ . '/../cli/migrate.php';
ob_end_clean();

$db = Database::get();
$storage = new PackageStorage(Paths::repoRoot());
$placeholderPath = $storage->ensurePlaceholderPackage(
    'data/releases/com.example/0.1.0.pkg',
    "mochiOS placeholder package for com.example 0.1.0\n"
);

$db->prepare(
    'INSERT OR IGNORE INTO apps (
        bundle_id,
        name,
        version,
        developer,
        description,
        icon
    ) VALUES (
        :bundle_id,
        :name,
        :version,
        :developer,
        :description,
        :icon
    )'
)->execute([
    ':bundle_id' => 'com.example',
    ':name' => 'Example App',
    ':version' => '0.1.0',
    ':developer' => 'mochiOS',
    ':description' => 'Example app for AppStore development.',
    ':icon' => null,
]);

$db->prepare(
    'INSERT OR IGNORE INTO releases (
        bundle_id,
        version,
        size,
        sha256,
        changelog,
        download_path,
        created_at
    ) VALUES (
        :bundle_id,
        :version,
        :size,
        :sha256,
        :changelog,
        :download_path,
        :created_at
    )'
)->execute([
    ':bundle_id' => 'com.example',
    ':version' => '0.1.0',
    ':size' => filesize($placeholderPath) ?: 0,
    ':sha256' => hash_file('sha256', $placeholderPath),
    ':changelog' => 'Initial example release.',
    ':download_path' => 'data/releases/com.example/0.1.0.pkg',
    ':created_at' => '2026-06-06T00:00:00+00:00',
]);

require_once __DIR__ . '/PathsTest.php';
require_once __DIR__ . '/ApiTest.php';
require_once __DIR__ . '/SecurityTest.php';

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
