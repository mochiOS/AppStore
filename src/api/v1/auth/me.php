<?php

if (session_status() !== PHP_SESSION_ACTIVE) {
    session_start();
}

require_once __DIR__ . '/../../../helper/Paths.php';
require_once __DIR__ . '/../../../helper/Database.php';

header('Content-Type: application/json');

$userId = $_SESSION['user_id'] ?? null;
$developerId = $_SESSION['developer_id'] ?? null;

if (!$userId && !$developerId) {
    http_response_code(401);
    echo json_encode([
        'authenticated' => false,
        'user' => null,
        'developer_id' => null,
    ]);
    exit;
}

if ($userId) {
    $pdo = Database::get();

    $stmt = $pdo->prepare(
        'SELECT id, provider, provider_user_id, username, display_name, avatar_url
         FROM users
         WHERE id = :id
         LIMIT 1'
    );

    $stmt->execute([
        ':id' => $userId,
    ]);

    $user = $stmt->fetch(PDO::FETCH_ASSOC) ?: null;
} else {
    $user = null;
}

echo json_encode([
    'authenticated' => true,
    'user' => $user,
    'developer_id' => $developerId,
], JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);
