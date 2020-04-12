# Geometry Kernel

This is a self-contained C++ server around OpenCASCADE, communicating over gRPC.  It exposes methods that Object Definitions or Operations can call, mostly during object updates to recalculate triangulations.  It is also stateless, every RPC call starts its own instance of OpenCASCADE which is torn down after returning the results. 

While the geometry-kernel is in C++, it's recommended to build and run it solely in Docker because the setup is non-trivial.  C++ isn't awesome like Rust is, so cross-platform builds with third-party dependencies are a nightmare to manage.  If you want to try a native build, you'll need to install [OCE](https://github.com/tpaviot/oce), [protobuf](https://github.com/protocolbuffers/protobuf) and [gRPC](https://github.com/grpc/grpc) first, then massage your install so it matches the CMakeLists.txt.


