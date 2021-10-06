use std::ffi::CStr;
use std::io::{
    Error as IoError, ErrorKind as IoErrorKind, Result as IoResult, Write,
};
use std::os::raw::{c_char, c_float, c_int, c_void};
use std::slice::from_raw_parts;

use crate::defs::IntoResult;
use crate::fm;
use crate::fm::record::Type::*;
use crate::fm::{Write as _, Writer, WriterParams};

type FmWriteCallback = extern "C" fn(
    fm_data: *const u8,
    fm_size: usize,
    cb_data: *mut c_void,
) -> c_int;

struct FmWriterData {
    pub callback: FmWriteCallback,
    pub cb_data: *mut c_void,
}

impl Write for FmWriterData {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let ret = (self.callback)(buf.as_ptr(), buf.len(), self.cb_data);
        match ret.into_result(|| format!("FFI error")) {
            Ok(()) => Ok(buf.len()),
            Err(err) => Err(IoError::new(IoErrorKind::Other, err)),
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

pub type FmWriter = *mut c_void;

#[no_mangle]
pub unsafe extern "C" fn fm_create_writer(
    callback: FmWriteCallback,
    cb_data: *mut c_void,
    writer: &mut FmWriter,
) -> c_int {
    let data = FmWriterData { callback, cb_data };
    let params = WriterParams::default();
    match Writer::new(data, &params) {
        Ok(w) => {
            *writer = Box::into_raw(Box::new(w)) as FmWriter;
            0
        }
        Err(err) => err.kind as c_int,
    }
}

#[no_mangle]
pub unsafe extern "C" fn fm_close_writer(writer: FmWriter) -> c_int {
    let boxed = Box::from_raw(writer as *mut Writer<FmWriterData>);
    match boxed.into_inner() {
        Ok(_) => 0,
        Err((_, err)) => err.kind as c_int,
    }
}

#[repr(C)]
pub struct FmPoint3 {
    x: c_float,
    y: c_float,
    z: c_float,
}

#[repr(C)]
pub struct FmScan {
    name: *const c_char,
    camera_angle_of_view: c_float,
    camera_up_angle: c_float,
    camera_angular_velocity: c_float,
    camera_initial_position: FmPoint3,
    camera_initial_direction: FmPoint3,
    image_width: c_int,
    image_height: c_int,
    depth_width: c_int,
    depth_height: c_int,
    sensor_plane_depth: c_int,
}

#[no_mangle]
pub unsafe extern "C" fn fm_write_scan(
    writer: FmWriter,
    scan: &FmScan,
) -> c_int {
    let rec = fm::Record {
        r#type: Some(Scan(fm::Scan {
            name: CStr::from_ptr(scan.name).to_str().unwrap().to_owned(),
            camera_angle_of_view: scan.camera_angle_of_view,
            camera_up_angle: scan.camera_up_angle,
            camera_angular_velocity: scan.camera_angular_velocity,
            camera_initial_position: Some(fm::Point3 {
                x: scan.camera_initial_position.x,
                y: scan.camera_initial_position.y,
                z: scan.camera_initial_position.z,
            }),
            camera_initial_direction: Some(fm::Point3 {
                x: scan.camera_initial_direction.x,
                y: scan.camera_initial_direction.y,
                z: scan.camera_initial_direction.z,
            }),
            image_width: scan.image_width as u32,
            image_height: scan.image_height as u32,
            depth_width: scan.depth_width as u32,
            depth_height: scan.depth_height as u32,
            sensor_plane_depth: scan.sensor_plane_depth != 0,
        })),
    };

    write_record(writer, &rec)
}

#[repr(C)]
pub struct FmImage {
    r#type: c_int,
    data: *const u8,
    data_size: usize,
}

#[repr(C)]
pub struct FmScanFrame {
    scan: *const c_char,
    time: i64,
    image: FmImage,
    depths: *const c_float,
    depths_size: usize,
    depth_confidences: *const u8,
    depth_confidences_size: usize,
}

#[no_mangle]
pub unsafe extern "C" fn fm_write_scan_frame(
    writer: FmWriter,
    frame: &FmScanFrame,
) -> c_int {
    let rec = fm::Record {
        r#type: Some(ScanFrame(fm::ScanFrame {
            scan: CStr::from_ptr(frame.scan).to_str().unwrap().to_owned(),
            time: frame.time,
            image: Some(fm::Image {
                r#type: frame.image.r#type,
                data: from_raw_parts(frame.image.data, frame.image.data_size)
                    .to_vec(),
            }),
            depths: from_raw_parts(frame.depths, frame.depths_size).to_vec(),
            depth_confidences: from_raw_parts(
                frame.depth_confidences,
                frame.depth_confidences_size,
            )
            .iter()
            .map(|c| *c as i32)
            .collect(),
        })),
    };

    write_record(writer, &rec)
}

unsafe fn write_record(writer: FmWriter, rec: &fm::Record) -> c_int {
    match (*(writer as *mut Writer<FmWriterData>)).write_record(rec) {
        Ok(()) => 0,
        Err(err) => err.kind as c_int,
    }
}
