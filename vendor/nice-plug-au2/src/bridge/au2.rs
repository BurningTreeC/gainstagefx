use coreaudio_sys::{AudioComponentDescription, AudioComponentPlugInInterface};

unsafe extern "C" {
    fn nice_au2_register_plugin_entry();
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn NiceAu2Factory(
    description: *const AudioComponentDescription,
) -> *mut AudioComponentPlugInInterface {
    unsafe { nice_au2_register_plugin_entry() };

    if !description.is_null() {
        let mut expected = [0; 5];
        super::nice_au2_get_component_description(expected.as_mut_ptr());
        let description = unsafe { &*description };
        if expected[0] != 0
            && (description.componentType != expected[0]
                || description.componentSubType != expected[1]
                || description.componentManufacturer != expected[2])
        {
            return std::ptr::null_mut();
        }
    }

    super::component::Component::allocate()
}
