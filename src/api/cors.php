<?php

function appstoreAllowedOrigins(array $appConfig): array
{
    $origins = $appConfig['allowed_origins'] ?? [];

    if (!is_array($origins)) {
        return [];
    }

    return array_values(array_filter($origins, static fn ($origin): bool => is_string($origin) && $origin !== ''));
}

function appstoreIsAllowedOrigin(array $appConfig, string $origin): bool
{
    return $origin !== '' && in_array($origin, appstoreAllowedOrigins($appConfig), true);
}

function appstoreApplyCors(array $appConfig): void
{
    $origin = $_SERVER['HTTP_ORIGIN'] ?? '';

    if (!appstoreIsAllowedOrigin($appConfig, $origin)) {
        return;
    }

    header('Access-Control-Allow-Origin: ' . $origin);
    header('Vary: Origin');
    header('Access-Control-Allow-Credentials: true');
    header('Access-Control-Allow-Headers: Content-Type, Authorization, X-CSRF-Token');
    header('Access-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS');
}
