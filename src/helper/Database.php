<?php

class Database
{
    private static array $pdoByDsn = [];

    public static function get(): PDO
    {
        $dataDir = Paths::dataDir();
        $dsn = 'sqlite:' . $dataDir . '/store.db';

        if (!isset(self::$pdoByDsn[$dsn])) {
            if (!is_dir($dataDir)) {
                mkdir($dataDir, 0777, true);
            }

            $pdo = new PDO($dsn);

            $pdo->setAttribute(
                PDO::ATTR_ERRMODE,
                PDO::ERRMODE_EXCEPTION
            );

            $pdo->exec('PRAGMA foreign_keys = ON');

            self::$pdoByDsn[$dsn] = $pdo;
        }

        return self::$pdoByDsn[$dsn];
    }

    public static function reset(): void
    {
        self::$pdoByDsn = [];
    }
}


