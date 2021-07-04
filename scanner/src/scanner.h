#ifndef FITSME_SCANNER_H_
#define FITSME_SCANNER_H_

#include <stddef.h>
#include <stdint.h>

enum FmError {
  kOk = 0,
  kIoError = 3,
  kMalformedData = 6,
  kUnsupportedFeature = 7,
};

typedef void *FmWriter;

FmError fm_create_file_writer(const char *filename, FmWriter *writer);

FmError fm_close_file_writer(FmWriter *writer);

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

FmError fm_write_scan(FmWriter *writer, const FmScan *scan);

struct FmScanFrame {
  int64_t time;
  const char *png;
  size_t png_size;
  const float *depths;
  size_t depths_size;
};

FmError fm_write_scan_frame(FmWriter *writer, const FmScanFrame *frame);

#endif  // FITSME_SCANNER_H_
