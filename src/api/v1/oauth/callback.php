<?php

if (session_status() !== PHP_SESSION_ACTIVE) {
    session_start();
}

require_once __DIR__ . '/../../../helper/OAuth/Provider.php';
require_once __DIR__ . '/../../../helper/Paths.php';
require_once __DIR__ . '/../../../helper/Database.php';
require_once __DIR__ . '/../../../helper/AppConfig.php';
require_once __DIR__ . '/../../../helper/DeveloperRepository.php';

use OAuth\Provider;

$provider = $_SESSION['oauth_provider'] ?? '';
$expectedState = $_SESSION['oauth_state'] ?? '';

$code = $_GET['code'] ?? '';
$state = $_GET['state'] ?? '';

if (!$provider || !$expectedState || !$code || !$state) {
    http_response_code(400);
    echo 'Invalid OAuth callback';
    exit;
}

if (!hash_equals($expectedState, $state)) {
    http_response_code(400);
    echo 'Invalid OAuth state';
    exit;
}

unset($_SESSION['oauth_state'], $_SESSION['oauth_provider']);

$config = Provider::get($provider);

if (!$config) {
    http_response_code(400);
    echo 'Invalid OAuth provider';
    exit;
}

$scheme = (!empty($_SERVER['HTTPS']) && $_SERVER['HTTPS'] !== 'off') ? 'https' : 'http';
$host = $_SERVER['HTTP_HOST'];

$appConfig = AppConfig::get();

$redirectUri = $appConfig['api_url'] . '/v1/oauth/callback.php';

try {
    $token = oauthPost($config['token_url'], [
        'client_id' => $config['client_id'],
        'client_secret' => $config['client_secret'],
        'code' => $code,
        'redirect_uri' => $redirectUri,
        'grant_type' => 'authorization_code',
    ]);
} catch (Throwable $e) {
    http_response_code(401);
    echo 'OAuth code is invalid or already used. Please start login again.';
    exit;
}

$accessToken = $token['access_token'] ?? null;

if (!$accessToken) {
    http_response_code(401);
    echo 'Failed to get access token';
    exit;
}

try {
    $rawUser = oauthGet($config['user_url'], $accessToken);
} catch (Throwable $e) {
    http_response_code(502);
    echo 'Failed to get OAuth user';
    exit;
}

$providerSubject = Provider::subject($provider, $rawUser);
if ($providerSubject === null) {
    http_response_code(401);
    echo 'Failed to get OAuth user';
    exit;
}

try {
    $developers = new DeveloperRepository(Database::get());
    $developer = $developers->findOrCreateByOAuth($provider, $providerSubject);
} catch (Throwable $e) {
    http_response_code(500);
    if (($appConfig['env'] ?? '') === 'local') {
        echo 'Failed to create developer session: ' . $e->getMessage();
        exit;
    }

    echo 'Failed to create developer session';
    exit;
}

unset($_SESSION['user_id']);
$_SESSION['developer_id'] = $developer['developer_id'];

header('Location: ' . $appConfig['frontend_url'] . '/auth/callback');
exit;

function oauthPost(string $url, array $params): array
{
    $ch = curl_init($url);

    curl_setopt_array($ch, [
        CURLOPT_POST => true,
        CURLOPT_POSTFIELDS => http_build_query($params),
        CURLOPT_RETURNTRANSFER => true,
        CURLOPT_HTTPHEADER => [
            'Accept: application/json',
            'Content-Type: application/x-www-form-urlencoded',
        ],
    ]);

    $body = curl_exec($ch);

    if ($body === false) {
        throw new RuntimeException(curl_error($ch));
    }

    $status = curl_getinfo($ch, CURLINFO_HTTP_CODE);
    curl_close($ch);

    if ($status < 200 || $status >= 300) {
        throw new RuntimeException('OAuth token request failed');
    }

    $json = json_decode($body, true);

    if (!is_array($json)) {
        throw new RuntimeException('Invalid OAuth token response');
    }

    return $json;
}

function oauthGet(string $url, string $accessToken): array
{
    $ch = curl_init($url);

    curl_setopt_array($ch, [
        CURLOPT_RETURNTRANSFER => true,
        CURLOPT_HTTPHEADER => [
            'Authorization: Bearer ' . $accessToken,
            'Accept: application/json',
            'User-Agent: PHP-OAuth-App',
        ],
    ]);

    $body = curl_exec($ch);

    if ($body === false) {
        throw new RuntimeException(curl_error($ch));
    }

    $status = curl_getinfo($ch, CURLINFO_HTTP_CODE);
    curl_close($ch);

    if ($status < 200 || $status >= 300) {
        throw new RuntimeException('OAuth user request failed');
    }

    $json = json_decode($body, true);

    if (!is_array($json)) {
        throw new RuntimeException('Invalid OAuth user response');
    }

    return $json;
}
