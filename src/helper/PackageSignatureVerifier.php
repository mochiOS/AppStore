<?php

class PackageSignatureVerifier
{
    public function verify(string $packagePath): array
    {
        if (!is_file($packagePath)) {
            throw new RuntimeException('Package file not found');
        }

        $verifyPath = $this->copyToTemporaryPkg($packagePath);

        try {
            $result = $this->runMsignVerify($verifyPath);
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

    private function copyToTemporaryPkg(string $packagePath): string
    {
        $tmp = tempnam(sys_get_temp_dir(), 'msign_verify_');

        if ($tmp === false) {
            throw new RuntimeException('Failed to create temporary file');
        }

        $tmpPkg = $tmp . '.pkg';
        @unlink($tmp);

        if (!copy($packagePath, $tmpPkg)) {
            throw new RuntimeException('Failed to prepare package for verification');
        }

        return $tmpPkg;
    }

    private function runMsignVerify(string $packagePath): array
    {
        $descriptors = [
            0 => ['pipe', 'r'],
            1 => ['pipe', 'w'],
            2 => ['pipe', 'w'],
        ];

        $process = proc_open(
            ['msign', 'verify', $packagePath],
            $descriptors,
            $pipes
        );

        if (!is_resource($process)) {
            throw new RuntimeException('Failed to execute msign');
        }

        fclose($pipes[0]);

        $stdout = stream_get_contents($pipes[1]);
        $stderr = stream_get_contents($pipes[2]);

        fclose($pipes[1]);
        fclose($pipes[2]);

        $exitCode = proc_close($process);

        return [
            'exit_code' => $exitCode,
            'output' => trim((string)$stdout . "\n" . (string)$stderr),
        ];
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