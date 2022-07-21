FROM rust:1.62-slim-bullseye

# Install the required platform dependencies
RUN apt-get update && \
    apt-get install -y libssl-dev libudev-dev pkg-config zlib1g-dev llvm clang cmake make libprotobuf-dev protobuf-compiler bash curl screen

# Install the Solana CLI tools from https://docs.solana.com/cli/install-solana-cli-tools & default local development configuration
RUN sh -c "$(curl -sSfL https://release.solana.com/v1.10.31/install)"
RUN echo "export PATH=\"/root/.local/share/solana/install/active_release/bin:$PATH\"" >> /root/.bashrc
RUN . /root/.bashrc && solana config set -ul

# Install the SPL token CLI from https://spl.solana.com/token
RUN cargo install spl-token-cli

CMD /bin/bash
