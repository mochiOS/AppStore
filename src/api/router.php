<?php

$path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?: '/';
$path = urldecode($path);

$candidate = __DIR__ . $path;
if ($path !== '/' && is_file($candidate)) {
    return false;
}

if ($path === '/' || $path === '/index.html') {
    readfile(__DIR__ . '/index.html');
    return true;
}

if ($path === '/v1' || $path === '/v1/') {
    readfile(__DIR__ . '/v1/index.html');
    return true;
}

if (preg_match('#^/(?:v1/)?(?:apps|search)(?:/|$)#', $path)) {
    require __DIR__ . '/v1/index.php';
    return true;
}

http_response_code(404);
echo 'Not Found';

return true;


