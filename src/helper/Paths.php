<?php

class Paths
{
    public static function repoRoot(): string
    {
        return dirname(__DIR__, 2);
    }

    public static function dataDir(): string
    {
        $override = getenv('APPSTORE_DATA_DIR');
        if ($override !== false && $override !== '') {
            return rtrim($override, '/');
        }

        return self::repoRoot() . '/data';
    }
}

