FROM ubuntu:24.04

SHELL ["/bin/bash", "-c"]

ARG USER
ARG UID
ARG HOME
ARG RUST_TOOLCHAIN
ARG DEBIAN_FRONTEND=noninteractive

# ------ Setup environment ------
RUN apt-get update
RUN apt-get install --yes --no-install-recommends \
    sudo

RUN echo "$USER *=(ALL:ALL) NOPASSWD: ALL" >> /etc/sudoers

RUN userdel -r ubuntu
RUN useradd -rm -d $HOME -s /bin/bash -g root -G sudo -u $UID $USER
USER $USER
WORKDIR $HOME

RUN sudo mkdir -p ~
RUN touch ~/.bashrc

# ------ Install basic packages ------
RUN sudo apt-get install --yes --no-install-recommends \
    apt-transport-https \
    ca-certificates \
    software-properties-common \
    curl \
    wget

RUN sudo apt-get install --yes --no-install-recommends \
    build-essential \
    nano \
    vim \
    zsh

# ------ Install Rust ------
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain $RUST_TOOLCHAIN -y

# ------ Install Zephyr ------
# Zephyr SDK Prequisites
RUN sudo apt-get install --yes --no-install-recommends \
    git \
    cmake \
    ninja-build gperf \
    ccache \
    dfu-util \
    device-tree-compiler \
    wget \
    python3-venv \
    python3-setuptools \
    python3-tk \
    python3-wheel \
    xz-utils \
    file \
    make \
    gcc \
    gcc-multilib \
    g++-multilib \
    libsdl2-dev \
    libmagic1

RUN sudo add-apt-repository ppa:deadsnakes/ppa
RUN sudo apt-get update && sudo apt-get install --yes --no-install-recommends \
    python3.12 \
    python3.12-dev

RUN python3.12 -m venv $HOME/.venv
RUN echo "source ~/.venv/bin/activate" >> $HOME/.bashrc
ENV PATH="$HOME/.venv/bin:$PATH"

# Install west
RUN pip install west
RUN echo 'export PATH=~/.local/bin:"$PATH"' >> $HOME/.bashrc
ENV PATH="$PATH:~/.local/bin"

# Fetch Zephyr project & modules
RUN west init --mr v3.7.0 $HOME/zephyrproject
WORKDIR $HOME/zephyrproject
RUN west update

RUN pip3 install -r $HOME/zephyrproject/zephyr/scripts/requirements.txt
RUN west zephyr-export

# Install Zephyr SDK
WORKDIR $HOME
RUN wget https://github.com/zephyrproject-rtos/sdk-ng/releases/download/v0.16.8/zephyr-sdk-0.16.8_linux-x86_64.tar.xz
RUN wget -O - https://github.com/zephyrproject-rtos/sdk-ng/releases/download/v0.16.8/sha256.sum | shasum --check --ignore-missing

RUN tar xvf zephyr-sdk-0.16.8_linux-x86_64.tar.xz
RUN rm zephyr-sdk-0.16.8_linux-x86_64.tar.xz
WORKDIR $HOME/zephyr-sdk-0.16.8
RUN ./setup.sh -h -c -t all

RUN sudo apt-get install --yes --no-install-recommends \
    udev

RUN sudo cp ~/zephyr-sdk-0.16.8/sysroots/x86_64-pokysdk-linux/usr/share/openocd/contrib/60-openocd.rules /etc/udev/rules.d
RUN sudo adduser $USER dialout

WORKDIR $HOME

# ------ Install zephyr-rust ------

RUN sudo apt-get install --yes --no-install-recommends \
    git \
    libclang-dev

RUN git clone --recurse-submodules https://github.com/tylerwhall/zephyr-rust.git
WORKDIR $HOME/zephyr-rust
RUN git checkout --recurse-submodules ff51ff709f79b2adbcbd0c34644eab59bce0dc11
WORKDIR $HOME

# Patch zephyr-rust
COPY assets/riscv32imc-unknown-zephyr-elf.json $HOME/data/riscv32imc-unknown-zephyr-elf.json
COPY assets/wrapper.h $HOME/data/wrapper.h
RUN cp $HOME/data/riscv32imc-unknown-zephyr-elf.json $HOME/zephyr-rust/rust/targets/riscv32imc-unknown-zephyr-elf.json
RUN cat $HOME/data/wrapper.h >> $HOME/zephyr-rust/rust/zephyr-sys/wrapper.h
RUN sudo rm -rf $HOME/data

# Create link to zephyr-rust
RUN echo "ln -s $HOME/zephyr-rust $HOME/vnv_heap/zephyr/zephyr-rust" >> $HOME/.bashrc

# ------ Finalize Zephyr installation ------

COPY assets/.zephyrrc $HOME/.zephyrrc
RUN echo "source ~/zephyrproject/zephyr/zephyr-env.sh" >> $HOME/.bashrc

# ------ Install additional development tools ------
RUN sudo apt-get install --yes --no-install-recommends \
    minicom \
    picocom

# ------ Install required python packages ------

COPY evaluation/requirements.txt $HOME/requirements_evaluation.txt
COPY zephyr/vnv_heap_benchmark/requirements.txt $HOME/requirements_benchmark.txt

RUN pip3 install -r $HOME/requirements_evaluation.txt
RUN pip3 install -r $HOME/requirements_benchmark.txt

# ------ Some final touches ------

ENV DOCKER=true
RUN mkdir -p $HOME/vnv_heap

