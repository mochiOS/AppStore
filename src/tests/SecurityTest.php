<?php

function createPackageArchive(array $files, array $tarNames = [], array $tarOptions = []): string
{
    $workDir = sys_get_temp_dir() . '/appstore-pkg-' . bin2hex(random_bytes(4));
    if (!mkdir($workDir, 0777, true) && !is_dir($workDir)) {
        throw new RuntimeException('Failed to create package work dir');
    }

    foreach ($files as $path => $content) {
        $absolute = $workDir . '/' . $path;
        $directory = dirname($absolute);
        if (!is_dir($directory)) {
            mkdir($directory, 0777, true);
        }
        file_put_contents($absolute, $content);
    }

    $packagePath = tempnam(sys_get_temp_dir(), 'appstore-pkg-out-') . '.pkg';
    $names = $tarNames === [] ? array_keys($files) : $tarNames;
    $command = ['tar', ...$tarOptions, '--hard-dereference', '-czf', $packagePath, '-C', $workDir, ...$names];

    $process = proc_open($command, [
        0 => ['pipe', 'r'],
        1 => ['pipe', 'w'],
        2 => ['pipe', 'w'],
    ], $pipes);

    if (!is_resource($process)) {
        throw new RuntimeException('Failed to execute tar');
    }

    fclose($pipes[0]);
    $stderr = stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);

    if (proc_close($process) !== 0) {
        throw new RuntimeException('tar failed: ' . $stderr);
    }

    return $packagePath;
}

function createDirectoryHeavyPackage(int $directoryCount): string
{
    $workDir = sys_get_temp_dir() . '/appstore-dir-pkg-' . bin2hex(random_bytes(4));
    mkdir($workDir . '/app', 0777, true);

    file_put_contents($workDir . '/about.toml', "bundle_id = \"org.mochios.secure\"\nversion = \"1.0.0\"\nname = \"Secure\"\nentry = \"app/main.js\"\n");
    file_put_contents($workDir . '/manifest.toml', "[app]\nid = \"org.mochios.secure\"\n");
    file_put_contents($workDir . '/app/main.js', "console.log('ok');\n");

    $names = [];
    for ($i = 0; $i < $directoryCount; $i++) {
        $name = 'dirs/d' . $i;
        mkdir($workDir . '/' . $name, 0777, true);
        $names[] = $name;
    }

    $names[] = 'about.toml';
    $names[] = 'manifest.toml';
    $names[] = 'app/main.js';

    $packagePath = tempnam(sys_get_temp_dir(), 'appstore-dir-pkg-out-') . '.pkg';
    $command = ['tar', '--hard-dereference', '-czf', $packagePath, '-C', $workDir, ...$names];

    $process = proc_open($command, [
        0 => ['pipe', 'r'],
        1 => ['pipe', 'w'],
        2 => ['pipe', 'w'],
    ], $pipes);

    if (!is_resource($process)) {
        throw new RuntimeException('Failed to execute tar');
    }

    fclose($pipes[0]);
    $stderr = stream_get_contents($pipes[2]);
    fclose($pipes[1]);
    fclose($pipes[2]);

    if (proc_close($process) !== 0) {
        throw new RuntimeException('tar failed: ' . $stderr);
    }

    return $packagePath;
}

function minimalPackageFiles(array $extra = []): array
{
    return [
        'about.toml' => "bundle_id = \"org.mochios.secure\"\nversion = \"1.0.0\"\nname = \"Secure\"\nentry = \"app/main.js\"\n",
        'manifest.toml' => "[app]\nid = \"org.mochios.secure\"\n",
        'app/main.js' => "console.log('ok');\n",
    ] + $extra;
}

it('validates base64 ed25519 public keys for registration', function (): void {
    assertTrue(PublicKeyRepository::isValidEd25519PublicKey(base64_encode(str_repeat('k', 32))));
    assertSame(false, PublicKeyRepository::isValidEd25519PublicKey(''));
    assertSame(false, PublicKeyRepository::isValidEd25519PublicKey(base64_encode(str_repeat('k', 31))));
    assertSame(false, PublicKeyRepository::isValidEd25519PublicKey('ssh-ed25519 AAAA comment'));
});

it('verifies package signatures with the registered public key file', function (): void {
    $dir = sys_get_temp_dir() . '/appstore-msign-' . bin2hex(random_bytes(4));
    mkdir($dir, 0777, true);

    $logPath = $dir . '/args.log';
    $msignPath = $dir . '/msign';
    file_put_contents($msignPath, "#!/bin/sh\nprintf '%s\\n' \"$@\" > " . escapeshellarg($logPath) . "\ncat \"$4\" | grep -q registered-public-key || exit 7\nprintf 'key_id: registered-key\\n'\n");
    chmod($msignPath, 0755);

    $packagePath = tempnam(sys_get_temp_dir(), 'appstore-msign-pkg-');
    file_put_contents($packagePath, 'package');

    $verifier = new PackageSignatureVerifier($msignPath, 2, 4096);
    $result = $verifier->verifyWithPublicKey($packagePath, 'registered-public-key');

    assertSame('registered-key', $result['key_id']);
    assertContains('--pubkey', file_get_contents($logPath));
});

it('keeps localhost out of production cors origins', function (): void {
    $originalEnv = getenv('APPSTORE_ENV');
    putenv('APPSTORE_ENV=production');

    try {
        $config = AppConfig::get();
    } finally {
        if ($originalEnv === false) {
            putenv('APPSTORE_ENV');
        } else {
            putenv('APPSTORE_ENV=' . $originalEnv);
        }
    }

    assertSame(false, in_array('http://localhost:3000', appstoreAllowedOrigins($config), true));
    assertTrue(in_array('https://console.mochios.org', appstoreAllowedOrigins($config), true));
});

it('inspects normal packages and rejects duplicate paths', function (): void {
    $service = new PackageInspectService();
    $normal = createPackageArchive(minimalPackageFiles());
    $inspection = $service->inspect($normal);

    assertSame('org.mochios.secure', $inspection['about']['bundle_id']);
    assertTrue(isset($inspection['hashes']['package_sha256']));
    assertTrue(isset($inspection['hashes']['content_hash']));

    $duplicate = createPackageArchive(
        minimalPackageFiles(),
        ['about.toml', 'about.toml', 'manifest.toml', 'app/main.js']
    );

    try {
        $service->inspect($duplicate);
        throw new RuntimeException('duplicate package path was accepted');
    } catch (RuntimeException $e) {
        assertContains('Duplicate package path', $e->getMessage());
    }
});

it('rejects pax headers and excessive directory entries', function (): void {
    $service = new PackageInspectService();
    $longPath = 'long/' . str_repeat('a', 120) . '.txt';
    $pax = createPackageArchive(
        minimalPackageFiles([$longPath => 'x']),
        [],
        ['--format=posix']
    );

    try {
        $service->inspect($pax);
        throw new RuntimeException('pax package was accepted');
    } catch (RuntimeException $e) {
        assertContains('Only regular files are allowed', $e->getMessage());
    }

    $manyDirectories = createDirectoryHeavyPackage(4105);

    try {
        $service->inspect($manyDirectories);
        throw new RuntimeException('directory-heavy package was accepted');
    } catch (RuntimeException $e) {
        assertContains('too many entries', $e->getMessage());
    }
});
