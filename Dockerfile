# Use the official Rust image as the base image
FROM rust:latest

# Install trunk (and any other dependencies you need)
RUN curl https://github.com/thedodd/trunk/releases/download/v0.16.0/trunk-x86_64-unknown-linux-gnu.tar.gz | tar xz -C /usr/local/bin

# Install additional dependencies if needed (e.g., build tools, etc.)
RUN apt-get update && apt-get install -y \
    build-essential \
    libssl-dev \
    pkg-config

# Set the working directory to your project directory
WORKDIR /usr/src/app

# Copy your source code into the container
COPY . .

# Install cargo dependencies (optional but recommended)
RUN cargo install trunk

# Set up the build output directory
CMD ["trunk", "build", "--release"]
