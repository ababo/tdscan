#ifndef FITSME_BASE_FFI_H_
#define FITSME_BASE_FFI_H_

#include <stddef.h>
#include <stdint.h>

enum FmError {
  kFmOk = 0,
  kFmIoError = 4,
  kFmMalformedData = 7,
  kFmUnsupportedFeature = 8,
};

typedef void *FmWriter;

typedef enum FmError (*FmWriteCallback)(const uint8_t *fm_data, size_t fm_size,
                                        void *cb_data);

enum FmError fm_create_writer(FmWriteCallback callback, void *cb_data,
                              FmWriter *writer);

enum FmError fm_close_writer(FmWriter writer);

struct FmPoint3 {
  float x;
  float y;
  float z;
};

enum FmImageType {
  kFmImageNone = 0,
  kFmImagePng = 1,
  kFmImageJpeg = 2,
};

struct FmImage {
  enum FmImageType type;
  const uint8_t *data;
  size_t data_size;
};

struct FmScan {
  const char *name;
  float camera_angle_of_view;
  float camera_landscape_angle;
  float camera_view_elevation;
  float camera_angular_velocity;
  struct FmPoint3 camera_initial_position;
  int image_width;
  int image_height;
  int depth_width;
  int depth_height;
};

enum FmError fm_write_scan(FmWriter writer, const struct FmScan *scan);

enum FmDepthConfidence {
  kFmDepthConfidenceNone = 0,
  kFmDepthConfidenceLow = 1,
  kFmDepthConfidenceMedium = 2,
  kFmDepthConfidenceHigh = 3,
};

struct FmScanFrame {
  const char *scan;
  int64_t time;
  struct FmImage image;
  const float *depths;
  size_t depths_size;
  const uint8_t *depth_confidences;
  size_t depth_confidences_size;
};

enum FmError fm_write_scan_frame(FmWriter writer,
                                 const struct FmScanFrame *frame);

#endif  // FITSME_BASE_FFI_H_
