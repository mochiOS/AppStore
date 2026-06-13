<?php

class AppConfig
{
    public static function get(): array
    {
        $configPath = __DIR__ . '/../../config/app.php';

        if (!is_file($configPath)) {
            $configPath = __DIR__ . '/../../config/app.example.php';
        }

        $config = require $configPath;

        $env = getenv('APPSTORE_ENV') ?: ($config['env'] ?? 'local');

        if (!isset($config[$env])) {
            throw new RuntimeException('Invalid app environment.');
        }

        $shared = [];

        foreach ($config as $key => $value) {
            if ($key === 'env' || is_array($value)) {
                continue;
            }

            $shared[$key] = $value;
        }

        $resolved = $config[$env] + $shared + [
            'env' => $env,
        ];

        $envOverrides = [
            'APPSTORE_FRONTEND_URL' => 'frontend_url',
            'APPSTORE_API_URL' => 'api_url',
            'APPSTORE_MSIGN_TIMEOUT_SECONDS' => 'msign_timeout_seconds',
            'APPSTORE_MSIGN_MAX_OUTPUT_BYTES' => 'msign_max_output_bytes',
            'APPSTORE_SESSION_COOKIE_NAME' => 'session_cookie_name',
        ];

        foreach ($envOverrides as $envName => $configKey) {
            $value = getenv($envName);

            if ($value !== false && $value !== '') {
                $resolved[$configKey] = $value;
            }
        }

        if (($resolved['env'] ?? '') === 'production' && isset($resolved['allowed_origins']) && is_array($resolved['allowed_origins'])) {
            $resolved['allowed_origins'] = array_values(array_filter(
                $resolved['allowed_origins'],
                static fn ($origin): bool => is_string($origin)
                    && !str_starts_with($origin, 'http://localhost')
                    && !str_starts_with($origin, 'http://127.0.0.1')
            ));
        }

        return $resolved;
    }
}
