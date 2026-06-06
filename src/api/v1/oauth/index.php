<?php

use Random\RandomException;

session_start();

require_once __DIR__ . '/../../../helper/Paths.php';

$oauthConfig = require  '../../../../config/oauth.php';

$provider = $_GET['provider'] ?? 'github';

$providers = [
    'github' => [
        'auth_url' => 'https://github.com/login/oauth/authorize',
        'scope' => 'read:user',
    ],
    'gitlab' => [
        'auth_url' => 'https://gitlab.com/oauth/authorize',
        'scope' => 'read_user',
    ],
    'google' => [
        'auth_url' => 'https://accounts.google.com/o/oauth2/v2/auth',
        'scope' => 'openid profile',
    ],
];

if (!isset($providers[$provider])) {
    http_response_code(400);
    echo 'Invalid OAuth provider.';
    exit;
}

if (
    !isset($oauthConfig[$provider]) ||
    empty($oauthConfig[$provider]['client_id'])
) {
    http_response_code(500);
    echo 'OAuth provider is not configured.';
    exit;
}

try {
    $state = bin2hex(random_bytes(32));
} catch (RandomException $e) {
    http_response_code(500);
}

$_SESSION['oauth_state'] = $state;
$_SESSION['oauth_provider'] = $provider;

$scheme = (!empty($_SERVER['HTTPS']) && $_SERVER['HTTPS'] !== 'off')
    ? 'https'
    : 'http';

$host = $_SERVER['HTTP_HOST'];

$redirectUri = $scheme . '://' . $host . '/oauth/callback.php';

$params = [
    'client_id' => $oauthConfig[$provider]['client_id'],
    'redirect_uri' => $redirectUri,
    'response_type' => 'code',
    'scope' => $providers[$provider]['scope'],
    'state' => $state,
];

header('Location: ' . $providers[$provider]['auth_url'] . '?' . http_build_query($params));
exit;