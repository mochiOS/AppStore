PHP = php
PORT = 8000

run:
	@cd src && $(PHP) -S localhost:$(PORT)

.PHONY: run