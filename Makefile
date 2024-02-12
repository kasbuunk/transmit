DOCKER_COMPOSE_FILE := docker-compose.yaml
DOCKER_COMPOSE := docker-compose -f $(DOCKER_COMPOSE_FILE)
DOCKER_COMPOSE_FILE_E2E := tests/docker-compose.yaml
DOCKER_COMPOSE_E2E := docker-compose -f $(DOCKER_COMPOSE_FILE_E2E)

.PHONY: setup
setup:
	$(DOCKER_COMPOSE) up -d --build

.PHONY: unittest
unittest:
	cargo test --lib # Unit tests.

.PHONY: teardown
teardown:
	$(DOCKER_COMPOSE) down

.PHONY: test
test: setup unittest teardown

.PHONY: run
run: setup
	cargo run

.PHONY: e2esetup
e2esetup:
	$(DOCKER_COMPOSE_E2E) up -d --build

.PHONY: e2etest
e2etest:
	cargo test --test end_to_end

.PHONY: e2edown
e2edown:
	$(DOCKER_COMPOSE_E2E) down

.PHONY: e2e
e2e: e2esetup e2etest e2edown

.PHONY: e2ek8s
e2ek8s: k8s e2etest k8sdown

.PHONY: k8s
k8s: k8sdown
	kind create cluster --name e2e || echo "Cluster 'e2e' is already running"
	kubectl cluster-info --context kind-e2e
	helm install postgresql bitnami/postgresql --set auth.postgresPassword="postgres" --set auth.database="transmit"
	helm install prometheus bitnami/prometheus --set serverFiles.prometheus.yml.scrape-configs[0].job_name="transmit" --set serverFiles.prometheus.yml.scrape-configs[0].static_configs[0].targets[0]="transmit.default.svc.cluster.local:9090"
	helm install nats nats/nats
	helm install transmit ./chart --values tests/values.yaml
	sleep 120
	kubectl port-forward svc/nats 4222:4222 &
	kubectl port-forward svc/transmit 8080:80 &

.PHONY: k8sdown
k8sdown:
	kind delete cluster --name e2e || echo "No cluster running named 'e2e'"
