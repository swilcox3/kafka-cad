cmake_minimum_required(VERSION 3.14.1)

project(geom-server VERSION 0.1.0)
set(CMAKE_CXX_STANDARD 17)

# Get gRPC
include(FetchContent)
FetchContent_Declare(
  gRPC
  GIT_REPOSITORY https://github.com/grpc/grpc
  GIT_TAG v1.27.2
  )
FetchContent_MakeAvailable(gRPC)

set(_PROTOBUF_LIBPROTOBUF $<TARGET_FILE:libprotobuf>)
set(_PROTOBUF_PROTOC $<TARGET_FILE:protoc>)
set(_GRPC_CPP_PLUGIN_EXECUTABLE $<TARGET_FILE:grpc_cpp_plugin>)

include("/usr/local/lib/cmake/opencascade/OpenCASCADEConfig.cmake")
include_directories(${CMAKE_CURRENT_BINARY_DIR} inc ${OpenCASCADE_INCLUDE_DIR} ${gRPC_INCLUDE_DIRS})

get_filename_component(geom_proto "../../proto/geom.proto" ABSOLUTE)
get_filename_component(geom_proto_path "${geom_proto}" PATH)

get_filename_component(gm_proto "../../proto/geom_kernel.proto" ABSOLUTE)
get_filename_component(gm_proto_path "${gm_proto}" PATH)

set(geom_proto_srcs "${CMAKE_CURRENT_BINARY_DIR}/geom.pb.cc")
set(geom_proto_hdrs "${CMAKE_CURRENT_BINARY_DIR}/geom.pb.h")

set(gm_proto_srcs "${CMAKE_CURRENT_BINARY_DIR}/geom_kernel.pb.cc")
set(gm_proto_hdrs "${CMAKE_CURRENT_BINARY_DIR}/geom_kernel.pb.h")
set(gm_grpc_srcs "${CMAKE_CURRENT_BINARY_DIR}/geom_kernel.grpc.pb.cc")
set(gm_grpc_hdrs "${CMAKE_CURRENT_BINARY_DIR}/geom_kernel.grpc.pb.h")

add_custom_command(
      OUTPUT "${geom_proto_srcs}" "${geom_proto_hdrs}" "${gm_proto_srcs}" "${gm_proto_hdrs}" "${gm_grpc_srcs}" "${gm_grpc_hdrs}"
      COMMAND ${_PROTOBUF_PROTOC}
      ARGS --grpc_out "${CMAKE_CURRENT_BINARY_DIR}"
        --cpp_out "${CMAKE_CURRENT_BINARY_DIR}"
        -I "${gm_proto_path}"
        -I "${geom_proto_path}"
        --plugin=protoc-gen-grpc="${_GRPC_CPP_PLUGIN_EXECUTABLE}"
        "${gm_proto}"
        "${geom_proto}"
      DEPENDS "${geom_proto}" "${gm_proto}")

# Include generated *.pb.h files
add_executable(geom_server src/geom_server.cpp src/oce_interface.cpp ${geom_proto_srcs} ${gm_proto_srcs} ${gm_grpc_srcs})
target_link_libraries(geom_server grpc++_unsecure ${OpenCASCADE_LIBRARIES})
install(TARGETS geom_server DESTINATION ${CMAKE_INSTALL_PREFIX})
