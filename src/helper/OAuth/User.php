<?php

namespace OAuth;

use Database;
use PDO;
use RuntimeException;

class User
{
    public static function findOrCreate(array $user): array
    {
        $pdo = Database::get();

        $stmt = $pdo->prepare(
            'SELECT * FROM users WHERE provider = :provider AND provider_user_id = :provider_user_id LIMIT 1'
        );

        $stmt->execute([
            ':provider' => $user['provider'],
            ':provider_user_id' => $user['provider_user_id'],
        ]);

        $existing = $stmt->fetch(PDO::FETCH_ASSOC);

        $now = date('c');

        if ($existing) {
            $update = $pdo->prepare(
                'UPDATE users
                 SET username = :username,
                     display_name = :display_name,
                     avatar_url = :avatar_url,
                     updated_at = :updated_at
                 WHERE id = :id'
            );

            $update->execute([
                ':username' => $user['username'],
                ':display_name' => $user['display_name'],
                ':avatar_url' => $user['avatar_url'],
                ':updated_at' => $now,
                ':id' => $existing['id'],
            ]);

            return self::findById((int)$existing['id']);
        }

        $insert = $pdo->prepare(
            'INSERT INTO users (
                provider,
                provider_user_id,
                username,
                display_name,
                avatar_url,
                created_at,
                updated_at
            ) VALUES (
                :provider,
                :provider_user_id,
                :username,
                :display_name,
                :avatar_url,
                :created_at,
                :updated_at
            )'
        );

        $insert->execute([
            ':provider' => $user['provider'],
            ':provider_user_id' => $user['provider_user_id'],
            ':username' => $user['username'],
            ':display_name' => $user['display_name'],
            ':avatar_url' => $user['avatar_url'],
            ':created_at' => $now,
            ':updated_at' => $now,
        ]);

        return self::findById((int)$pdo->lastInsertId());
    }

    public static function findById(int $id): array
    {
        $pdo = Database::get();

        $stmt = $pdo->prepare('SELECT * FROM users WHERE id = :id LIMIT 1');
        $stmt->execute([':id' => $id]);

        $user = $stmt->fetch(PDO::FETCH_ASSOC);

        if (!$user) {
            throw new RuntimeException('User not found');
        }

        return $user;
    }
}