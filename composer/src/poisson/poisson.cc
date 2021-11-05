#include <cstddef>

#include "Src_CC_wrap/PoissonReconLib.h"

extern "C" {
typedef void *cloud32;

size_t poisson_cloud32_size(const cloud32 *cloud);
bool poisson_cloud32_has_normals(const cloud32 *cloud);
bool poisson_cloud32_has_colors(const cloud32 *cloud);
void poisson_cloud32_get_point(const cloud32 *cloud, size_t index,
                               float *coords);
void poisson_cloud32_get_normal(const cloud32 *cloud, size_t index,
                                float *coords);
void poisson_cloud32_get_color(const cloud32 *cloud, size_t index, float *rgb);

typedef void *cloud64;

size_t poisson_cloud64_size(const cloud64 *cloud);
bool poisson_cloud64_has_normals(const cloud64 *cloud);
bool poisson_cloud64_has_colors(const cloud64 *cloud);
void poisson_cloud64_get_point(const cloud64 *cloud, size_t index,
                               double *coords);
void poisson_cloud64_get_normal(const cloud64 *cloud, size_t index,
                                double *coords);
void poisson_cloud64_get_color(const cloud64 *cloud, size_t index, double *rgb);

typedef void *mesh32;

void poisson_mesh32_add_vertex(mesh32 *mesh32, const float *coords);
void poisson_mesh32_add_normal(mesh32 *mesh32, const float *coords);
void poisson_mesh32_add_color(mesh32 *mesh32, const float *rgb);
void poisson_mesh32_add_density(mesh32 *mesh32, double d);
void poisson_mesh32_add_triangle(mesh32 *mesh32, size_t i1, size_t i2,
                                 size_t i3);

typedef void *mesh64;

void poisson_mesh64_add_vertex(mesh64 *mesh64, const double *coords);
void poisson_mesh64_add_normal(mesh64 *mesh64, const double *coords);
void poisson_mesh64_add_color(mesh64 *mesh64, const double *rgb);
void poisson_mesh64_add_density(mesh64 *mesh64, double d);
void poisson_mesh64_add_triangle(mesh64 *mesh64, size_t i1, size_t i2,
                                 size_t i3);
}

namespace {

struct Cloud32 : public PoissonReconLib::ICloud<float> {
  const cloud32 *cloud;

  size_t size() const override { return poisson_cloud32_size(cloud); }
  bool hasNormals() const override {
    return poisson_cloud32_has_normals(cloud);
  }
  bool hasColors() const { return poisson_cloud32_has_colors(cloud); }
  void getPoint(size_t index, float *coords) const override {
    poisson_cloud32_get_point(cloud, index, coords);
  }
  void getNormal(size_t index, float *coords) const override {
    poisson_cloud32_get_normal(cloud, index, coords);
  }
  void getColor(size_t index, float *rgb) const override {
    poisson_cloud32_get_color(cloud, index, rgb);
  }
};

struct Cloud64 : public PoissonReconLib::ICloud<double> {
  const cloud64 *cloud;

  size_t size() const override { return poisson_cloud64_size(cloud); }
  bool hasNormals() const override {
    return poisson_cloud64_has_normals(cloud);
  }
  bool hasColors() const { return poisson_cloud64_has_colors(cloud); }
  void getPoint(size_t index, double *coords) const override {
    poisson_cloud64_get_point(cloud, index, coords);
  }
  void getNormal(size_t index, double *coords) const override {
    poisson_cloud64_get_normal(cloud, index, coords);
  }
  void getColor(size_t index, double *rgb) const override {
    poisson_cloud64_get_color(cloud, index, rgb);
  }
};

struct Mesh32 : public PoissonReconLib::IMesh<float> {
  mesh32 *mesh;

  void addVertex(const float *coords) override {
    poisson_mesh32_add_vertex(mesh, coords);
  }
  void addNormal(const float *coords) override {
    poisson_mesh32_add_normal(mesh, coords);
  }
  void addColor(const float *rgb) override {
    poisson_mesh32_add_color(mesh, rgb);
  }
  void addDensity(double d) override { poisson_mesh32_add_density(mesh, d); }
  void addTriangle(size_t i1, size_t i2, size_t i3) override {
    poisson_mesh32_add_triangle(mesh, i1, i2, i3);
  }
};

struct Mesh64 : public PoissonReconLib::IMesh<double> {
  mesh64 *mesh;

  void addVertex(const double *coords) override {
    poisson_mesh64_add_vertex(mesh, coords);
  }
  void addNormal(const double *coords) override {
    poisson_mesh64_add_normal(mesh, coords);
  }
  void addColor(const double *rgb) override {
    poisson_mesh64_add_color(mesh, rgb);
  }
  void addDensity(double d) override { poisson_mesh64_add_density(mesh, d); }
  void addTriangle(size_t i1, size_t i2, size_t i3) override {
    poisson_mesh64_add_triangle(mesh, i1, i2, i3);
  }
};

} // namespace

extern "C" {
typedef PoissonReconLib::Parameters params;

bool poisson_reconstruct32(const params *params, const cloud32 *cloud,
                           mesh32 *mesh) {
  Mesh32 mesh_wrapper;
  mesh_wrapper.mesh = mesh;
  Cloud32 cloud_wrapper;
  cloud_wrapper.cloud = cloud;
  return PoissonReconLib::Reconstruct(*params, cloud_wrapper, mesh_wrapper);
}

bool poisson_reconstruct64(const params *params, const cloud32 *cloud,
                           mesh32 *mesh) {
  Mesh64 mesh_wrapper;
  mesh_wrapper.mesh = mesh;
  Cloud64 cloud_wrapper;
  cloud_wrapper.cloud = cloud;
  return PoissonReconLib::Reconstruct(*params, cloud_wrapper, mesh_wrapper);
}
}
