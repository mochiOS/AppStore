<?php

define('ROOT', dirname(__DIR__) . '/');

require_once ROOT . 'helper/Paths.php';
require_once ROOT . 'helper/Database.php';
require_once ROOT . 'helper/AdminRepository.php';

function usage(): never
{
    fwrite(STDERR, <<<TXT
Usage:
  make admin add <developer_id> [role]
  make admin del <developer_id>

Examples:
  make admin add 019b9b17-6f1e-7d18-8a62-9306c63e41a2
  make admin add 019b9b17-6f1e-7d18-8a62-9306c63e41a2 owner
  make admin del 019b9b17-6f1e-7d18-8a62-9306c63e41a2

Roles:
  admin
  owner

TXT);

    exit(1);
}

function requireDeveloperId(?string $value): string
{
    $developerId = trim((string) $value);

    if ($developerId === '') {
        fwrite(STDERR, "error: developer_id is required\n");
        usage();
    }

    if (!preg_match('/^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i', $developerId)) {
        fwrite(STDERR, "error: developer_id is invalid\n");
        exit(1);
    }

    return $developerId;
}

$command = $argv[1] ?? null;

try {
    $db = Database::get();
    $repo = new AdminRepository($db);

    if ($command === 'add') {
        $developerId = requireDeveloperId($argv[2] ?? null);
        $role = trim((string) ($argv[3] ?? 'admin'));

        if (!in_array($role, ['admin', 'owner'], true)) {
            fwrite(STDERR, "error: role must be admin or owner\n");
            exit(1);
        }

        $admin = $repo->add($developerId, $role);

        echo "added admin developer\n";
        echo "developer_id: {$admin['developer_id']}\n";
        echo "role: {$admin['role']}\n";
        echo "created_at: {$admin['created_at']}\n";
        exit(0);
    }

    if ($command === 'del') {
        $developerId = requireDeveloperId($argv[2] ?? null);
        $deleted = $repo->delete($developerId);

        if (!$deleted) {
            echo "admin developer not found\n";
            echo "developer_id: {$developerId}\n";
            exit(0);
        }

        echo "deleted admin developer\n";
        echo "developer_id: {$developerId}\n";
        exit(0);
    }

    usage();
} catch (Throwable $e) {
    fwrite(STDERR, "error: {$e->getMessage()}\n");
    exit(1);
}
