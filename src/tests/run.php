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
putenv('APPSTORE_DATA_DIR=' . $tempDataDir);
Database::reset();

ob_start();
require __DIR__ . '/../cli/migrate.php';
ob_end_clean();

require_once __DIR__ . '/PathsTest.php';
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

