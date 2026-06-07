<?php

use OAuth\Provider;

if (!function_exists('oauthPost')) {
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
            $error = curl_error($ch);
            curl_close($ch);
            throw new RuntimeException($error);
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
}

if (!function_exists('oauthGet')) {
    function oauthGet(string $url, string $accessToken): array
    {
        $ch = curl_init($url);

        curl_setopt_array($ch, [
            CURLOPT_RETURNTRANSFER => true,
            CURLOPT_HTTPHEADER => [
                'Authorization: Bearer ' . $accessToken,
                'Accept: application/json',
                'User-Agent: mochiOS-AppStore',
            ],
        ]);

        $body = curl_exec($ch);

        if ($body === false) {
            $error = curl_error($ch);
            curl_close($ch);
            throw new RuntimeException($error);
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
}

return function (ApiContext $ctx): bool {
    if ($ctx->path === '/oauth') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $provider = ApiRequest::queryString('provider', 'github');

        $config = Provider::get($provider);
        if ($config === null) {
            ApiResponse::error('OAUTH_PROVIDER_NOT_CONFIGURED', 'OAuth provider is not configured', 500);
            return true;
        }

        try {
            $state = bin2hex(random_bytes(32));
        } catch (Throwable) {
            ApiResponse::error('OAUTH_STATE_FAILED', 'Failed to create OAuth state', 500);
            return true;
        }

        $_SESSION['oauth_state'] = $state;
        $_SESSION['oauth_provider'] = $provider;

        $redirectUri = rtrim($ctx->appConfig['api_url'], '/') . '/v1/oauth/callback';

        $params = [
            'client_id' => $config['client_id'],
            'redirect_uri' => $redirectUri,
            'response_type' => 'code',
            'scope' => $config['scope'],
            'state' => $state,
        ];

        header('Location: ' . $config['auth_url'] . '?' . http_build_query($params));
        return true;
    }

    if ($ctx->path === '/oauth/callback') {
        if ($ctx->method !== 'GET') {
            ApiResponse::error('METHOD_NOT_ALLOWED', 'Method not allowed', 405);
            return true;
        }

        $provider = $_SESSION['oauth_provider'] ?? '';
        $expectedState = $_SESSION['oauth_state'] ?? '';

        $code = ApiRequest::queryString('code', '');
        $state = ApiRequest::queryString('state', '');

        if ($provider === '' || $expectedState === '' || $code === '' || $state === '') {
            http_response_code(400);
            echo 'Invalid OAuth callback';
            return true;
        }

        if (!hash_equals($expectedState, $state)) {
            http_response_code(400);
            echo 'Invalid OAuth state';
            return true;
        }

        unset($_SESSION['oauth_state'], $_SESSION['oauth_provider']);

        $config = Provider::get($provider);
        if ($config === null) {
            http_response_code(400);
            echo 'Invalid OAuth provider';
            return true;
        }

        $redirectUri = rtrim($ctx->appConfig['api_url'], '/') . '/v1/oauth/callback';

        try {
            $token = oauthPost($config['token_url'], [
                'client_id' => $config['client_id'],
                'client_secret' => $config['client_secret'],
                'code' => $code,
                'redirect_uri' => $redirectUri,
                'grant_type' => 'authorization_code',
            ]);
        } catch (Throwable) {
            http_response_code(401);
            echo 'OAuth code is invalid or already used. Please start login again.';
            return true;
        }

        $accessToken = $token['access_token'] ?? null;

        if (!is_string($accessToken) || $accessToken === '') {
            http_response_code(401);
            echo 'Failed to get access token';
            return true;
        }

        try {
            $rawUser = oauthGet($config['user_url'], $accessToken);
        } catch (Throwable) {
            http_response_code(502);
            echo 'Failed to get OAuth user';
            return true;
        }

        $providerSubject = Provider::subject($provider, $rawUser);
        if ($providerSubject === null) {
            http_response_code(401);
            echo 'Failed to get OAuth user';
            return true;
        }

        $providerUsername = Provider::username($provider, $rawUser);

        try {
            $developer = $ctx->developerRepo->findOrCreateByOAuth(
                $provider,
                $providerSubject,
                $providerUsername
            );
        } catch (Throwable $e) {
            http_response_code(500);

            if (($ctx->appConfig['env'] ?? '') === 'local') {
                echo 'Failed to create developer session: ' . $e->getMessage();
                return true;
            }

            echo 'Failed to create developer session';
            return true;
        }

        unset($_SESSION['user_id']);
        $_SESSION['developer_id'] = $developer['developer_id'];

        header('Location: ' . rtrim($ctx->appConfig['frontend_url'], '/') . '/');
        return true;
    }

    return false;
};