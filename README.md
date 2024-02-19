# Transmit

![Transmit](https://github.com/kasbuunk/transmit/assets/20124087/fec16493-9561-4730-bc2c-0d2d929682fe)

Transmit is a program designed for reliable scheduling and transmission of delayed and periodic messages. It is built with a modular architecture and agnostic design, allowing for seamless integration into various deployment environments.

Key Features:

- Flexible Scheduling: Supports Delayed, Interval, and Cron schedules, with provided number of iterations.
- Reliable Delivery: Ensures messages reach their destination through robust transmission adapters with fault-tolerant exactly-once delivery.
- Data Integrity: Secures message data during transmission and storage with configurable adapters appropriate for its deployment environment, like in-memory and Postgres.
- Adaptable Deployment: Runs as a standalone binary, Docker container, or Kubernetes microservice.
- Comprehensive Configuration: Provides a dedicated algebraic data structure for correct configuration.
- Observability: Integrates with Prometheus metrics for insights into program behaviour and performance.
- Health Checks: Implements the [gRPC Health Checking Protocol](https://github.com/grpc/grpc/blob/master/doc/health-checking.md) for monitoring service health.
- Graceful Shutdown: Handles termination signals (SIGINT, SIGTERM) for proper program closure.
- Rigorous Testing: Emphasizes thorough unit, integration and end-to-end testing practices to ensure reliability and stability.

## Installation

### Standalone binary

Currently, the defined binary application only supports a postgres repository and nats transmitter adapters. It is hence required that these are running and accessible on the host and ports defined in your provided configuration file.

For convenience, these dependencies are provided in the `./docker-compose.yaml` file at the root of the repository.

```sh
# Checkout the repository.
git checkout https://github.com/kasbuunk/transmit
cp transmit

# Run dependencies (verify no conflicting pre-existing processes listen on these ports).
make setup

# Install
# The Rust toolchain includes cargo can be installed [here](https://www.rust-lang.org/tools/install).
cargo install transmit

# Copy the configuration file.
cp sample.ron config.ron # Adjust to access nats and postgres dependencies.

# Run the program.
transmit config.ron
```

### Docker-compose

To run the entire program and its dependencies in docker-compose, refer to the `tests` directory for a correct and continuously tested example configuration.

### Kubernetes

The primary deployment environment for which this program is designed, is to run as a microservice in a Kubernetes cluster.

#### Helm

To deploy with helm, run the following.

```sh
# Add the helm repository.
helm repo add transmit https://kasbuunk.github.io/transmit

# Install the release. N.B.: make sure the required dependencies (Nats and Postgres)
# are deployed and accepting connections on the configured ports, with the configurable
# Postgres connection credentials.
helm install transmit transmit/transmit --values tests/values.yaml # Replace with your configuration.
```

## Usage

Usage depends on the configured transport adapter and chosen `Message` variant. 

Supported transport adapters:

- gRPC: Provided that the program is running and accessible through a gRPC server, use the protobuf-generated client for the programming language, located in the `proto/client/{LANGUAGE}`. Initialise the client with the host and port that the Tranmit service is running on. Next, build and send gRPC requests with the client.

Supported `Message` variants:

- `NatsEvent`: Provided that a message was scheduled to be transmitted, connect to the Nats server configured as a transmission dependency of the Transmit deployment and assert the expected scheduled events are published.

For example, if Nats is running in Kubernetes, run in separate terminals:

Prerequisites: `kubectl` and `nats` CLIs.

```sh
# Make nats accessible from the host.
kubectl port-forward svc/nats 4222:4222
```

```sh
# Observe and assert the expected events are published when scheduled.
nats sub -s "nats://localhost:4222" "MYSUBJECT.details"
```

### Go client

Please refer to the `client/go/client_test.go` file for a simple, correct test how to interact with the Transmit program.

### Other languages

The `proto/transmit.proto` file contains the necessary types and gRPC definitions in order to generate the protobuf and gRPC client code for most popular programming languages.

## Design

The program applies careful separation of concerns. It is designed in the first place to be agnostic to its deployment environment.

The application has a set of loosely-coupled components. Each is defined by a trait in the `contract` module, such that components are properly unit-tested. The compose to a suitable set of adapters be implemented across an array of deployment environments.

- Scheduler
- Repository
- Transmitter
- Transport
- Config

### Scheduler

The scheduler is the core in the program that defines the domain behaviour. It is responsible to determine what messages ought to be transmitted, when to do so and negotiates state retrieval and management of the scheduled transmissions with the repository.

Supported `Schedule` variants:

- `Delayed`: schedule transmission at a given time.
- `Interval`: schedule transmission, starting at a given time and then repeated with a given interval duration.
- `Cron`: schedule transmission, starting after a given time and then repeated with a given cron schedule.

### Repository

The repository is responsible for executing the state updates as commanded by the scheduler.

Supported adapters:

- In-memory: a simple repository that runs in-memory and drops all state when it exits.
- [Postgres](https://www.postgresql.org): a production-ready adapter that manages state by means of a given Postgres database connection.

### Transmitter

For each `Message` variant, a transmission adapter is implemented to broker transmission. The transmitter is responsible to send the messages and report if transmission was successful, such that the scheduler can remain agnostic of the implementation details. 

Supported adapters:

- [Nats](https://nats.io): a Cloud-Native event bus.

### Transport

The transport adapter is responsible for incoming message parsing and brokerage between outside invokers and the domain core, i.e. the Scheduler.

Supported transports:

- [gRPC](https://grpc.io): transmissions can be scheduled via its gRPC interface. This provides type-safe language interoperabile communication over a network.

### Config

The `config` module provides an algebraic data structure that fully models the valid state of the system at startup. As such, the program will only start running if configured precisely with:

- Exactly one transport adapter.
- Exactly one repository adapter.
- Exactly one transmission adapter per `Message` variant.
- Several other self-explanatory configuration fields.

## Contributing

Any feedback and help with development is greatly appreciated. Please feel free to open issues or pull requests for feature requests, bug reports, installation issues or any other questions you may have.

## Roadmap

The roadmap lists many features, additional `Message` variants and targeted deployment environments, as well as other interesting features that make the state management of the Transmit program much more powerful. The roadmap is expected to change its contents and priorities. It is not ready to be shared publicly as of the current state of affairs.
