#include <algorithm>
#include <chrono>
#include <cmath>
#include <iostream>
#include <memory>
#include <string>

#include <grpc/grpc.h>
#include <grpcpp/server.h>
#include <grpcpp/server_builder.h>
#include <grpcpp/server_context.h>
#include <grpcpp/security/server_credentials.h>
#include "geom_kernel.grpc.pb.h"
#include "oce_interface.hpp"

using grpc::Server;
using grpc::ServerBuilder;
using grpc::ServerContext;
using grpc::ServerReader;
using grpc::ServerReaderWriter;
using grpc::ServerWriter;
using grpc::Status;
using grpc::StatusCode;
using std::chrono::system_clock;
using namespace geom_kernel;

gp_Pnt GetPoint(const Point3Msg &pt)
{
	gp_Pnt result(pt.x(), pt.y(), pt.z());
	return result;
}

void handle_eptr(std::exception_ptr eptr) // passing by value is ok
{
	try
	{
		if (eptr)
		{
			std::rethrow_exception(eptr);
		}
	}
	catch (const std::exception &e)
	{
		std::cout << "Caught exception \"" << e.what() << "\"\n";
	}
}

class GeomKernelImpl final : public GeometryKernel::Service
{
public:
	explicit GeomKernelImpl() {}

	Status MakePrism(ServerContext *context, const MakePrismInput *request, MakePrismOutput *response) override
	{
		Status result(StatusCode::UNKNOWN, "default");
		if (request != nullptr && response != nullptr)
		{
			gp_Pnt firstPt = GetPoint(request->firstpt());
			gp_Pnt secondPt = GetPoint(request->secondpt());
			double width = request->width();
			double height = request->height();
			std::vector<double> positions;
			std::vector<uint64_t> indices;
			try
			{
				oce_interface::make_prism(firstPt, secondPt, width, height, positions, indices);
			}
			catch (...)
			{
				std::exception_ptr p = std::current_exception();
				handle_eptr(p);
			}
			*response->mutable_positions() = {positions.begin(), positions.end()};
			*response->mutable_indices() = {indices.begin(), indices.end()};
			result = Status::OK;
		}
		else
		{
			std::cout << "Invalid args" << std::endl;
			result = Status(StatusCode::INVALID_ARGUMENT, "args were null");
		}
		return result;
	}
};

void RunServer()
{
	std::string server_address("0.0.0.0:5000");
	GeomKernelImpl service;

	ServerBuilder builder;
	builder.AddListeningPort(server_address, grpc::InsecureServerCredentials());
	builder.RegisterService(&service);
	std::unique_ptr<Server> server(builder.BuildAndStart());
	std::cout << "Server listening on " << server_address << std::endl;
	server->Wait();
}

int main(int argc, char **argv)
{
	RunServer();
	return 0;
}
