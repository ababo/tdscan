#include "../base/src/ffi.h"

#include <stdlib.h>
#include <string.h>

struct WriterImpl {
  FmWriteCallback callback;
  void *cb_data;
};

static void call_callback(FmWriter writer, const char *data) {
  struct WriterImpl *impl = (struct WriterImpl *)writer;
  impl->callback((uint8_t *)data, strlen(data), impl->cb_data);
}

enum FmError fm_create_writer(FmWriteCallback callback, void *cb_data,
                              FmWriter *writer) {
  struct WriterImpl *impl = malloc(sizeof(struct WriterImpl));
  impl->callback = callback;
  impl->cb_data = cb_data;
  *writer = impl;
  call_callback(impl, "fm_create_writer ");
  return kFmOk;
}

enum FmError fm_close_writer(FmWriter writer) {
  call_callback(writer, "fm_close_writer ");
  free(writer);
  return kFmOk;
}

enum FmError fm_write_scan(FmWriter writer, const struct FmScan *scan) {
  call_callback(writer, "fm_write_scan ");
  return kFmOk;
}

enum FmError fm_write_scan_frame(FmWriter writer,
                                 const struct FmScanFrame *frame) {
  call_callback(writer, "fm_write_scan_frame ");
  return kFmOk;
}
