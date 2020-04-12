#include <iostream>
#include "oce_interface.hpp"
#include "gp_Pnt.hxx"
#include "gp_Dir.hxx"
#include "TopExp_Explorer.hxx"
#include "TopoDS.hxx"
#include "Poly_Triangulation.hxx"
#include "TColgp_Array1OfPnt.hxx"
#include "BRepPrimAPI_MakeBox.hxx"
#include "BRepTools.hxx"
#include "BRepMesh_IncrementalMesh.hxx"
gp_Pnt GetVertex(gp_Pnt base, gp_Vec dir)
{
    return gp_Pnt(base.X() + dir.X(), base.Y() + dir.Y(), base.Z() + dir.Z());
}

void printPnt(std::ostream &out, const gp_Pnt &pt)
{
    out << pt.X() << "," << pt.Y() << "," << pt.Z() << std::endl;
}

void pushPt(std::vector<double> &outPositions, const gp_Pnt &pt)
{
    outPositions.push_back(pt.X());
    outPositions.push_back(pt.Y());
    outPositions.push_back(pt.Z());
}

void oce_interface::make_prism(gp_Pnt gp_first, gp_Pnt gp_second, double width, double height, std::vector<double> &outPositions, std::vector<uint64_t> &outIndices)
{
    std::cout << "Make prism" << std::endl;
    gp_Vec dir(gp_first, gp_second);
    gp_Vec perp = dir.Crossed(gp_Vec(0, 0, 1)).Normalized();
    gp_Vec offset = perp.Multiplied(width);
    gp_Vec vert_offset = gp_Vec(0.0, 0.0, height);
    gp_Pnt first_corner = GetVertex(gp_first, offset);
    gp_Pnt second_corner = GetVertex(gp_second, -offset + vert_offset);

    BRepPrimAPI_MakeBox prismBuilder(first_corner, second_corner);
    TopoDS_Shape prism = prismBuilder.Shape();

    BRepMesh_IncrementalMesh triangulation(prism, 0);

    outPositions.clear();
    outIndices.clear();
    TopExp_Explorer aExpFace;
    uint64_t curIndex = 0;
    for (aExpFace.Init(prism, TopAbs_FACE); aExpFace.More(); aExpFace.Next())
    {
        TopoDS_Face aFace = TopoDS::Face(aExpFace.Current());
        TopAbs_Orientation faceOrientation = aFace.Orientation();
        TopLoc_Location aLocation;

        Handle(Poly_Triangulation) aTr = BRep_Tool::Triangulation(aFace, aLocation);

        if (!aTr.IsNull())
        {
            const TColgp_Array1OfPnt &aNodes = aTr->Nodes();
            const Poly_Array1OfTriangle &triangles = aTr->Triangles();
            for (size_t i = triangles.Lower(); i <= triangles.Upper(); i++)
            {
                auto tri = triangles.Value(i);
                Standard_Integer first;
                Standard_Integer second;
                Standard_Integer third;
                tri.Get(first, second, third);
                gp_Pnt aPnt1 = aNodes(first);
                gp_Pnt aPnt2 = aNodes(second);
                gp_Pnt aPnt3 = aNodes(third);
                if (faceOrientation == TopAbs_Orientation::TopAbs_FORWARD)
                {
                    pushPt(outPositions, aPnt1);
                    outIndices.push_back(curIndex++);
                    pushPt(outPositions, aPnt2);
                    outIndices.push_back(curIndex++);
                    pushPt(outPositions, aPnt3);
                    outIndices.push_back(curIndex++);
                }
                else
                {
                    pushPt(outPositions, aPnt3);
                    outIndices.push_back(curIndex++);
                    pushPt(outPositions, aPnt2);
                    outIndices.push_back(curIndex++);
                    pushPt(outPositions, aPnt1);
                    outIndices.push_back(curIndex++);
                }
            }
        }
    }
    std::cout << "Built prism successfully" << std::endl;
}