<?php

class ApiResponse
{
    public static function json(array $payload, int $status = 200): void
    {
        http_response_code($status);
        header('Content-Type: application/json; charset=utf-8');
        echo json_encode(
            $payload,
            JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE
        );
    }

    public static function error(string $code, string $message, int $status): void
    {
        self::json([
            'error' => [
                'code' => $code,
                'message' => $message,
            ],
        ], $status);
    }

    public static function streamFile(string $path, string $filename): void
    {
        if (!is_file($path)) {
            self::error('RELEASE_NOT_FOUND', 'Release not found', 404);
            return;
        }

        http_response_code(200);
        header('Content-Type: application/octet-stream');
        header('Content-Length: ' . filesize($path));
        header('Content-Disposition: attachment; filename="' . addslashes($filename) . '"');

        readfile($path);
    }
}


