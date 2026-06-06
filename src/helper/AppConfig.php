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

        $shared = [];

        foreach ($config as $key => $value) {
            if ($key === 'env' || is_array($value)) {
                continue;
            }

            $shared[$key] = $value;
        }

        return $config[$env] + $shared + [
            'env' => $env,
        ];
    }
}
