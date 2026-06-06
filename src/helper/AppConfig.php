<?php

class AppConfig
{
    public static function get(): array
    {
        $config = require __DIR__ . '/../../config/app.php';

        $env = $config['env'] ?? 'local';

        if (!isset($config[$env])) {
            throw new RuntimeException('Invalid app environment.');
        }

        return $config[$env] + [
                'env' => $env,
            ];
    }
}