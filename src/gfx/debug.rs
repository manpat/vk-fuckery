use ash::vk;
use std::ffi::CStr;

pub struct Debug {
	pub debug_util_fns: ash::ext::debug_utils::Instance,
	pub vk_debug_messenger: vk::DebugUtilsMessengerEXT,
}

impl Debug {
	pub fn install(vk_entry: &ash::Entry, vk_instance: &ash::Instance) -> anyhow::Result<Debug> {
		let debug_util_fns = ash::ext::debug_utils::Instance::new(&vk_entry, &vk_instance);
		let vk_debug_messenger = unsafe {
			debug_util_fns.create_debug_utils_messenger(&new_debug_create_info(), None)?
		};

		Ok(Debug {
			debug_util_fns,
			vk_debug_messenger,
		})
	}

	pub fn destroy(self) {
		unsafe {
			self.debug_util_fns.destroy_debug_utils_messenger(self.vk_debug_messenger, None);
		}
	}
}

pub fn new_debug_create_info() -> vk::DebugUtilsMessengerCreateInfoEXT<'static> {
	let message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
		| vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
		| vk::DebugUtilsMessageSeverityFlagsEXT::INFO
		| vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;

	let message_type = vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
		| vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
		| vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION;

	vk::DebugUtilsMessengerCreateInfoEXT::default()
		.pfn_user_callback(Some(vulkan_debug_utils_callback))
		.message_severity(message_severity)
		.message_type(message_type)
}

unsafe extern "system" fn vulkan_debug_utils_callback(
	message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
	message_type: vk::DebugUtilsMessageTypeFlagsEXT,
	p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
	_p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {

	// Don't care about verbose general messages
	if message_type == vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
		&& message_severity < vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
	{
		return vk::FALSE
	}

	let log_level = match message_severity {
		vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::Level::Trace,
		vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::Level::Warn,
		vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::Level::Error,
		vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::Level::Info,
		_ => log::Level::Trace,
	};

	let types = match message_type {
		vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "general",
		vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "performance",
		vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "validation",
		_ => "?",
	};

	let c_message = CStr::from_ptr((*p_callback_data).p_message);
	match c_message.to_str() {
		Ok(message) => log::log!(log_level, "[vk {types}] {message}"),
		Err(_) => log::log!(log_level, "[vk {types}] {c_message:?}"),
	}

	vk::FALSE
}