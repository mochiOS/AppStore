<?php

header('X-AppStore-Router: hit');

require_once __DIR__ . '/../helper/AppConfig.php';
require_once __DIR__ . '/cors.php';

$appConfig = AppConfig::get();
appstoreApplyCors($appConfig);

if (($_SERVER['REQUEST_METHOD'] ?? 'GET') === 'OPTIONS') {
    http_response_code(204);
    return true;
}

$path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?: '/';
$path = urldecode($path);

if ($path === '/' || $path === '/index.html') {
    header('Content-Type: text/html; charset=utf-8');
    readfile(__DIR__ . '/index.html');
    return true;
}

if ($path === '/v1' || $path === '/v1/') {
    header('Content-Type: application/json; charset=utf-8');
    echo json_encode([
        'name' => 'mochiOS API',
        'version' => 'v1',
        'status' => 'ok',
    ], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);
    return true;
}

if (preg_match('#^/(?:v1/)?(?:apps|teams|search|auth|oauth|developers|developer|keys|bundle-ids|developer-verifications|certificate-requests|certificates|ca|admin)(?:/|$)#', $path)) {
    require __DIR__ . '/v1/index.php';
    return true;
}

http_response_code(404);
header('Content-Type: application/json; charset=utf-8');
echo json_encode([
    'error' => [
        'code' => 'NOT_FOUND',
        'message' => 'Not found',
    ],
], JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);

return true;
