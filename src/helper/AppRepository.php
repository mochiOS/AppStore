<?php

class AppRepository
{
    public function __construct(
        private PDO $db
    ) {}

    public function findAll(): array
    {
        $stmt = $this->db->query(
            'SELECT * FROM apps ORDER BY name'
        );

        return $stmt->fetchAll(PDO::FETCH_ASSOC);
    }
}

?>