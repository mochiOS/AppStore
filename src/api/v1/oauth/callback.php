<?php

session_start();

require_once __DIR__ . '/../../../helper/OAuth/Provider.php';
require_once __DIR__ . '/../../../helper/OAuth/User.php';
require_once __DIR__ . '/../../../helper/Database.php';

use OAuth\Provider;
use OAuth\User;

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

$redirectUri = $scheme . '://' . $host . '/oauth/callback.php';

$token = oauthPost($config['token_url'], [
    'client_id' => $config['client_id'],
    'client_secret' => $config['client_secret'],
    'code' => $code,
    'redirect_uri' => $redirectUri,
    'grant_type' => 'authorization_code',
]);

$accessToken = $token['access_token'] ?? null;

if (!$accessToken) {
    http_response_code(401);
    echo 'Failed to get access token';
    exit;
}

$rawUser = oauthGet($config['user_url'], $accessToken);
$userData = Provider::normalizeUser($provider, $rawUser);

if (!$userData) {
    http_response_code(401);
    echo 'Failed to get OAuth user';
    exit;
}

$user = User::findOrCreate($userData);

$_SESSION['user_id'] = (int)$user['id'];

header('Location: /');
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