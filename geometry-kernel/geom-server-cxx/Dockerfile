FROM ubuntu:latest as build-img

WORKDIR /usr/geom_server

# Jump through some hoops to get access to CMake 3.14.1 (allows us to automate gRPC build)
RUN apt-get update && apt-get install -y \
  apt-transport-https \
  ca-certificates \
  gnupg \
  software-properties-common \
  wget
RUN wget -O - https://apt.kitware.com/keys/kitware-archive-latest.asc 2>/dev/null | apt-key add -
RUN apt-add-repository 'deb https://apt.kitware.com/ubuntu/ bionic main'

RUN apt-get update && apt-get install -y \
  make \
  cmake \
  git \
  build-essential \
  autoconf \
  libtool \
  binutils-gold \
  pkg-config \
  golang \
  zlib1g-dev \
  libc-ares-dev \
  openssl

RUN git clone https://github.com/adam-higuera/occt.git
# Let's let Docker cache the previous line in case the below fails
# Build OCCT, disabling unused modules so we can avoid pulling in OpenGL and Tcl/Tk as dependencies
RUN cd occt && mkdir build && cd build && cmake \
  -DBUILD_MODULE_Draw:BOOL=False \
  -DBUILD_MODULE_Visualization:BOOL=False \
  -DBUILD_MODULE_ApplicationFramework:BOOL=False \
  -DBUILD_MODULE_DataExchange:BOOL=False \
  .. && make && make install

RUN mkdir geometry-kernel && cd geometry-kernel && mkdir geom-server-cxx
RUN mkdir proto
COPY ./proto/geom.proto ./proto/geom.proto
COPY ./proto/geom_kernel.proto ./proto/geom_kernel.proto
COPY ./geometry-kernel/geom-server-cxx/ ./geometry-kernel/geom-server-cxx

RUN cd geometry-kernel/geom-server-cxx && mkdir build && cd build && cmake ..
RUN cd geometry-kernel/geom-server-cxx/build && make -j $(nproc)

ENV LD_LIBRARY_PATH=/usr/local/lib
EXPOSE 5000
ENTRYPOINT [ "/usr/geom_server/geometry-kernel/geom-server-cxx/build/geom_server" ]




