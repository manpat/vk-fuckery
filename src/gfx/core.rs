use anyhow::{Result, Context};
use ash::vk;

use crate::gfx;

use winit::event_loop::OwnedDisplayHandle;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use std::cell::Cell;
use std::mem::ManuallyDrop;


pub struct Core {
	display_handle: OwnedDisplayHandle,

	vk_entry: ash::Entry,
	pub vk_instance: ash::Instance,
	pub vk_device: ash::Device,
	pub vk_physical_device: vk::PhysicalDevice,

	pub vk_queue: vk::Queue,
	pub vk_cmd_pool: vk::CommandPool,

	pub vk_timeline_semaphore: vk::Semaphore,
	pub timeline_value: Cell<u64>,

	// Must be dropped before instance.
	// TODO(pat.m): make ManuallyDrop not required
	pub debug: ManuallyDrop<gfx::Debug>,

	pub surface_fns: ash::khr::surface::Instance,
	pub swapchain_fns: ash::khr::swapchain::Device,
}

impl Core {
	pub fn new(display_handle: OwnedDisplayHandle) -> Result<Core> {
		let vk_entry = unsafe { ash::Entry::load()? };

		let vk_app_info = vk::ApplicationInfo::default()
			.application_name(c"Vk Fuck")
			.application_version(vk::make_api_version(0, 1, 0, 0))
			.engine_name(c"Vk Fuck")
			.engine_version(vk::make_api_version(0, 1, 0, 0))
			.api_version(vk::API_VERSION_1_3);

		let raw_display_handle = display_handle.display_handle()?.as_raw();

		let mut required_extensions = ash_window::enumerate_required_extensions(raw_display_handle)?.to_owned();
		required_extensions.push(c"VK_EXT_debug_utils".as_ptr());

		let validation_layer_name = [c"VK_LAYER_KHRONOS_validation".as_ptr()];

		let vk_instance = unsafe {
			let mut debug_create_info = gfx::new_debug_create_info();
			let vk_instance_info = vk::InstanceCreateInfo::default()
				.application_info(&vk_app_info)
				.enabled_extension_names(&required_extensions)
				.enabled_layer_names(&validation_layer_name)
				.push_next(&mut debug_create_info); // Allow messages from create_instance to be caught

			vk_entry.create_instance(&vk_instance_info, None)?
		};

		let debug = gfx::Debug::install(&vk_entry, &vk_instance)?;

		let vk_physical_device = select_physical_device(&vk_instance)?;
		let queue_family_idx = select_graphics_queue_family(&vk_instance, vk_physical_device)?;

		let device_properties = unsafe { vk_instance.get_physical_device_properties(vk_physical_device) };
		let extensions = unsafe { vk_instance.enumerate_device_extension_properties(vk_physical_device)? };

		let extensions = extensions.into_iter()
			.filter_map(|props| {
				props.extension_name_as_c_str().ok()
					.map(|s| s.to_string_lossy().into_owned())
			})
			.collect::<Vec<_>>();

		log::info!("Physical device properties: {device_properties:#?}");
		log::info!("Supported device extensions: {extensions:?}");
		// TODO(pat.m): check for vk::KHR_SWAPCHAIN_MUTABLE_FORMAT_NAME

		let vk_device = unsafe {
			let ext_names = [
				vk::KHR_SWAPCHAIN_NAME.as_ptr(),

				// TODO(pat.m): can we be sure this is available.
				vk::KHR_SWAPCHAIN_MUTABLE_FORMAT_NAME.as_ptr(),
			];

			let queue_create_infos = [
				vk::DeviceQueueCreateInfo::default()
					.queue_family_index(queue_family_idx)
					.queue_priorities(&[1.0])
			];

			let mut features_12 = vk::PhysicalDeviceVulkan12Features::default()
				.timeline_semaphore(true)
				.buffer_device_address(true)
				.scalar_block_layout(true);

			let mut features_13 = vk::PhysicalDeviceVulkan13Features::default()
				.dynamic_rendering(true)
				.synchronization2(true);

			let device_create_info = vk::DeviceCreateInfo::default()
				.queue_create_infos(&queue_create_infos)
				.enabled_extension_names(&ext_names)
				.push_next(&mut features_12)
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

		let vk_timeline_semaphore = unsafe {
			let mut timeline_create_info = vk::SemaphoreTypeCreateInfo::default()
				.semaphore_type(vk::SemaphoreType::TIMELINE)
				.initial_value(0);

			vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default().push_next(&mut timeline_create_info), None)?
		};

		let surface_fns = ash::khr::surface::Instance::new(&vk_entry, &vk_instance);
		let swapchain_fns = ash::khr::swapchain::Device::new(&vk_instance, &vk_device);

		log::info!("gfx core init");

		Ok(Core {
			display_handle,

			vk_entry,
			vk_instance,
			vk_device,
			vk_physical_device,

			vk_queue,
			vk_cmd_pool,

			vk_timeline_semaphore,
			timeline_value: Cell::new(0),

			debug: ManuallyDrop::new(debug),

			surface_fns,
			swapchain_fns,
		})
	}

	pub fn create_surface(&self, window_handle: impl HasWindowHandle) -> Result<vk::SurfaceKHR> {
		let display_handle = self.display_handle.display_handle()?.as_raw();
		let window_handle = window_handle.window_handle()?.as_raw();
		unsafe {
			ash_window::create_surface(&self.vk_entry, &self.vk_instance, display_handle, window_handle, None)
				.map_err(Into::into)
		}
	}

	pub fn get_surface_capabilities(&self, surface: vk::SurfaceKHR) -> anyhow::Result<vk::SurfaceCapabilitiesKHR> {
		unsafe {
			self.surface_fns.get_physical_device_surface_capabilities(self.vk_physical_device, surface)
				.map_err(Into::into)
		}
	}

	pub fn next_timeline_value(&self) -> u64 {
		let next_value = self.timeline_value.get() + 1;
		self.timeline_value.set(next_value);
		next_value
	}

	pub fn wait_idle(&self) {
		unsafe {
			self.vk_device.device_wait_idle().unwrap();
		}
	}
}

impl Drop for Core {
	fn drop(&mut self) {
		unsafe {
			self.vk_device.device_wait_idle().unwrap();

			self.vk_device.destroy_semaphore(self.vk_timeline_semaphore, None);

			self.vk_device.destroy_command_pool(self.vk_cmd_pool, None);
			self.vk_device.destroy_device(None);

			ManuallyDrop::take(&mut self.debug).destroy();
			self.vk_instance.destroy_instance(None);
		}
	}
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




#[derive(Debug)]
pub struct StagingBuffer {
	vk_memory: vk::DeviceMemory,
	vk_buffer: vk::Buffer,

	mapped_ptr: *mut u8,
	allocation_size: usize,
	write_cursor: usize,

	pub device_address: vk::DeviceAddress,

	last_upload_timeline_value: u64,
}

impl StagingBuffer {
	pub fn new(core: &gfx::Core, allocator: &gfx::DeviceAllocator) -> anyhow::Result<StagingBuffer> {
		let allocation_size = 100 << 20;
		let vk_memory = allocator.allocate_staging_memory(core, allocation_size)?;

		let buffer_usage = vk::BufferUsageFlags::TRANSFER_SRC
			| vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;

		let buffer_info = vk::BufferCreateInfo::default()
			.size(allocation_size)
			.usage(buffer_usage);

		let vk_buffer = unsafe { core.vk_device.create_buffer(&buffer_info, None)? };
		let buffer_requirements = unsafe { core.vk_device.get_buffer_memory_requirements(vk_buffer) };

		log::info!("Staging buffer memory requirements: size {}MiB - align {}", buffer_requirements.size >> 20, buffer_requirements.alignment);

		// vulkan guarantees that vk_memory will be adequately aligned for anything we want to put in it.
		// its only at non-zero offsets that we need to care about alignment.
		unsafe {
			core.vk_device.bind_buffer_memory(vk_buffer, vk_memory, 0)?;
		}

		let mapped_ptr = unsafe {
			let offset = 0;
			let memory_map_flags = vk::MemoryMapFlags::empty();
			core.vk_device.map_memory(vk_memory, offset, vk::WHOLE_SIZE, memory_map_flags)?.cast()
		};

		let device_address = unsafe {
			core.vk_device.get_buffer_device_address(&vk::BufferDeviceAddressInfo::default().buffer(vk_buffer))
		};

		// TODO(pat.m): must align mapped_ptr range to VkPhysicalDeviceLimits::nonCoherentAtomSize if memory isn't HOST_COHERENT.

		Ok(StagingBuffer {
			vk_memory,
			vk_buffer,

			mapped_ptr,
			allocation_size: allocation_size as usize,
			write_cursor: 0,

			device_address,

			last_upload_timeline_value: 0,
		})
	}

	pub fn queue_deletion(&self, deletion_queue: &mut gfx::DeletionQueue) {
		// No unmap required! vkFreeMemory will implicitly unmap memory.

		deletion_queue.queue_deletion_after(self.vk_buffer, self.last_upload_timeline_value);
		deletion_queue.queue_deletion_after(self.vk_memory, self.last_upload_timeline_value + 1);
	}

	pub fn allocate_write_space(&mut self, size: usize, alignment: usize) -> usize {
		let align_offset = unsafe {
			self.mapped_ptr.add(self.write_cursor).align_offset(alignment)
		};

		let offset = self.write_cursor + align_offset;
		self.write_cursor = offset + size;

		assert!(self.write_cursor <= self.allocation_size, "Staging buffer overflow");

		offset
	}

	pub fn write<T>(&mut self, data: &T) -> vk::DeviceAddress
		where T: bytemuck::NoUninit + Copy
	{
		let alignment = std::mem::align_of::<T>().max(8);
		let offset = self.allocate_write_space(std::mem::size_of::<T>(), alignment);

		unsafe {
			self.mapped_ptr
				.add(offset)
				.cast::<T>()
				.write(*data);
		}

		self.device_address + offset as u64
	}
}
