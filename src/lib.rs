use ftd2xx_sys::*;

use std::{error, ffi, fmt, io, os, ptr};

#[derive(Debug)]
pub struct FTError {
    status: FT_STATUS,
}

impl FTError {
    fn from_raw(status: FT_STATUS) -> Option<FTError> {
        if status == FT_OK as FT_STATUS {
            None
        } else {
            Some(FTError { status })
        }
    }

    fn raw(&self) -> u32 {
        self.status as u32
    }
}

impl error::Error for FTError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(self)
    }
}

impl fmt::Display for FTError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FT_STATUS: {}", self.raw())
    }
}

// TODO: There's definitely a more elegant solution for this
fn status_to_result(status: FT_STATUS) -> Result<()> {
    match FTError::from_raw(status) {
        None => Ok(()),
        Some(err) => Err(err),
    }
}

type Result<T> = std::result::Result<T, FTError>;

/// Scans for any connected FTD2XX devices
pub fn scan_devices() -> Result<Vec<Device>> {
    let mut devices = Vec::new();
    let mut num_devices = 0;
    unsafe {
        status_to_result(FT_CreateDeviceInfoList(&mut num_devices))?;
    }

    if num_devices > 0 {
        let mut info_nodes = vec![FT_DEVICE_LIST_INFO_NODE::default(); num_devices as usize];
        unsafe {
            status_to_result(FT_GetDeviceInfoList(
                info_nodes.as_mut_ptr(),
                &mut num_devices,
            ))?;
        }

        for index in 0..num_devices {
            devices.push(Device {
                index: index as usize,
                info: info_nodes[index as usize],
            });
        }
    }

    Ok(devices)
}

pub struct FTProgramData {
    manufacturer: [char; 32],
    manufacturer_id: [char; 16],
    description: [char; 64],
    serial_number: [char; 16],
    inner: FT_PROGRAM_DATA,
}

impl FTProgramData {
    pub fn get_manufacturer(&self) -> &str {
        unsafe {
            ffi::CStr::from_ptr(self.manufacturer.as_ptr() as *const os::raw::c_char)
                .to_str()
                .unwrap()
        }
    }
    pub fn get_manufacturer_id(&self) -> &str {
        unsafe {
            ffi::CStr::from_ptr(self.manufacturer_id.as_ptr() as *const os::raw::c_char)
                .to_str()
                .unwrap()
        }
    }
    pub fn get_description(&self) -> &str {
        unsafe {
            ffi::CStr::from_ptr(self.description.as_ptr() as *const os::raw::c_char)
                .to_str()
                .unwrap()
        }
    }
    pub fn get_serial_number(&self) -> &str {
        unsafe {
            ffi::CStr::from_ptr(self.serial_number.as_ptr() as *const os::raw::c_char)
                .to_str()
                .unwrap()
        }
    }
}

impl fmt::Display for FTProgramData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#?}", self.inner)
    }
}

pub struct Device {
    index: usize,
    info: _ft_device_list_info_node,
}

impl Device {
    pub fn is_open(&self) -> bool {
        self.info.Flags & 0x1 != 0
    }
    pub fn is_high_speed(&self) -> bool {
        self.info.Flags & 0x2 != 0
    }
    pub fn get_flags(&self) -> u32 {
        self.info.Flags
    }
    pub fn get_type(&self) -> u32 {
        self.info.Type
    }
    pub fn get_id(&self) -> u32 {
        self.info.ID
    }
    pub fn get_local_id(&self) -> u32 {
        self.info.LocId
    }
    pub fn get_serial_number(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr(self.info.SerialNumber.as_ptr())
                .to_str()
                .unwrap()
        }
    }
    pub fn get_description(&self) -> &str {
        unsafe {
            std::ffi::CStr::from_ptr(self.info.Description.as_ptr())
                .to_str()
                .unwrap()
        }
    }
    pub fn get_handle(&self) -> usize {
        self.info.ftHandle as usize
    }
    pub fn get_bitmode(&self) -> Result<u8> {
        let mut bitmode = 0;
        unsafe {
            status_to_result(FT_GetBitMode(self.info.ftHandle, &mut bitmode))?;
        }
        Ok(bitmode)
    }
    pub fn open(&mut self) -> Result<()> {
        unsafe { status_to_result(FT_Open(self.index as i32, &mut self.info.ftHandle)) }
    }
    pub fn close(&mut self) -> Result<()> {
        unsafe {
            status_to_result(FT_Close(self.info.ftHandle))?;
        }

        self.info.ftHandle = ptr::null_mut();

        Ok(())
    }
    pub fn set_baud_rate(&mut self, rate: u32) -> Result<()> {
        unsafe { status_to_result(FT_SetBaudRate(self.info.ftHandle, rate)) }
    }
    pub fn query_program_data(&self) -> Result<FTProgramData> {
        let mut data = FTProgramData {
            // TODO: There's got to be a better way to initialize these...
            manufacturer: ['\0'; 32],
            manufacturer_id: ['\0'; 16],
            description: ['\0'; 64],
            serial_number: ['\0'; 16],
            inner: FT_PROGRAM_DATA::default(),
        };
        data.inner.Signature1 = 0x00000000;
        data.inner.Signature2 = 0xffffffff;
        data.inner.Version = 0x00000005;
        data.inner.Manufacturer = data.manufacturer.as_mut_ptr() as *mut os::raw::c_char;
        data.inner.ManufacturerId = data.manufacturer_id.as_mut_ptr() as *mut os::raw::c_char;
        data.inner.Description = data.description.as_mut_ptr() as *mut os::raw::c_char;
        data.inner.SerialNumber = data.serial_number.as_mut_ptr() as *mut os::raw::c_char;
        unsafe {
            status_to_result(FT_EE_Read(self.info.ftHandle, &mut data.inner))?;
        }
        Ok(data)
    }
}

impl io::Read for Device {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        unsafe {
            let mut bytes_read = 0;
            if status_to_result(FT_Read(
                self.info.ftHandle,
                buf.as_mut_ptr() as *mut ffi::c_void,
                buf.len() as u32,
                &mut bytes_read,
            ))
            .is_ok()
            {
                Ok(bytes_read as usize)
            } else {
                Err(io::Error::from(io::ErrorKind::Other))
            }
        }
    }
}

impl io::Write for Device {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            let mut bytes_written = 0;
            if status_to_result(FT_Write(
                self.info.ftHandle,
                buf.as_ptr() as *mut ffi::c_void,
                buf.len() as u32,
                &mut bytes_written,
            ))
            .is_ok()
            {
                Ok(bytes_written as usize)
            } else {
                Err(io::Error::from(io::ErrorKind::Other))
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
