//! The COM side of the WPD backend (Windows only).

use std::ffi::c_void;

use windows::Win32::Devices::PortableDevices::{
    IPortableDevice, IPortableDeviceContent, IPortableDeviceDataStream,
    IPortableDeviceKeyCollection, IPortableDeviceManager, IPortableDevicePropVariantCollection,
    IPortableDeviceProperties, IPortableDeviceResources, IPortableDeviceValues,
    PORTABLE_DEVICE_DELETE_NO_RECURSION, PortableDeviceFTM, PortableDeviceKeyCollection,
    PortableDeviceManager, PortableDevicePropVariantCollection, PortableDeviceValues,
    WPD_CLIENT_MAJOR_VERSION, WPD_CLIENT_MINOR_VERSION, WPD_CLIENT_NAME, WPD_CLIENT_REVISION,
    WPD_CLIENT_SECURITY_QUALITY_OF_SERVICE, WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER,
    WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ, WPD_COMMAND_MTP_EXT_READ_DATA,
    WPD_CONTENT_TYPE_FOLDER, WPD_CONTENT_TYPE_GENERIC_FILE,
    WPD_OBJECT_CONTAINER_FUNCTIONAL_OBJECT_ID, WPD_OBJECT_CONTENT_TYPE, WPD_OBJECT_FORMAT,
    WPD_OBJECT_NAME, WPD_OBJECT_ORIGINAL_FILE_NAME, WPD_OBJECT_PARENT_ID, WPD_OBJECT_SIZE,
    WPD_PROPERTY_COMMON_COMMAND_CATEGORY, WPD_PROPERTY_COMMON_COMMAND_ID,
    WPD_PROPERTY_COMMON_HRESULT, WPD_PROPERTY_MTP_EXT_OPERATION_CODE,
    WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS, WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE,
    WPD_PROPERTY_MTP_EXT_RESPONSE_CODE, WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT,
    WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ,
    WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ, WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE,
    WPD_RESOURCE_DEFAULT,
};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::StructuredStorage::{PROPVARIANT, PropVariantClear};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
    CoUninitialize, IStream, STGC_DEFAULT, STGM_READ,
};
use windows::Win32::System::Variant::{VT_ERROR, VT_LPWSTR, VT_UI4};
use windows::core::{GUID, HSTRING, Interface, PWSTR, w};

use super::{
    format_code, format_guid, is_garmin_device_id, mtp_error, object_id, open_error, parent_handle,
    parse_object_id, parse_storage_id, read_only_from_object_info, storage_object_id,
};
use crate::error::{Error, ErrorCode};
use crate::mtp::{CreateError, Mtp, MtpError, NewObject};
use crate::ptp::{self, ALL_OBJECTS, DeviceInfo, ObjectInfo, ROOT, format, op, response};
use crate::trace;

/// Read size when WPD suggests none.
const DEFAULT_CHUNK: u32 = 64 * 1024;
/// `SECURITY_IMPERSONATION`: the quality of service WPD's samples request.
const SECURITY_IMPERSONATION: u32 = 0x0002_0000;

/// This thread's COM initialization, undone on drop if this call did it.
struct Com(bool);

impl Com {
    fn init() -> Com {
        // S_OK and S_FALSE count and are balanced in `drop`. RPC_E_CHANGED_MODE means the thread
        // is already in a single-threaded apartment (a UI thread), where WPD works too.
        Com(unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok())
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

fn hresult(e: &windows::core::Error) -> u32 {
    e.code().0 as u32
}

fn message(e: &windows::core::Error) -> String {
    e.message().trim().to_owned()
}

/// Maps a failed WPD call standing in for MTP `operation`.
fn failed(operation: u16, action: &'static str) -> impl FnOnce(windows::core::Error) -> MtpError {
    move |e| mtp_error(operation, action, hresult(&e), message(&e))
}

fn opening(context: &'static str) -> impl FnOnce(windows::core::Error) -> Error {
    move |e| open_error(context, hresult(&e), &message(&e))
}

fn create<T: Interface>(clsid: &GUID) -> windows::core::Result<T> {
    unsafe { CoCreateInstance(clsid, None, CLSCTX_INPROC_SERVER) }
}

/// Takes ownership of a string WPD allocated with `CoTaskMemAlloc`.
///
/// # Safety
/// `p` is null or a NUL-terminated string from `CoTaskMemAlloc` that nothing else frees.
unsafe fn take_string(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    let s = String::from_utf16_lossy(unsafe { p.as_wide() });
    unsafe { CoTaskMemFree(Some(p.0 as *const c_void)) };
    s
}

fn string_value(values: &IPortableDeviceValues, key: &PROPERTYKEY) -> Option<String> {
    let p = unsafe { values.GetStringValue(key) }.ok()?;
    Some(unsafe { take_string(p) })
}

fn guid_value(values: &IPortableDeviceValues, key: &PROPERTYKEY) -> Option<GUID> {
    unsafe { values.GetGuidValue(key) }.ok()
}

fn uint_variant(value: u32) -> PROPVARIANT {
    let mut v = PROPVARIANT::default();
    unsafe {
        let inner = &mut *v.Anonymous.Anonymous;
        inner.vt = VT_UI4;
        inner.Anonymous.ulVal = value;
    }
    v
}

/// A `VT_LPWSTR` borrowing `wide`; collections copy it, so it is never cleared.
fn string_variant(wide: &mut [u16]) -> PROPVARIANT {
    let mut v = PROPVARIANT::default();
    unsafe {
        let inner = &mut *v.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = PWSTR(wide.as_mut_ptr());
    }
    v
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// An open WPD session with the attached Garmin watch.
pub struct WpdMtp {
    device: IPortableDevice,
    content: IPortableDeviceContent,
    properties: IPortableDeviceProperties,
    resources: IPortableDeviceResources,
    /// The properties [`Mtp::object_info`] reads.
    keys: IPortableDeviceKeyCollection,
    info: Option<DeviceInfo>,
    /// Last, so COM is torn down after every interface above is released.
    _com: Com,
}

fn device_ids(manager: &IPortableDeviceManager) -> Result<Vec<String>, Error> {
    let context = "cannot list portable devices";
    let mut count = 0u32;
    unsafe {
        manager.RefreshDeviceList().map_err(opening(context))?;
        manager
            .GetDevices(std::ptr::null_mut(), &mut count)
            .map_err(opening(context))?;
    }
    let mut ids = vec![PWSTR::null(); count as usize];
    if count > 0 {
        unsafe { manager.GetDevices(ids.as_mut_ptr(), &mut count) }.map_err(opening(context))?;
    }
    ids.truncate(count as usize);
    Ok(ids.into_iter().map(|p| unsafe { take_string(p) }).collect())
}

fn client_info() -> windows::core::Result<IPortableDeviceValues> {
    let values: IPortableDeviceValues = create(&PortableDeviceValues)?;
    let version = |v: &str| v.parse::<u32>().unwrap_or(0);
    unsafe {
        values.SetStringValue(&WPD_CLIENT_NAME, w!("Georgian for Garmin"))?;
        values.SetUnsignedIntegerValue(
            &WPD_CLIENT_MAJOR_VERSION,
            version(env!("CARGO_PKG_VERSION_MAJOR")),
        )?;
        values.SetUnsignedIntegerValue(
            &WPD_CLIENT_MINOR_VERSION,
            version(env!("CARGO_PKG_VERSION_MINOR")),
        )?;
        values.SetUnsignedIntegerValue(
            &WPD_CLIENT_REVISION,
            version(env!("CARGO_PKG_VERSION_PATCH")),
        )?;
        values.SetUnsignedIntegerValue(
            &WPD_CLIENT_SECURITY_QUALITY_OF_SERVICE,
            SECURITY_IMPERSONATION,
        )?;
    }
    Ok(values)
}

fn object_keys() -> windows::core::Result<IPortableDeviceKeyCollection> {
    let keys: IPortableDeviceKeyCollection = create(&PortableDeviceKeyCollection)?;
    for key in [
        WPD_OBJECT_PARENT_ID,
        WPD_OBJECT_NAME,
        WPD_OBJECT_ORIGINAL_FILE_NAME,
        WPD_OBJECT_FORMAT,
        WPD_OBJECT_CONTENT_TYPE,
        WPD_OBJECT_SIZE,
        WPD_OBJECT_CONTAINER_FUNCTIONAL_OBJECT_ID,
    ] {
        unsafe { keys.Add(&key)? };
    }
    Ok(keys)
}

/// Finds the single Garmin device WPD lists and opens it through Windows' MTP driver.
pub fn connect() -> Result<WpdMtp, Error> {
    let com = Com::init();
    let context = "cannot start Windows Portable Devices";
    let manager: IPortableDeviceManager =
        create(&PortableDeviceManager).map_err(opening(context))?;
    let mut garmin: Vec<String> = device_ids(&manager)?
        .into_iter()
        .filter(|id| is_garmin_device_id(id))
        .collect();
    let id = match garmin.len() {
        0 => {
            return Err(Error::new(
                ErrorCode::NoDevice,
                "No Garmin watch in USB file-transfer (MTP) mode was found. Connect it with a data cable.",
            ));
        }
        1 => garmin.pop().unwrap(),
        n => {
            return Err(Error::new(
                ErrorCode::MultipleDevices,
                format!("{n} Garmin devices are connected; connect only the watch to update."),
            ));
        }
    };
    trace!("WPD device {id}");
    let context = "cannot open the watch";
    let device: IPortableDevice = create(&PortableDeviceFTM).map_err(opening(context))?;
    let client = client_info().map_err(opening(context))?;
    unsafe { device.Open(&HSTRING::from(id.as_str()), &client) }.map_err(opening(context))?;
    let setup = || -> windows::core::Result<_> {
        let content = unsafe { device.Content() }?;
        let properties = unsafe { content.Properties() }?;
        let resources = unsafe { content.Transfer() }?;
        Ok((content, properties, resources, object_keys()?))
    };
    let (content, properties, resources, keys) = match setup() {
        Ok(parts) => parts,
        Err(e) => {
            let _ = unsafe { device.Close() };
            return Err(opening(context)(e));
        }
    };
    Ok(WpdMtp {
        device,
        content,
        properties,
        resources,
        keys,
        info: None,
        _com: com,
    })
}

impl WpdMtp {
    /// Child object IDs of WPD object `parent`.
    fn children(&self, parent: &HSTRING) -> Result<Vec<String>, MtpError> {
        let fail = || failed(op::GET_OBJECT_HANDLES, "WPD EnumObjects");
        let objects = unsafe {
            self.content
                .EnumObjects(0, parent, None::<&IPortableDeviceValues>)
        }
        .map_err(fail())?;
        let mut ids = Vec::new();
        let mut batch = [PWSTR::null(); 32];
        loop {
            let mut fetched = 0u32;
            let hr = unsafe { objects.Next(&mut batch, &mut fetched) };
            for p in &batch[..(fetched as usize).min(batch.len())] {
                ids.push(unsafe { take_string(*p) });
            }
            hr.ok().map_err(fail())?;
            // S_FALSE: fewer than requested, the end of the list.
            if hr != windows::core::HRESULT(0) || fetched == 0 {
                trace!("EnumObjects {parent}: {} objects", ids.len());
                return Ok(ids);
            }
        }
    }

    /// A WPD command's parameters, with its category and ID set.
    fn command(&self, command: &PROPERTYKEY) -> windows::core::Result<IPortableDeviceValues> {
        let values: IPortableDeviceValues = create(&PortableDeviceValues)?;
        unsafe {
            values.SetGuidValue(&WPD_PROPERTY_COMMON_COMMAND_CATEGORY, &command.fmtid)?;
            values.SetUnsignedIntegerValue(&WPD_PROPERTY_COMMON_COMMAND_ID, command.pid)?;
        }
        Ok(values)
    }

    /// Sends a command; a command that ran but failed reports its HRESULT in the results.
    fn send(
        &self,
        operation: u16,
        params: &IPortableDeviceValues,
    ) -> Result<IPortableDeviceValues, MtpError> {
        let action = "WPD MTP extension command";
        let results =
            unsafe { self.device.SendCommand(0, params) }.map_err(failed(operation, action))?;
        if let Ok(hr) = unsafe { results.GetErrorValue(&WPD_PROPERTY_COMMON_HRESULT) }
            && hr.is_err()
        {
            return Err(mtp_error(
                operation,
                action,
                hr.0 as u32,
                hr.message().trim().to_owned(),
            ));
        }
        Ok(results)
    }

    /// Runs MTP `operation` with a data-in phase through WPD's MTP extension commands. Only
    /// read-only operations go this way, so the driver's view of the device stays current.
    fn mtp_read(&self, operation: u16, params: &[u32]) -> Result<Vec<u8>, MtpError> {
        let fail = || failed(operation, "WPD MTP extension command");
        let start = self
            .command(&WPD_COMMAND_MTP_EXT_EXECUTE_COMMAND_WITH_DATA_TO_READ)
            .map_err(fail())?;
        let collection: IPortableDevicePropVariantCollection =
            create(&PortableDevicePropVariantCollection).map_err(fail())?;
        unsafe {
            start
                .SetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_OPERATION_CODE, u32::from(operation))
                .map_err(fail())?;
            for &p in params {
                collection.Add(&uint_variant(p)).map_err(fail())?;
            }
            start
                .SetIPortableDevicePropVariantCollectionValue(
                    &WPD_PROPERTY_MTP_EXT_OPERATION_PARAMS,
                    &collection,
                )
                .map_err(fail())?;
        }
        let started = self.send(operation, &start)?;
        let context = string_value(&started, &WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT)
            .ok_or_else(|| MtpError::Protocol("WPD gave no MTP transfer context".into()))?;
        let context = HSTRING::from(context.as_str());
        let total = unsafe {
            started.GetUnsignedLargeIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_TOTAL_DATA_SIZE)
        }
        .unwrap_or(0);
        let optimal = unsafe {
            started.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_OPTIMAL_TRANSFER_BUFFER_SIZE)
        }
        .ok()
        .filter(|&n| n > 0)
        .unwrap_or(DEFAULT_CHUNK);

        let read = self.read_data(operation, &context, total, optimal);
        // The transfer is ended even after a failed read, so the driver takes new commands.
        let end = self.end_transfer(operation, &context);
        let data = read?;
        let code = end?;
        trace!(
            "MTP extension {} {params:08x?}: {} bytes, response {code:#06x}",
            ptp::operation_name(operation),
            data.len()
        );
        if code != response::OK {
            return Err(MtpError::Response { operation, code });
        }
        Ok(data)
    }

    fn read_data(
        &self,
        operation: u16,
        context: &HSTRING,
        total: u64,
        optimal: u32,
    ) -> Result<Vec<u8>, MtpError> {
        let fail = || failed(operation, "WPD MTP extension command");
        let total = usize::try_from(total)
            .map_err(|_| MtpError::Protocol(format!("{total}-byte MTP data phase")))?;
        let mut data = Vec::with_capacity(total);
        let request = vec![0u8; total.min(optimal as usize)];
        while data.len() < total {
            let want = (total - data.len()).min(request.len());
            let params = self
                .command(&WPD_COMMAND_MTP_EXT_READ_DATA)
                .map_err(fail())?;
            unsafe {
                params
                    .SetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, context)
                    .map_err(fail())?;
                params
                    .SetUnsignedIntegerValue(
                        &WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_TO_READ,
                        want as u32,
                    )
                    .map_err(fail())?;
                params
                    .SetBufferValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, &request[..want])
                    .map_err(fail())?;
            }
            let results = self.send(operation, &params)?;
            let got = unsafe {
                results.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_NUM_BYTES_READ)
            }
            .map_err(fail())? as usize;
            let mut buffer: *mut u8 = std::ptr::null_mut();
            let mut len = 0u32;
            unsafe {
                results
                    .GetBufferValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_DATA, &mut buffer, &mut len)
                    .map_err(fail())?;
                if !buffer.is_null() {
                    let n = got.min(len as usize).min(want);
                    data.extend_from_slice(std::slice::from_raw_parts(buffer, n));
                    CoTaskMemFree(Some(buffer as *const c_void));
                }
            }
            if got == 0 {
                break;
            }
        }
        Ok(data)
    }

    fn end_transfer(&self, operation: u16, context: &HSTRING) -> Result<u16, MtpError> {
        let fail = || failed(operation, "WPD MTP extension command");
        let params = self
            .command(&WPD_COMMAND_MTP_EXT_END_DATA_TRANSFER)
            .map_err(fail())?;
        unsafe { params.SetStringValue(&WPD_PROPERTY_MTP_EXT_TRANSFER_CONTEXT, context) }
            .map_err(fail())?;
        let results = self.send(operation, &params)?;
        let code = unsafe { results.GetUnsignedIntegerValue(&WPD_PROPERTY_MTP_EXT_RESPONSE_CODE) }
            .map_err(fail())?;
        Ok(code as u16)
    }

    /// Writes `data` through a WPD object-creation stream and commits it; returns the new
    /// object's ID.
    fn write_new(&self, info: &ObjectInfo, parent: u32, data: &[u8]) -> Result<String, MtpError> {
        let fail = || {
            failed(
                op::SEND_OBJECT_INFO,
                "WPD CreateObjectWithPropertiesAndData",
            )
        };
        let values: IPortableDeviceValues = create(&PortableDeviceValues).map_err(fail())?;
        let name = HSTRING::from(info.filename.as_str());
        unsafe {
            values
                .SetStringValue(&WPD_OBJECT_PARENT_ID, &HSTRING::from(object_id(parent)))
                .map_err(fail())?;
            values
                .SetStringValue(&WPD_OBJECT_NAME, &name)
                .map_err(fail())?;
            values
                .SetStringValue(&WPD_OBJECT_ORIGINAL_FILE_NAME, &name)
                .map_err(fail())?;
            values
                .SetUnsignedLargeIntegerValue(&WPD_OBJECT_SIZE, data.len() as u64)
                .map_err(fail())?;
            values
                .SetGuidValue(
                    &WPD_OBJECT_FORMAT,
                    &GUID::from_u128(format_guid(info.format)),
                )
                .map_err(fail())?;
            values
                .SetGuidValue(&WPD_OBJECT_CONTENT_TYPE, &WPD_CONTENT_TYPE_GENERIC_FILE)
                .map_err(fail())?;
        }
        let mut stream: Option<IStream> = None;
        let mut optimal = 0u32;
        unsafe {
            self.content.CreateObjectWithPropertiesAndData(
                &values,
                &mut stream,
                &mut optimal,
                std::ptr::null_mut(),
            )
        }
        .map_err(fail())?;
        let stream =
            stream.ok_or_else(|| MtpError::Protocol("WPD returned no object stream".into()))?;
        let chunk = if optimal == 0 { DEFAULT_CHUNK } else { optimal } as usize;
        let send = || failed(op::SEND_OBJECT, "WPD object stream write");
        let written = (|| {
            let mut offset = 0;
            while offset < data.len() {
                let n = (data.len() - offset).min(chunk);
                let mut wrote = 0u32;
                unsafe {
                    stream
                        .Write(data[offset..].as_ptr().cast(), n as u32, Some(&mut wrote))
                        .ok()
                        .map_err(send())?;
                }
                if wrote == 0 {
                    return Err(MtpError::Protocol(format!(
                        "WPD accepted no data at byte {offset} of {}",
                        data.len()
                    )));
                }
                offset += wrote as usize;
            }
            unsafe { stream.Commit(STGC_DEFAULT) }.map_err(send())
        })();
        if let Err(e) = written {
            let _ = unsafe { stream.Revert() };
            return Err(e);
        }
        let stream: IPortableDeviceDataStream = stream.cast().map_err(send())?;
        let id = unsafe { stream.GetObjectID() }.map_err(send())?;
        let id = unsafe { take_string(id) };
        trace!(
            "created {id} in {} ({} bytes)",
            object_id(parent),
            data.len()
        );
        Ok(id)
    }
}

impl Drop for WpdMtp {
    fn drop(&mut self) {
        let _ = unsafe { self.device.Close() };
    }
}

impl Mtp for WpdMtp {
    fn device_info(&mut self) -> Result<DeviceInfo, MtpError> {
        if let Some(info) = &self.info {
            return Ok(info.clone());
        }
        let info = DeviceInfo::decode(&self.mtp_read(op::GET_DEVICE_INFO, &[])?)?;
        trace!(
            "device: {} {} {}, vendor extension {:?}, operations {:04x?}",
            info.manufacturer,
            info.model,
            info.device_version,
            info.vendor_extension_desc,
            info.operations_supported
        );
        self.info = Some(info.clone());
        Ok(info)
    }

    fn storage_ids(&mut self) -> Result<Vec<u32>, MtpError> {
        // WPD_DEVICE_OBJECT_ID: the device object, whose children are its storages.
        let device = HSTRING::from("DEVICE");
        let ids = self.children(&device)?;
        Ok(ids.iter().filter_map(|id| parse_storage_id(id)).collect())
    }

    fn object_handles(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let parent_id = match parent {
            // No WPD call lists a whole storage; MTP's GetObjectHandles does, read-only.
            ALL_OBJECTS => {
                let data = self.mtp_read(op::GET_OBJECT_HANDLES, &[storage, 0, ALL_OBJECTS])?;
                return Ok(ptp::Reader::new(&data).u32_array()?);
            }
            ROOT => storage_object_id(storage),
            handle => object_id(handle),
        };
        let ids = self.children(&HSTRING::from(parent_id))?;
        Ok(ids
            .iter()
            .filter_map(|id| {
                let handle = parse_object_id(id);
                if handle.is_none() {
                    trace!("skipping WPD child {id:?}: not an MTP object");
                }
                handle
            })
            .collect())
    }

    fn object_info(&mut self, handle: u32) -> Result<ObjectInfo, MtpError> {
        let id = object_id(handle);
        let values = unsafe {
            self.properties
                .GetValues(&HSTRING::from(id.as_str()), &self.keys)
        }
        .map_err(failed(op::GET_OBJECT_INFO, "WPD GetValues"))?;
        // For an ID with no object behind it, WPD answers without these properties.
        let missing = || MtpError::Response {
            operation: op::GET_OBJECT_INFO,
            code: response::INVALID_OBJECT_HANDLE,
        };
        let parent = string_value(&values, &WPD_OBJECT_PARENT_ID)
            .and_then(|p| parent_handle(&p))
            .ok_or_else(missing)?;
        let filename = string_value(&values, &WPD_OBJECT_ORIGINAL_FILE_NAME)
            .or_else(|| string_value(&values, &WPD_OBJECT_NAME))
            .ok_or_else(missing)?;
        let storage_id = string_value(&values, &WPD_OBJECT_CONTAINER_FUNCTIONAL_OBJECT_ID)
            .and_then(|s| parse_storage_id(&s))
            .ok_or_else(|| MtpError::Protocol(format!("WPD reports no storage for {id}")))?;
        let format =
            match guid_value(&values, &WPD_OBJECT_FORMAT).and_then(|g| format_code(g.to_u128())) {
                Some(code) => code,
                None if guid_value(&values, &WPD_OBJECT_CONTENT_TYPE)
                    == Some(WPD_CONTENT_TYPE_FOLDER) =>
                {
                    format::ASSOCIATION
                }
                None => format::UNDEFINED,
            };
        let size = unsafe { values.GetUnsignedLargeIntegerValue(&WPD_OBJECT_SIZE) }.unwrap_or(0);
        // Only the fields inspect and stage use; ProtectionStatus has no WPD property, see
        // `read_only`, so it stays at the default here.
        Ok(ObjectInfo {
            storage_id,
            format,
            // MTP's ObjectCompressedSize reads 0xFFFFFFFF for objects of 4 GiB or more.
            compressed_size: u32::try_from(size).unwrap_or(u32::MAX),
            parent,
            filename,
            ..ObjectInfo::default()
        })
    }

    fn get_object(&mut self, handle: u32) -> Result<Vec<u8>, MtpError> {
        let fail = || failed(op::GET_OBJECT, "WPD resource read");
        let mut stream: Option<IStream> = None;
        let mut optimal = 0u32;
        unsafe {
            self.resources.GetStream(
                &HSTRING::from(object_id(handle)),
                &WPD_RESOURCE_DEFAULT,
                STGM_READ.0,
                &mut optimal,
                &mut stream,
            )
        }
        .map_err(fail())?;
        let stream =
            stream.ok_or_else(|| MtpError::Protocol("WPD returned no resource stream".into()))?;
        let chunk = if optimal == 0 { DEFAULT_CHUNK } else { optimal } as usize;
        let mut data = Vec::new();
        loop {
            data.reserve(chunk);
            let mut read = 0u32;
            unsafe {
                stream
                    .Read(
                        data.spare_capacity_mut().as_mut_ptr().cast(),
                        chunk as u32,
                        Some(&mut read),
                    )
                    .ok()
                    .map_err(fail())?;
            }
            if read == 0 {
                trace!("read {} ({} bytes)", object_id(handle), data.len());
                return Ok(data);
            }
            // SAFETY: the stream wrote `read` (at most `chunk`) bytes into the reserved space.
            unsafe { data.set_len(data.len() + (read as usize).min(chunk)) };
        }
    }

    fn create_object(
        &mut self,
        storage: u32,
        parent: u32,
        info: &ObjectInfo,
        data: &[u8],
    ) -> Result<NewObject, CreateError> {
        // Until WPD names the committed object, any failure leaves its existence unknown.
        let id = self
            .write_new(info, parent, data)
            .map_err(CreateError::Unconfirmed)?;
        let handle = parse_object_id(&id).ok_or_else(|| {
            CreateError::Unconfirmed(MtpError::Protocol(format!(
                "WPD named the new object {id:?}, not an MTP object ID"
            )))
        })?;
        let placed = self
            .object_info(handle)
            .map_err(|error| CreateError::Data { handle, error })?;
        let new = NewObject {
            storage: placed.storage_id,
            parent: placed.parent,
            handle,
        };
        if new.parent != parent || new.storage != storage {
            return Err(CreateError::Misplaced(new));
        }
        Ok(new)
    }

    fn delete_object(&mut self, handle: u32) -> Result<(), MtpError> {
        let fail = || failed(op::DELETE_OBJECT, "WPD Delete");
        let ids: IPortableDevicePropVariantCollection =
            create(&PortableDevicePropVariantCollection).map_err(fail())?;
        let mut id = wide(&object_id(handle));
        let mut results: Option<IPortableDevicePropVariantCollection> = None;
        unsafe {
            ids.Add(&string_variant(&mut id)).map_err(fail())?;
            self.content
                .Delete(
                    PORTABLE_DEVICE_DELETE_NO_RECURSION.0 as u32,
                    &ids,
                    &mut results,
                )
                .map_err(fail())?;
        }
        // Delete answers S_FALSE when an object was not deleted; the result says why.
        if let Some(results) = results {
            let mut result = PROPVARIANT::default();
            unsafe {
                // GetAt writes the value, though its binding declares the pointer const.
                if results.GetAt(0, (&raw mut result).cast_const()).is_ok() {
                    let inner = &*result.Anonymous.Anonymous;
                    let code = (inner.vt == VT_ERROR).then_some(inner.Anonymous.scode);
                    let _ = PropVariantClear(&mut result);
                    if let Some(code) = code.filter(|&c| c < 0) {
                        let hr = windows::core::HRESULT(code);
                        return Err(mtp_error(
                            op::DELETE_OBJECT,
                            "WPD Delete",
                            code as u32,
                            hr.message().trim().to_owned(),
                        ));
                    }
                }
            }
        }
        trace!("deleted {}", object_id(handle));
        Ok(())
    }

    fn garmin_object_ids(&mut self, storage: u32, parent: u32) -> Result<Vec<u32>, MtpError> {
        let data = self.mtp_read(op::GARMIN_GET_OBJECT_IDS, &[storage, parent])?;
        Ok(ptp::Reader::new(&data).u32_array()?)
    }

    /// ProtectionStatus from the watch's own ObjectInfo, through WPD's MTP extension command
    /// (`read_only_from_object_info` says why); unknown if that fails.
    fn read_only(&mut self, handle: u32, _info: &ObjectInfo) -> Option<bool> {
        match self.mtp_read(op::GET_OBJECT_INFO, &[handle]) {
            Ok(data) => read_only_from_object_info(&data),
            Err(e) => {
                trace!("ProtectionStatus of {}: {e}", object_id(handle));
                None
            }
        }
    }
}
