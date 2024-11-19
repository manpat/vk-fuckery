use crate::gfx;
use ash::vk;

#[derive(Debug)]
pub enum DeletableResource {
	DeviceMemory(vk::DeviceMemory),

	Swapchain(vk::SwapchainKHR),
	Surface(vk::SurfaceKHR),

	Semaphore(vk::Semaphore),

	ImageView(vk::ImageView),
	Image(vk::Image),
	Buffer(vk::Buffer),
	Pipeline(vk::Pipeline),
}


impl From<vk::DeviceMemory> for DeletableResource {
	fn from(resource: vk::DeviceMemory) -> Self {
		Self::DeviceMemory(resource)
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

	pub fn queue_deletion(&mut self, resource: impl Into<DeletableResource>, core: &gfx::Core) {
		self.queue_deletion_after(resource, core.timeline_value.get());
	}

	pub fn destroy_ready(&mut self, core: &gfx::Core) {
		let current_timeline_value = unsafe {
			core.vk_device.get_semaphore_counter_value(core.vk_timeline_semaphore).unwrap()
		};

		self.pending_deletions.sort_by_key(|d| d.timeline_value);
		let partition_point = self.pending_deletions.partition_point(|d| d.timeline_value <= current_timeline_value);

		for PendingDeletion{resource, ..} in self.pending_deletions.drain(..partition_point) {
			unsafe {
				destroy_resource_immediate(core, resource);
			}
		}
	}

	pub unsafe fn destroy_all_immediate(&mut self, core: &gfx::Core) {
		// Deletions should be submitted in order to avoid resources being destroyed after resources derived from them.
		self.pending_deletions.sort_by_key(|d| d.timeline_value);

		for PendingDeletion{resource, ..} in self.pending_deletions.drain(..) {
			destroy_resource_immediate(core, resource);
		}
	}
}


unsafe fn destroy_resource_immediate(core: &gfx::Core, resource: impl Into<DeletableResource>) {
	use DeletableResource::*;

	let resource = resource.into();
	log::debug!("Destroying resource {resource:?}");

	unsafe {
		match resource {
			DeviceMemory(vk_resource) => core.vk_device.free_memory(vk_resource, None),

			Swapchain(vk_resource) => core.swapchain_fns.destroy_swapchain(vk_resource, None),
			Surface(vk_resource) => core.surface_fns.destroy_surface(vk_resource, None),

			Semaphore(vk_resource) => core.vk_device.destroy_semaphore(vk_resource, None),

			ImageView(vk_resource) => core.vk_device.destroy_image_view(vk_resource, None),
			Image(vk_resource) => core.vk_device.destroy_image(vk_resource, None),
			Buffer(vk_resource) => core.vk_device.destroy_buffer(vk_resource, None),

			Pipeline(vk_resource) => core.vk_device.destroy_pipeline(vk_resource, None),
		}
	}
}