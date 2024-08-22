use stablegui_common::ElId;
use stablegui_transfer;
use std::sync::Mutex;

mod worker_instance;

static WORKER_INSTANCE: Mutex<Option<worker_instance::WorkerInstance>> = Mutex::new(None);

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

#[repr(C)]
pub struct Attribute {
    pub key: *const char,
    pub value: *const char,
}

#[repr(C)]
pub struct Element {
    pub value: ElId,
    pub parent: ElId,
    pub attribute_count: i32,
    pub attributes: *const Attribute,
    pub string_value: *const char,
}

#[no_mangle]
pub extern "C" fn stablegui_init() {
    WORKER_INSTANCE
        .lock()
        .unwrap()
        .replace(worker_instance::WorkerInstance::new().unwrap());
}

#[no_mangle]
pub extern "C" fn stablegui_add_element(el: *const Element) {
    match WORKER_INSTANCE.lock().unwrap().as_mut() {
        Some(worker_instance) => unsafe {
            let transfer = stablegui_transfer::Element {
                value: (*el).value,
                parent: (*el).parent,
                attributes: if (*el).attribute_count == 0 {
                    vec![]
                } else {
                    std::slice::from_raw_parts((*el).attributes, (*el).attribute_count as usize)
                        .iter()
                        .map(|attr| (strize(&attr.key), strize(&attr.value)))
                        .collect()
                },
                string_value: strize(&(*el).string_value),
            };

            worker_instance.add_element(&transfer)
        },
        None => eprintln!("Worker instance not initialized"),
    }
}

unsafe fn strize(c_str: &*const char) -> &str {
    std::ffi::CStr::from_ptr(*c_str as *const i8)
        .to_str()
        .unwrap_or_else(|e| {
            eprintln!("{}", e);
            ""
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
