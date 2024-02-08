DOCKER_COMPOSE_FILE := docker-compose.yaml
DOCKER_COMPOSE := docker-compose -f $(DOCKER_COMPOSE_FILE)
DOCKER_COMPOSE_FILE_E2E := tests/docker-compose.yaml
DOCKER_COMPOSE_E2E := docker-compose -f $(DOCKER_COMPOSE_FILE_E2E)

.PHONY: deps
deps:
	$(DOCKER_COMPOSE) up -d --build

.PHONY: unittest
unittest:
	cargo test --lib transmit # Unit tests.

.PHONY: teardown
teardown:
	$(DOCKER_COMPOSE) down

.PHONY: test
test: deps unittest teardown

.PHONY: run
run: deps
	cargo run

.PHONY: e2esetup
e2esetup:
	$(DOCKER_COMPOSE_E2E) up -d --build

.PHONY: e2etest
e2etest:
	cargo test --test end_to_end

.PHONY: e2eteardown
e2eteardown:
	$(DOCKER_COMPOSE_E2E) down

.PHONY: e2e
e2e: e2esetup e2etest e2eteardown
