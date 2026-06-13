<?php

class PackageSignatureVerifier
{
    public function __construct(
        private readonly string $msignPath = 'msign',
        private readonly int $timeoutSeconds = 10,
        private readonly int $maxOutputBytes = 65536
    ) {
    }

    public function verify(string $packagePath): array
    {
        return $this->verifyInternal($packagePath, null);
    }

    public function verifyWithPublicKey(string $packagePath, string $publicKey): array
    {
        $publicKeyPath = $this->writeTemporaryPublicKey($publicKey);

        try {
            return $this->verifyInternal($packagePath, $publicKeyPath);
        } finally {
            @unlink($publicKeyPath);
        }
    }

    private function verifyInternal(string $packagePath, ?string $publicKeyPath): array
    {
        if (!is_file($packagePath)) {
            throw new RuntimeException('Package file not found');
        }

        $verifyPath = $this->copyToTemporaryPkg($packagePath);

        try {
            $result = $this->runMsignVerify($verifyPath, $publicKeyPath);
        } finally {
            @unlink($verifyPath);
        }

        if ($result['exit_code'] !== 0) {
            throw new RuntimeException(
                'Package signature verification failed: ' . trim($result['output'])
            );
        }

        $keyId = $this->parseKeyId($result['output']);

        if ($keyId === null || $keyId === '') {
            throw new RuntimeException('msign verify succeeded but key_id was not found');
        }

        return [
            'key_id' => $keyId,
            'output' => $result['output'],
        ];
    }

    private function writeTemporaryPublicKey(string $publicKey): string
    {
        $tmp = tempnam(sys_get_temp_dir(), 'msign_pubkey_');

        if ($tmp === false) {
            throw new RuntimeException('Failed to create temporary public key file');
        }

        if (file_put_contents($tmp, trim($publicKey) . "\n") === false) {
            @unlink($tmp);
            throw new RuntimeException('Failed to write temporary public key file');
        }

        return $tmp;
    }

    private function copyToTemporaryPkg(string $packagePath): string
    {
        $tmp = tempnam(sys_get_temp_dir(), 'msign_verify_');

        if ($tmp === false) {
            throw new RuntimeException('Failed to create temporary file');
        }

        $tmpPkg = $tmp . '.pkg';
        @unlink($tmp);

        if (!copy($packagePath, $tmpPkg)) {
            @unlink($tmpPkg);
            throw new RuntimeException('Failed to prepare package for verification');
        }

        return $tmpPkg;
    }

    private function runMsignVerify(string $packagePath, ?string $publicKeyPath): array
    {
        $this->assertMsignExecutable();

        $descriptors = [
            0 => ['pipe', 'r'],
            1 => ['pipe', 'w'],
            2 => ['pipe', 'w'],
        ];

        $command = [$this->msignPath, 'verify', $packagePath];

        if ($publicKeyPath !== null) {
            $command[] = '--pubkey';
            $command[] = $publicKeyPath;
        }

        $process = proc_open(
            $command,
            $descriptors,
            $pipes
        );

        if (!is_resource($process)) {
            throw new RuntimeException('Failed to execute msign');
        }

        fclose($pipes[0]);

        stream_set_blocking($pipes[1], false);
        stream_set_blocking($pipes[2], false);

        $stdout = '';
        $stderr = '';
        $startedAt = time();
        $timedOut = false;
        $outputLimitExceeded = false;
        $observedExitCode = null;

        while (true) {
            $stdout .= $this->readLimited($pipes[1], $this->remainingOutputBytes($stdout, $stderr));
            $stderr .= $this->readLimited($pipes[2], $this->remainingOutputBytes($stdout, $stderr));

            $status = proc_get_status($process);
            if (!$status['running']) {
                $observedExitCode = is_int($status['exitcode'] ?? null) ? $status['exitcode'] : null;
                break;
            }

            if ($this->remainingOutputBytes($stdout, $stderr) === 0) {
                $outputLimitExceeded = true;
                proc_terminate($process);
                usleep(100000);

                $status = proc_get_status($process);
                if ($status['running']) {
                    proc_terminate($process, 9);
                }

                break;
            }

            if (time() - $startedAt >= max(1, $this->timeoutSeconds)) {
                $timedOut = true;
                proc_terminate($process);
                usleep(100000);

                $status = proc_get_status($process);
                if ($status['running']) {
                    proc_terminate($process, 9);
                }

                break;
            }

            usleep(20000);
        }

        $stdout .= $this->readLimited($pipes[1], $this->remainingOutputBytes($stdout, $stderr));
        $stderr .= $this->readLimited($pipes[2], $this->remainingOutputBytes($stdout, $stderr));

        fclose($pipes[1]);
        fclose($pipes[2]);

        $exitCode = proc_close($process);
        if ($exitCode === -1 && $observedExitCode !== null) {
            $exitCode = $observedExitCode;
        }

        if ($timedOut) {
            return [
                'exit_code' => 124,
                'output' => 'msign verify timed out',
            ];
        }

        if ($outputLimitExceeded) {
            return [
                'exit_code' => 125,
                'output' => 'msign verify output exceeded limit',
            ];
        }

        return [
            'exit_code' => $exitCode,
            'output' => trim((string)$stdout . "\n" . (string)$stderr),
        ];
    }

    private function assertMsignExecutable(): void
    {
        if (str_contains($this->msignPath, '/')) {
            if (!is_file($this->msignPath) || !is_executable($this->msignPath)) {
                throw new RuntimeException('msign executable was not found or is not executable');
            }

            return;
        }

        if ($this->msignPath === '') {
            throw new RuntimeException('msign executable path is empty');
        }
    }

    private function remainingOutputBytes(string $stdout, string $stderr): int
    {
        return max(0, $this->maxOutputBytes - strlen($stdout) - strlen($stderr));
    }

    private function readLimited(mixed $pipe, int $remaining): string
    {
        if ($remaining === 0) {
            return '';
        }

        $chunk = stream_get_contents($pipe, min(8192, $remaining));

        return $chunk === false ? '' : $chunk;
    }

    private function parseKeyId(string $output): ?string
    {
        foreach (preg_split('/\R/', $output) as $line) {
            $line = trim($line);

            if (preg_match('/^key_id:\s*(.+)$/i', $line, $matches) === 1) {
                return trim($matches[1]);
            }

            if (preg_match('/^key-id:\s*(.+)$/i', $line, $matches) === 1) {
                return trim($matches[1]);
            }
        }

        return null;
    }
}
