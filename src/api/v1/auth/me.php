<?php

session_start();

require_once __DIR__ . '/../../../helper/Paths.php';
require_once __DIR__ . '/../../../helper/Database.php';

header('Content-Type: application/json');

$userId = $_SESSION['user_id'] ?? null;

if (!$userId) {
    http_response_code(401);
    echo json_encode([
        'ok' => false,
        'user' => null,
    ]);
    exit;
}

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

$user = $stmt->fetch(PDO::FETCH_ASSOC);

if (!$user) {
    http_response_code(401);
    echo json_encode([
        'ok' => false,
        'user' => null,
    ]);
    exit;
}

echo json_encode([
    'ok' => true,
    'user' => $user,
], JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES);