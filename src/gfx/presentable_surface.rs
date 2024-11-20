use ash::vk;
use anyhow::Context;
use winit::window::Window;
use crate::gfx;



pub struct Frame {
	vk_cmd_buffer: vk::CommandBuffer,
	swapchain_image: SwapchainImage,
	sync_index: usize,

	pub extent: vk::Extent2D,
}

impl Frame {
	pub fn cmd_buffer(&self) -> vk::CommandBuffer {
		self.vk_cmd_buffer
	}

	pub fn swapchain_image_view(&self) -> vk::ImageView {
		self.swapchain_image.vk_image_view
	}
}

struct FrameSync {
	image_available_semaphore: vk::Semaphore,
	raster_finish_semaphore: vk::Semaphore,

	prev_submit_timeline_value: u64,
}


pub struct PresentableSurface {
	vk_surface: vk::SurfaceKHR,

	swapchain: Swapchain,

	frame_syncs: Vec<FrameSync>,
	next_sync_index: usize,

	vk_cmd_buffers: Vec<vk::CommandBuffer>,

	pub swapchain_extent: vk::Extent2D,
	pub swapchain_format: vk::Format,
	swapchain_present_mode: vk::PresentModeKHR,
	num_swapchain_images: u32,
}

impl PresentableSurface {
	pub fn new(core: &gfx::Core, window: &Window) -> anyhow::Result<PresentableSurface> {
		let vk_surface = core.create_surface(&window)?;

		// Swapchain
		let surface_capabilities = core.get_surface_capabilities(vk_surface)?;
		let supported_formats = unsafe{ core.surface_fns.get_physical_device_surface_formats(core.vk_physical_device, vk_surface)? };
		let supported_present_modes = unsafe{ core.surface_fns.get_physical_device_surface_present_modes(core.vk_physical_device, vk_surface)? };

		const NO_CURRENT_EXTENT: vk::Extent2D = vk::Extent2D{ width: u32::MAX, height: u32::MAX };
		let swapchain_extent = match surface_capabilities.current_extent {
			NO_CURRENT_EXTENT => {
				let (width, height) = window.inner_size().into();
				vk::Extent2D{ width, height }
			},

			current => current,
		};

		log::info!("Surface capabilities: {surface_capabilities:#?}");
		log::info!("Supported formats: {supported_formats:#?}");
		log::info!("Supported present modes: {supported_present_modes:?}");

		anyhow::ensure!(
			surface_capabilities.supported_transforms.contains(vk::SurfaceTransformFlagsKHR::IDENTITY),
			"Can't present to surface - identity transform not supported");

		dbg!(&surface_capabilities, &supported_formats, &supported_present_modes);

		let selected_present_mode = supported_present_modes.into_iter()
			.max_by_key(|&mode| match mode {
				vk::PresentModeKHR::FIFO => 1,
				vk::PresentModeKHR::FIFO_RELAXED => 2,
				// vk::PresentModeKHR::MAILBOX => 10,
				_ => 0,
			})
			.context("Selecting supported present mode")?;

		let selected_format = supported_formats.into_iter()
			.filter(|format| format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR)
			.map(|format| format.format)
			.max_by_key(|&format| match format {
				vk::Format::R8G8B8A8_SRGB => 15,
				vk::Format::B8G8R8A8_SRGB => 14,
				vk::Format::A8B8G8R8_SRGB_PACK32 => 13,

				vk::Format::R8G8B8A8_UNORM => 5,
				vk::Format::B8G8R8A8_UNORM => 4,
				vk::Format::A8B8G8R8_UNORM_PACK32 => 3,
				_ => 0,
			})
			.context("Selecting supported swapchain format")?;

		let max_images = match surface_capabilities.max_image_count {
			0 => u32::MAX,
			n => n
		};

		let num_images = (surface_capabilities.min_image_count + 1).min(max_images);

		log::info!("Selected present mode: {selected_present_mode:?}");
		log::info!("Selected swapchain format: {selected_format:?}");

		let swapchain = Swapchain::new(core, vk_surface, selected_format, selected_present_mode, swapchain_extent, num_images, None)?;

		// command buffers
		let vk_cmd_buffers = unsafe {
			let create_info = vk::CommandBufferAllocateInfo::default()
				.command_buffer_count(swapchain.vk_images.len() as u32)
				.command_pool(core.vk_cmd_pool)
				.level(vk::CommandBufferLevel::PRIMARY);

			core.vk_device.allocate_command_buffers(&create_info)?
		};

		let frame_syncs = (0..swapchain.vk_images.len())
			.map(|_| unsafe {
				anyhow::Result::Ok(FrameSync {
					image_available_semaphore: core.vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
					raster_finish_semaphore: core.vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?,
					prev_submit_timeline_value: 0,
				})
			})
			.collect::<anyhow::Result<Vec<_>>>()?;

		Ok(PresentableSurface {
			vk_surface,

			swapchain,

			frame_syncs,
			next_sync_index: 0,

			vk_cmd_buffers,

			swapchain_extent,
			swapchain_format: selected_format,
			swapchain_present_mode: selected_present_mode,
			num_swapchain_images: num_images,
		})
	}

	pub fn queue_deletion(self, deletion_queue: &mut gfx::DeletionQueue) {
		let latest_submit_timeline_value = self.frame_syncs.iter()
			.map(|sync| sync.prev_submit_timeline_value)
			.max()
			.unwrap_or(0);

		for frame_sync in self.frame_syncs {
			deletion_queue.queue_deletion_after(frame_sync.image_available_semaphore, frame_sync.prev_submit_timeline_value);
			deletion_queue.queue_deletion_after(frame_sync.raster_finish_semaphore, frame_sync.prev_submit_timeline_value);
		}

		self.swapchain.queue_deletion(deletion_queue, latest_submit_timeline_value);

		// The surface must be deleted _after_ the swapchain
		deletion_queue.queue_deletion_after(self.vk_surface, latest_submit_timeline_value+1);
	}

	pub fn resize(&mut self, core: &gfx::Core, deletion_queue: &mut gfx::DeletionQueue, new_size: vk::Extent2D) -> anyhow::Result<()> {
		if self.swapchain_extent == new_size {
			return Ok(());
		}

		log::info!("Resize event {new_size:?}");

		let surface_capabilities = core.get_surface_capabilities(self.vk_surface)?;

		if new_size.width < surface_capabilities.min_image_extent.width
			|| new_size.width > surface_capabilities.max_image_extent.width
			|| new_size.width == 0
			|| new_size.height < surface_capabilities.min_image_extent.height
			|| new_size.height > surface_capabilities.max_image_extent.height
			|| new_size.height == 0
		{
			// TODO(pat.m): skip rendering
			self.swapchain_extent = vk::Extent2D{ width: 0, height: 0 };
			return Ok(());
		}

		let new_swapchain = Swapchain::new(core, self.vk_surface, self.swapchain_format, self.swapchain_present_mode, new_size, self.num_swapchain_images, Some(&self.swapchain))?;

		self.swapchain_extent = new_size;
		self.swapchain.queue_deletion(deletion_queue, core.timeline_value.get());

		self.swapchain = new_swapchain;

		Ok(())
	}

	pub fn start_frame(&mut self, core: &gfx::Core) -> anyhow::Result<Frame> {
		if self.swapchain_extent.width == 0 || self.swapchain_extent.height == 0 {
			anyhow::bail!("No swapchain");
		}

		let timeout_ns = 1000*1000*1000;

		let sync_index = self.next_sync_index;
		self.next_sync_index = (self.next_sync_index + 1) % self.frame_syncs.len();

		let frame_sync = &self.frame_syncs[sync_index];
		let vk_cmd_buffer = self.vk_cmd_buffers[sync_index];

		unsafe {
			core.vk_device.wait_semaphores(
				&vk::SemaphoreWaitInfo::default()
					.semaphores(&[core.vk_timeline_semaphore])
					.values(&[frame_sync.prev_submit_timeline_value]),
				timeout_ns
			)?;
		}

		let swapchain_image = self.swapchain.acquire_image(core, frame_sync.image_available_semaphore, timeout_ns)?;

		unsafe {
			core.vk_device.begin_command_buffer(vk_cmd_buffer,
				&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;

			core.vk_device.cmd_pipeline_barrier2(
				vk_cmd_buffer,
				&vk::DependencyInfo::default()
					.image_memory_barriers(&[
						vk::ImageMemoryBarrier2::default()
							.image(swapchain_image.vk_image)
							.old_layout(vk::ImageLayout::UNDEFINED) // Don't care about previous contents
							.new_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)

							.src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT) // Don't stall any pre-rasterisation stages
							.src_access_mask(vk::AccessFlags2::NONE)

							.dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
							.dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_READ | vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
							.subresource_range(
								vk::ImageSubresourceRange::default()
									.aspect_mask(vk::ImageAspectFlags::COLOR)
									.base_mip_level(0)
									.base_array_layer(0)
									.level_count(1)
									.layer_count(1)
							)
					]
				)
			);

		}

		Ok(Frame {
			vk_cmd_buffer,
			swapchain_image,
			sync_index,

			extent: self.swapchain_extent,
		})
	}

	pub fn submit_frame(&mut self, core: &gfx::Core, frame: Frame) -> anyhow::Result<()> {
		let frame_sync = &mut self.frame_syncs[frame.sync_index];
		let swapchain_image = frame.swapchain_image;

		unsafe {
			core.vk_device.cmd_pipeline_barrier2(
				frame.vk_cmd_buffer,
				&vk::DependencyInfo::default()
					.image_memory_barriers(&[
						vk::ImageMemoryBarrier2::default()
							.image(swapchain_image.vk_image)
							.old_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
							.new_layout(vk::ImageLayout::PRESENT_SRC_KHR)

							.src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
							.src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)

							// Don't wait for anything, vkQueuePresentKHR performs visibility operations automatically
							.dst_stage_mask(vk::PipelineStageFlags2::NONE)
							.dst_access_mask(vk::AccessFlags2::NONE)
							.subresource_range(
								vk::ImageSubresourceRange::default()
									.aspect_mask(vk::ImageAspectFlags::COLOR)
									.base_mip_level(0)
									.base_array_layer(0)
									.level_count(1)
									.layer_count(1)
							)
					]
				)
			);

			core.vk_device.end_command_buffer(frame.vk_cmd_buffer)?;

			let timeline_value = core.next_timeline_value();
			frame_sync.prev_submit_timeline_value = timeline_value;

			core.vk_device.queue_submit2(
				core.vk_queue,
				&[
					vk::SubmitInfo2::default()
						.wait_semaphore_infos(&[
							// image available happens-before wait operation, which happens-before any raster output.
							// i.e., don't block anything except raster while sema is unsignalled
							vk::SemaphoreSubmitInfo::default()
								.semaphore(frame_sync.image_available_semaphore)
								.stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
						])
						.command_buffer_infos(&[
							vk::CommandBufferSubmitInfo::default()
								.command_buffer(frame.vk_cmd_buffer)
						])
						.signal_semaphore_infos(&[
							// raster output happens-before 'raster finish sema' signal operation, which happens-before later present.
							vk::SemaphoreSubmitInfo::default()
								.semaphore(frame_sync.raster_finish_semaphore)
								.stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT),

							// timeline semaphore signal op happens-after all commands complete, which happens-before the next frame where images and cmd buffers can be reused.
							vk::SemaphoreSubmitInfo::default()
								.semaphore(core.vk_timeline_semaphore)
								.stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
								.value(timeline_value),
						])
				],
				vk::Fence::null()
			).context("Submitting command buffer")?;
		}

		self.swapchain.submit_image(core, swapchain_image, frame_sync.raster_finish_semaphore)?;

		Ok(())
	}
}




struct Swapchain {
	vk_swapchain: vk::SwapchainKHR,
	vk_images: Vec<vk::Image>,
	vk_image_views: Vec<vk::ImageView>,
}

impl Swapchain {
	fn new(core: &gfx::Core, surface: vk::SurfaceKHR, format: vk::Format, present_mode: vk::PresentModeKHR, extent: vk::Extent2D, num_images: u32, old_swapchain: Option<&Swapchain>) -> anyhow::Result<Swapchain> {
		let format_srgb = match format {
			vk::Format::R8G8B8A8_UNORM => vk::Format::R8G8B8A8_SRGB,
			vk::Format::B8G8R8A8_UNORM => vk::Format::B8G8R8A8_SRGB,
			vk::Format::A8B8G8R8_UNORM_PACK32 => vk::Format::A8B8G8R8_SRGB_PACK32,
			x => x,
		};

		let formats = [format, format_srgb];
		let mut format_list_info = vk::ImageFormatListCreateInfo::default()
			.view_formats(&formats);

		let mut swapchain_info = vk::SwapchainCreateInfoKHR::default()
			.surface(surface)
			.min_image_count(num_images)
			.image_format(format)
			.image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
			.image_extent(extent)
			.image_array_layers(1)
			.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
			.pre_transform(vk::SurfaceTransformFlagsKHR::IDENTITY)
			.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
			.present_mode(present_mode)
			.clipped(true);

		if let Some(old_swapchain) = old_swapchain {
			swapchain_info.old_swapchain = old_swapchain.vk_swapchain;
		}

		if format_srgb != format {
			swapchain_info = swapchain_info
				.flags(vk::SwapchainCreateFlagsKHR::MUTABLE_FORMAT)
				.push_next(&mut format_list_info);
		}

		let vk_swapchain = unsafe { core.swapchain_fns.create_swapchain(&swapchain_info, None).context("Creating swapchain")? };
		let vk_images = unsafe { core.swapchain_fns.get_swapchain_images(vk_swapchain).context("Getting swapchain images")? };

		let vk_image_views: Vec<_> = vk_images.iter()
			.map(|&image| unsafe {
				let create_info = vk::ImageViewCreateInfo::default()
					.image(image)
					.view_type(vk::ImageViewType::TYPE_2D)
					.format(format_srgb)
					.components(
						vk::ComponentMapping {
							r: vk::ComponentSwizzle::R,
							g: vk::ComponentSwizzle::G,
							b: vk::ComponentSwizzle::B,
							a: vk::ComponentSwizzle::A,
						}
					)
					.subresource_range(
						vk::ImageSubresourceRange::default()
							.aspect_mask(vk::ImageAspectFlags::COLOR)
							.base_mip_level(0)
							.base_array_layer(0)
							.level_count(1)
							.layer_count(1)
					);

				core.vk_device.create_image_view(&create_info, None).context("Creating swapchain image views")
			})
			.collect::<Result<_, _>>()?;

		Ok(Swapchain {
			vk_swapchain,
			vk_images,
			vk_image_views,
		})
	}

	fn queue_deletion(&self, deletion_queue: &mut gfx::DeletionQueue, timeline_value: u64) {
		deletion_queue.queue_deletion_after(self.vk_swapchain, timeline_value);

		for image_view in self.vk_image_views.iter() {
			deletion_queue.queue_deletion_after(*image_view, timeline_value);
		}
	}

	fn acquire_image(&self, core: &gfx::Core, image_acquire: vk::Semaphore, timeout_ns: u64) -> anyhow::Result<SwapchainImage> {
		let (image_index, _) = unsafe {
			core.swapchain_fns.acquire_next_image(
				self.vk_swapchain,
				timeout_ns,
				image_acquire,
				vk::Fence::null()
			).context("Acquiring swapchain image")?
		};

		Ok(SwapchainImage {
			vk_image: self.vk_images[image_index as usize],
			vk_image_view: self.vk_image_views[image_index as usize],
			image_index,
		})
	}

	fn submit_image(&self, core: &gfx::Core, image: SwapchainImage, raster_finish: vk::Semaphore) -> anyhow::Result<()> {
		unsafe {
			core.swapchain_fns.queue_present(
				core.vk_queue,
				&vk::PresentInfoKHR::default()
					.swapchains(&[self.vk_swapchain])
					.image_indices(&[image.image_index])
					.wait_semaphores(&[raster_finish])
			).context("Presenting to swapchain")?;
		}

		Ok(())
	}
}


struct SwapchainImage {
	vk_image: vk::Image,
	vk_image_view: vk::ImageView,
	image_index: u32,
}