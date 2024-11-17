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

		log::info!("Required extensions: {required_extensions:?}");


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

		unsafe {
			let extensions = vk_instance.enumerate_device_extension_properties(vk_physical_device)?;
			log::info!("Supported device extensions: {extensions:#?}");

			// TODO(pat.m): check for vk::KHR_SWAPCHAIN_MUTABLE_FORMAT_NAME
		}

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
				.timeline_semaphore(true);

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
pub enum DeletableResource {
	Swapchain(vk::SwapchainKHR),
	Surface(vk::SurfaceKHR),

	Semaphore(vk::Semaphore),

	ImageView(vk::ImageView),
	Image(vk::Image),

	Buffer(vk::Buffer),

	Pipeline(vk::Pipeline),
}

impl Core {
	pub unsafe fn destroy_resource_immediate(&self, resource: impl Into<DeletableResource>) {
		use DeletableResource::*;

		let resource = resource.into();
		log::info!("Destroying resource {resource:?}");

		unsafe {
			match resource {
				Swapchain(vk_resource) => self.swapchain_fns.destroy_swapchain(vk_resource, None),
				Surface(vk_resource) => self.surface_fns.destroy_surface(vk_resource, None),

				Semaphore(vk_resource) => self.vk_device.destroy_semaphore(vk_resource, None),

				ImageView(vk_resource) => self.vk_device.destroy_image_view(vk_resource, None),
				Image(vk_resource) => self.vk_device.destroy_image(vk_resource, None),
				Buffer(vk_resource) => self.vk_device.destroy_buffer(vk_resource, None),

				Pipeline(vk_resource) => self.vk_device.destroy_pipeline(vk_resource, None),
			}
		}
	} 
}

impl From<vk::SwapchainKHR> for DeletableResource {
	fn from(resource: vk::SwapchainKHR) -> Self {
		Self::Swapchain(resource)
	}
}

impl From<vk::SurfaceKHR> for DeletableResource {
	fn from(resource: vk::SurfaceKHR) -> Self {
		Self::Surface(resource)
	}
}

impl From<vk::Semaphore> for DeletableResource {
	fn from(resource: vk::Semaphore) -> Self {
		Self::Semaphore(resource)
	}
}

impl From<vk::Image> for DeletableResource {
	fn from(resource: vk::Image) -> Self {
		Self::Image(resource)
	}
}

impl From<vk::ImageView> for DeletableResource {
	fn from(resource: vk::ImageView) -> Self {
		Self::ImageView(resource)
	}
}

impl From<vk::Buffer> for DeletableResource {
	fn from(resource: vk::Buffer) -> Self {
		Self::Buffer(resource)
	}
}

impl From<vk::Pipeline> for DeletableResource {
	fn from(resource: vk::Pipeline) -> Self {
		Self::Pipeline(resource)
	}
}


pub struct PendingDeletion {
	timeline_value: u64,
	resource: DeletableResource,
}

#[derive(Default)]
pub struct DeletionQueue {
	pending_deletions: Vec<PendingDeletion>,
}

impl DeletionQueue {
	pub fn queue_deletion_after(&mut self, resource: impl Into<DeletableResource>, timeline_value: u64) {
		self.pending_deletions.push(PendingDeletion {
			resource: resource.into(),
			timeline_value,
		});
	}

	pub fn queue_deletion(&mut self, resource: impl Into<DeletableResource>, core: &Core) {
		self.queue_deletion_after(resource, core.timeline_value.get());
	}

	pub fn destroy_ready(&mut self, core: &Core) {
		let current_timeline_value = unsafe {
			core.vk_device.get_semaphore_counter_value(core.vk_timeline_semaphore).unwrap()
		};

		self.pending_deletions.sort_by_key(|d| d.timeline_value);
		let partition_point = self.pending_deletions.partition_point(|d| d.timeline_value <= current_timeline_value);

		for PendingDeletion{resource, ..} in self.pending_deletions.drain(..partition_point) {
			unsafe {
				core.destroy_resource_immediate(resource);
			}
		}
	}

	pub unsafe fn destroy_all_immediate(&mut self, core: &Core) {
		// Deletions should be submitted in order to avoid resources being destroyed after resources derived from them.
		self.pending_deletions.sort_by_key(|d| d.timeline_value);

		for PendingDeletion{resource, ..} in self.pending_deletions.drain(..) {
			core.destroy_resource_immediate(resource);
		}
	}
}