<?php

namespace OAuth;

class Provider
{
    public static function get(string $provider): ?array
    {
        $secrets = require __DIR__ . '/../../../config/oauth.php';

        $providers = [
            'github' => [
                'auth_url' => 'https://github.com/login/oauth/authorize',
                'token_url' => 'https://github.com/login/oauth/access_token',
                'user_url' => 'https://api.github.com/user',
                'scope' => 'read:user',
            ],
            'gitlab' => [
                'auth_url' => 'https://gitlab.com/oauth/authorize',
                'token_url' => 'https://gitlab.com/oauth/token',
                'user_url' => 'https://gitlab.com/api/v4/user',
                'scope' => 'read_user',
            ],
            'google' => [
                'auth_url' => 'https://accounts.google.com/o/oauth2/v2/auth',
                'token_url' => 'https://oauth2.googleapis.com/token',
                'user_url' => 'https://openidconnect.googleapis.com/v1/userinfo',
                'scope' => 'openid profile',
            ],
        ];

        if (!isset($providers[$provider], $secrets[$provider])) {
            return null;
        }

        $config = array_merge($providers[$provider], $secrets[$provider]);

        if (empty($config['client_id']) || empty($config['client_secret'])) {
            return null;
        }

        return $config;
    }

    public static function normalizeUser(string $provider, array $raw): ?array
    {
        return match ($provider) {
            'github' => isset($raw['id']) ? [
                'provider' => 'github',
                'provider_user_id' => (string)$raw['id'],
                'username' => $raw['login'] ?? null,
                'display_name' => $raw['name'] ?? $raw['login'] ?? null,
                'avatar_url' => $raw['avatar_url'] ?? null,
            ] : null,

            'gitlab' => isset($raw['id']) ? [
                'provider' => 'gitlab',
                'provider_user_id' => (string)$raw['id'],
                'username' => $raw['username'] ?? null,
                'display_name' => $raw['name'] ?? $raw['username'] ?? null,
                'avatar_url' => $raw['avatar_url'] ?? null,
            ] : null,

            'google' => isset($raw['sub']) ? [
                'provider' => 'google',
                'provider_user_id' => (string)$raw['sub'],
                'username' => null,
                'display_name' => $raw['name'] ?? null,
                'avatar_url' => $raw['picture'] ?? null,
            ] : null,

            default => null,
        };
    }
}