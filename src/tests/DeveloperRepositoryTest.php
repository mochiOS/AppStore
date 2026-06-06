<?php

it('creates developer tables during migration', function (): void {
    $pdo = Database::get();
    $tables = [];

    $stmt = $pdo->query(
        "SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name"
    );

    foreach ($stmt->fetchAll(PDO::FETCH_COLUMN) as $tableName) {
        $tables[] = $tableName;
    }

    $expectedTables = [
        'developers',
        'oauth_links',
        'public_keys',
        'bundle_ids',
        'developer_apps',
        'developer_releases',
        'revocations',
    ];

    foreach ($expectedTables as $expectedTable) {
        assertTrue(
            in_array($expectedTable, $tables, true),
            'expected migration to create table ' . $expectedTable
        );
    }
});

it('finds the same developer for the same oauth subject', function (): void {
    $repository = new DeveloperRepository(Database::get());

    $first = $repository->findOrCreateByOAuth('github', '123456');
    $second = $repository->findOrCreateByOAuth('github', '123456');

    assertTrue(str_starts_with($first['developer_id'], 'dev_'));
    assertSame($first['developer_id'], $second['developer_id']);
    assertSame('active', $first['status']);
});

it('stores oauth links with hashed subject values', function (): void {
    $repository = new DeveloperRepository(Database::get());
    $developer = $repository->findOrCreateByOAuth('github', '987654');

    $stmt = Database::get()->prepare(
        'SELECT provider_subject_hash
         FROM oauth_links
         WHERE developer_id = :developer_id
           AND provider = :provider
         LIMIT 1'
    );

    $stmt->execute([
        ':developer_id' => $developer['developer_id'],
        ':provider' => 'github',
    ]);

    $storedHash = $stmt->fetchColumn();

    assertTrue(is_string($storedHash) && $storedHash !== '');
    assertTrue($storedHash !== '987654');
    assertSame(
        hash_hmac('sha256', '987654', ''),
        $storedHash
    );
});
