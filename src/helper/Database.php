<?php

class Database
{
    public static function get(): PDO
    {
        static $pdo = null;

        if ($pdo === null) {
            $pdo = new PDO(
                'sqlite:' . __DIR__ . '/../../data/store.db'
            );

            $pdo->setAttribute(
                PDO::ATTR_ERRMODE,
                PDO::ERRMODE_EXCEPTION
            );
        }

        return $pdo;
    }
}

?>