PHP = php
PORT = 8000

run:
	@cd src/public && $(PHP) -S localhost:$(PORT)

.PHONY: run