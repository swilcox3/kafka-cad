#include <vector>
#include "gp_Pnt.hxx"

namespace oce_interface
{
void make_prism(gp_Pnt first_pt, gp_Pnt second_pt, double width, double height, std::vector<double> &outPositions, std::vector<uint64_t> &outIndices);
}