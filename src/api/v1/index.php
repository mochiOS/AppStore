<?php

try {
    /** @var ApiContext $context */
    $context = require __DIR__ . '/bootstrap.php';

    $routes = [
        require __DIR__ . '/routes/public_apps.php',
        require __DIR__ . '/routes/auth.php',
        require __DIR__ . '/routes/keys.php',
        require __DIR__ . '/routes/bundle_ids.php',
        require __DIR__ . '/routes/developer_apps.php',
        require __DIR__ . '/routes/developer_releases.php',
        require __DIR__ . '/routes/admin_releases.php',
        require __DIR__ . '/routes/teams.php',
    ];

    foreach ($routes as $route) {
        if ($route($context)) {
            return;
        }
    }
} catch (ApiAbortException $e) {
    return;
} catch (Throwable $e) {
    ApiResponse::error('INTERNAL_ERROR', 'Internal server error', 500);
    return;
}

ApiResponse::error('NOT_FOUND', 'Not found', 404);
