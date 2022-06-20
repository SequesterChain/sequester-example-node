# Sequester Example Node

An example implementation of a FRAME-based [Substrate](https://www.substrate.io/) node with integration of the Sequester donations pallet.

## Getting Started

### Rust Setup

First, complete the [basic Rust setup instructions](./docs/rust-setup.md).

### Initialize Environment

This command will initialize the WASM build environment and set up the latest Rust nightly build

```bash
make init
```

## Run

### Single-Node Development Chain

This command will start our single-node development chain with non-persistent state:

```bash
make run-dev
```

Purge the development chain's state:

```bash
make purge-chain
```

> Development chain means that the state of our chain will be in a tmp folder while the nodes are
> running. Also, **alice** account will be authority and sudo account as declared in the
> [genesis state](https://github.com/substrate-developer-hub/substrate-node-template/blob/main/node/src/chain_spec.rs#L49).
> At the same time the following accounts will be pre-funded:
>
> - Alice
> - Bob
> - Alice//stash
> - Bob//stash

### Pallets

We've integrated the [Sequester donations pallet](https://github.com/SequesterChain/pallets/tree/main/donations) into this example chain.

### Testing

In order to test the donations pallet, run the following command:

```bash
make test
```

Additionally, you can use the Substrate Front-end template and send funds between accounts to see Sequester pallet in action.

Instructions to install and run the front-end template can be found [in this Substrate tutorial.](https://docs.substrate.io/tutorials/v3/create-your-first-substrate-chain/#install-the-front-end-template).

The Donations pallet functionality can be verified through the UI by sending a transaction between users and observing events `donations:TxnFeeQueued` and `donations:TxnFeeSubsumed` in the Events UI section.  Every 9 blocks (as configured by our example chain), we will send the accumulated transaction fees to Sequester's account, observed through a `balances:Deposit` event.

### Run in Docker

First, install [Docker](https://docs.docker.com/get-docker/) and
[Docker Compose](https://docs.docker.com/compose/install/).

Then run the following command to start a single node development chain.

```bash
./scripts/docker-run.sh
```

This command will firstly compile your code, and then start a local development network.
