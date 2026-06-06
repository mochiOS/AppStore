<?php

$GLOBALS['APPSTORE_TESTS'] = [];

function it(string $name, callable $fn): void
{
    $GLOBALS['APPSTORE_TESTS'][] = [
        'name' => $name,
        'fn' => $fn,
    ];
}

function assertSame(mixed $expected, mixed $actual, string $message = ''): void
{
    if ($expected !== $actual) {
        $details = $message !== '' ? $message . ' ' : '';
        throw new RuntimeException($details . 'expected ' . var_export($expected, true) . ' got ' . var_export($actual, true));
    }
}

function assertTrue(bool $value, string $message = ''): void
{
    assertSame(true, $value, $message);
}

function assertContains(string $needle, string $haystack, string $message = ''): void
{
    if (!str_contains($haystack, $needle)) {
        $details = $message !== '' ? $message . ' ' : '';
        throw new RuntimeException($details . 'expected to contain ' . var_export($needle, true) . ' in ' . var_export($haystack, true));
    }
}

function assertFileExistsStrict(string $path, string $message = ''): void
{
    if (!is_file($path)) {
        $details = $message !== '' ? $message . ' ' : '';
        throw new RuntimeException($details . 'missing file ' . $path);
    }
}

function decodeJson(string $json): array
{
    $decoded = json_decode($json, true);
    if (!is_array($decoded)) {
        throw new RuntimeException('Invalid JSON: ' . $json);
    }

    return $decoded;
}

function apiRequest(string $path, array $query = [], string $method = 'GET'): array
{
    return apiJsonRequest($path, $query, $method, null, []);
}

function apiJsonRequest(
    string $path,
    array $query = [],
    string $method = 'GET',
    ?array $jsonBody = null,
    array $session = []
): array {
    $_SERVER['REQUEST_METHOD'] = $method;
    $_SERVER['REQUEST_URI'] = $path . ($query === [] ? '' : '?' . http_build_query($query));
    $_GET = $query;
    $_POST = [];

    if (session_status() !== PHP_SESSION_ACTIVE) {
        session_start();
    }

    $_SESSION = $session;
    $GLOBALS['APPSTORE_TEST_INPUT'] = $jsonBody === null
        ? null
        : json_encode($jsonBody, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE);

    http_response_code(200);
    ob_start();
    require __DIR__ . '/../api/v1/index.php';
    $body = ob_get_clean();
    $status = http_response_code();
    unset($GLOBALS['APPSTORE_TEST_INPUT']);

    return [
        'status' => $status === false ? 200 : $status,
        'body' => (string) $body,
    ];
}

