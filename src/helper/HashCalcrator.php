<?php

class HashCalcrator
{
    function fromString(string $str): string {
        return hash('sha256', $str);
    }
}
