<?php

class ApiRequest
{
    public static function method(): string
    {
        return strtoupper($_SERVER['REQUEST_METHOD'] ?? 'GET');
    }

    public static function path(): string
    {
        $path = parse_url($_SERVER['REQUEST_URI'] ?? '/', PHP_URL_PATH) ?: '/';
        $path = urldecode($path);
        $path = preg_replace('#/index\.php$#', '/', $path);

        if (str_starts_with($path, '/v1')) {
            $path = substr($path, 3);
            if ($path === '') {
                $path = '/';
            }
        }

        if ($path !== '/') {
            $path = rtrim($path, '/');
            if ($path === '') {
                $path = '/';
            }
        }

        return $path;
    }

    public static function queryInt(string $name, int $default = 0, int $min = 0): int
    {
        if (!isset($_GET[$name]) || $_GET[$name] === '') {
            return $default;
        }

        return max($min, (int) $_GET[$name]);
    }

    public static function queryString(string $name, ?string $default = null): ?string
    {
        if (!isset($_GET[$name])) {
            return $default;
        }

        $value = trim((string) $_GET[$name]);

        return $value === '' ? $default : $value;
    }
}


