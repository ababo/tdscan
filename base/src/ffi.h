#ifndef FITSME_BASE_FFI_H_
#define FITSME_BASE_FFI_H_

#include <stddef.h>
#include <stdint.h>

enum FmError {
  kOk = 0,
  kIoError = 3,
  kMalformedData = 6,
  kUnsupportedFeature = 7,
};

typedef void *FmWriter;

typedef FmError (*FmWriteCallback)(const char *data, size_t size);

FmError fm_create_writer(FmWriteCallback callback, FmWriter *writer);

FmError fm_close_writer(FmWriter writer);

struct FmPoint3 {
  float x;
  float y;
  float z;
};

struct FmScan {
  const char *name;
  float angular_velocity;
  FmPoint3 eye_position;
  float view_elevation;
};

FmError fm_write_scan(FmWriter writer, const struct FmScan *scan);

struct FmScanFrame {
  int64_t time;
  const char *png;
  size_t png_size;
  const float *depths;
  size_t depths_size;
};

FmError fm_write_scan_frame(FmWriter writer, const struct FmScanFrame *frame);

#endif  // FITSME_BASE_FFI_H_
