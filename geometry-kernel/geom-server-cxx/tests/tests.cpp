#define CATCH_CONFIG_MAIN
#include "catch.hpp"
#include "oce_interface.hpp"

TEST_CASE("Make prism")
{
    gp_Pnt first(0, 0, 0);
    gp_Pnt second(1, 0, 0);
    double width = 1;
    double height = 1;
    std::vector<double> outPos;
    std::vector<uint64_t> outIndices;
    oce_interface::make_prism(first, second, width, height, outPos, outIndices);
	REQUIRE(outPos.size() > 0);
	REQUIRE(outIndices.size() > 0);
}