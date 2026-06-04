PHP = php
PUBLIC_PORT = 8000
API_PORT = 8001

run:
	@cd src/api && $(PHP) -S localhost:$(API_PORT) & \
	cd src/public && $(PHP) -S localhost:$(PUBLIC_PORT)

api:
	@cd src/api && $(PHP) -S localhost:$(API_PORT)

public:
	@cd src/public && $(PHP) -S localhost:$(PUBLIC_PORT)

.PHONY: run api public
