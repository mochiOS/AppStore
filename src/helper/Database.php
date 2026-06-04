<?php

class Database
{
    public static function get(): PDO
    {
        static $pdo = null;

        if ($pdo === null) {
            $dataDir = __DIR__ . '/../../data';
            if (!is_dir($dataDir)) {
                mkdir($dataDir, 0777, true);
            }

            $pdo = new PDO(
                'sqlite:' . __DIR__ . '/../../data/store.db'
            );

            $pdo->setAttribute(
                PDO::ATTR_ERRMODE,
                PDO::ERRMODE_EXCEPTION
            );

            $pdo->exec('PRAGMA foreign_keys = ON');
        }

        return $pdo;
    }
}

?>
