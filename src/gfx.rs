use anyhow::{Result, Context};
use ash::vk;
use raw_window_handle::HasDisplayHandle;

use std::ffi::CStr;


pub struct Core {
	vk_entry: ash::Entry,
	vk_instance: ash::Instance,
	vk_device: ash::Device,

	vk_queue: vk::Queue,
	vk_cmd_pool: vk::CommandPool,

	vk_debug_messenger: vk::DebugUtilsMessengerEXT,

	debug_util_fns: ash::ext::debug_utils::Instance,
	surface_fns: ash::khr::surface::Instance,

	swapchain_fns: ash::khr::swapchain::Device,
}

impl Core {
	pub fn new(display_handle: impl HasDisplayHandle) -> Result<Core> {
		let vk_entry = unsafe { ash::Entry::load()? };

		let vk_app_info = vk::ApplicationInfo::default()
			.application_name(c"Vk Fuck")
			.application_version(vk::make_api_version(0, 1, 0, 0))
			.engine_name(c"Vk Fuck")
			.engine_version(vk::make_api_version(0, 1, 0, 0))
			.api_version(vk::API_VERSION_1_3);

		let display_handle = display_handle.display_handle()?;

		let mut required_extensions = ash_window::enumerate_required_extensions(display_handle.as_raw())?.to_owned();
		required_extensions.push(c"VK_EXT_debug_utils".as_ptr());

		let validation_layer_name = [c"VK_LAYER_KHRONOS_validation".as_ptr()];

		log::info!("Required extensions: {required_extensions:?}");

		let message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
			| vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
			| vk::DebugUtilsMessageSeverityFlagsEXT::INFO
			| vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;

		let message_type = vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
			| vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
			| vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION;

		let mut messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT::default()
			.pfn_user_callback(Some(vulkan_debug_utils_callback))
			.message_severity(message_severity)
			.message_type(message_type);

		let vk_instance = unsafe {
			let vk_instance_info = vk::InstanceCreateInfo::default()
				.application_info(&vk_app_info)
				.enabled_extension_names(&required_extensions)
				.enabled_layer_names(&validation_layer_name)
				.push_next(&mut messenger_create_info); // Allow messages from create_instance to be caught

			vk_entry.create_instance(&vk_instance_info, None)?
		};

		let debug_util_fns = ash::ext::debug_utils::Instance::new(&vk_entry, &vk_instance);
		let vk_debug_messenger = unsafe {
			debug_util_fns.create_debug_utils_messenger(&messenger_create_info, None)?
		};

		let vk_physical_device = select_physical_device(&vk_instance)?;
		let queue_family_idx = select_graphics_queue_family(&vk_instance, vk_physical_device)?;

		let vk_device = unsafe {
			let ext_names = [c"VK_KHR_swapchain".as_ptr()];

			let queue_create_infos = [
				vk::DeviceQueueCreateInfo::default()
					.queue_family_index(queue_family_idx)
					.queue_priorities(&[1.0])
			];

			let mut features_13 = vk::PhysicalDeviceVulkan13Features::default()
				.dynamic_rendering(true)
				.synchronization2(true);

			let device_create_info = vk::DeviceCreateInfo::default()
				.queue_create_infos(&queue_create_infos)
				.enabled_extension_names(&ext_names)
				.push_next(&mut features_13);

			vk_instance.create_device(vk_physical_device, &device_create_info, None)?
		};

		let vk_queue = unsafe { vk_device.get_device_queue(queue_family_idx, 0) };
		let vk_cmd_pool = unsafe {
			let create_info = vk::CommandPoolCreateInfo::default()
				.queue_family_index(queue_family_idx)
				.flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

			vk_device.create_command_pool(&create_info, None)?
		};

		let surface_fns = ash::khr::surface::Instance::new(&vk_entry, &vk_instance);
		let swapchain_fns = ash::khr::swapchain::Device::new(&vk_instance, &vk_device);

		log::info!("gfx core init");

		Ok(Core {
			vk_entry,
			vk_instance,
			vk_device,

			vk_queue,
			vk_cmd_pool,

			vk_debug_messenger,

			debug_util_fns,
			surface_fns,
			swapchain_fns,
		})
	}
}

impl Drop for Core {
	fn drop(&mut self) {
		unsafe {
			self.vk_device.device_wait_idle().unwrap();

			// TODO(pat.m): destroy resources
			// TODO(pat.m): destroy swapchains
			// TODO(pat.m): destroy surfaces

			self.vk_device.destroy_command_pool(self.vk_cmd_pool, None);
			self.vk_device.destroy_device(None);

			self.debug_util_fns.destroy_debug_utils_messenger(self.vk_debug_messenger, None);
			self.vk_instance.destroy_instance(None);
		}
	}
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
		Ok(message) => log::log!(log_level, "[vk {types}] {message}\n"),
		Err(_) => log::log!(log_level, "[vk {types}] {c_message:?}\n"),
	}

	vk::FALSE
}



fn select_physical_device(vk_instance: &ash::Instance) -> anyhow::Result<vk::PhysicalDevice> {
	unsafe {
		let physical_devices = vk_instance.enumerate_physical_devices()?;

		physical_devices.into_iter()
			.max_by_key(|physical_device| {
				let device_props = vk_instance.get_physical_device_properties(*physical_device);
				match device_props.device_type {
					vk::PhysicalDeviceType::DISCRETE_GPU => 10,
					vk::PhysicalDeviceType::INTEGRATED_GPU => 5,
					_ => 0,
				}
			})
			.context("No physical devices available")
	}
}

fn select_graphics_queue_family(vk_instance: &ash::Instance, physical_device: vk::PhysicalDevice) -> anyhow::Result<u32> {
	// To my knowledge most physical devices only have one graphics capable queue family anyway,
	// so just pick the first one we find.
	unsafe {
		vk_instance.get_physical_device_queue_family_properties(physical_device)
			.into_iter()
			.enumerate()
			.find(|(_, family_properties)| family_properties.queue_flags.contains(vk::QueueFlags::GRAPHICS))
			.map(|(idx, _)| idx as u32)
			.context("Selected physical device has no graphics queue family")
	}
}
