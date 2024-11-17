use ash::vk;
use anyhow::Context;
use winit::window::Window;
use crate::gfx;



pub struct Frame {
	pub vk_swapchain_image: vk::Image,
	pub vk_swapchain_image_view: vk::ImageView,
	pub vk_cmd_buffer: vk::CommandBuffer,

	sync_index: usize,
	image_index: usize,
}

struct FrameSync {
	image_available_semaphore: vk::Semaphore,
	raster_finish_semaphore: vk::Semaphore,

	prev_submit_timeline_value: u64,
}


pub struct PresentableSurface {
	vk_surface: vk::SurfaceKHR,

	vk_swapchain: vk::SwapchainKHR,
	vk_swapchain_images: Vec<vk::Image>,
	vk_swapchain_image_views: Vec<vk::ImageView>,

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

		const NO_CURRENT_EXTENT: vk::Extent2D = vk::Extent2D{ width: u32::MAX, height: u32::MAX };
		let swapchain_extent = match surface_capabilities.current_extent {
			NO_CURRENT_EXTENT => {
				let (width, height) = window.inner_size().into();
				vk::Extent2D{ width, height }
			},

			current => current,
		};

		let supported_formats = unsafe{ core.surface_fns.get_physical_device_surface_formats(core.vk_physical_device, vk_surface)? };
		let supported_present_modes = unsafe{ core.surface_fns.get_physical_device_surface_present_modes(core.vk_physical_device, vk_surface)? };

		log::info!("Supported formats: {supported_formats:#?}");
		log::info!("Supported present modes: {supported_present_modes:?}");

		dbg!(&surface_capabilities, &supported_formats, &supported_present_modes);

		let selected_present_mode = supported_present_modes.into_iter()
			.max_by_key(|&mode| match mode {
				vk::PresentModeKHR::FIFO => 1,
				vk::PresentModeKHR::FIFO_RELAXED => 2,
				vk::PresentModeKHR::MAILBOX => 10,
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

		let selected_format_srgb = match selected_format {
			vk::Format::R8G8B8A8_UNORM => vk::Format::R8G8B8A8_SRGB,
			vk::Format::B8G8R8A8_UNORM => vk::Format::B8G8R8A8_SRGB,
			vk::Format::A8B8G8R8_UNORM_PACK32 => vk::Format::A8B8G8R8_SRGB_PACK32,
			x => x,
		};

		let max_images = match surface_capabilities.max_image_count {
			0 => u32::MAX,
			n => n
		};

		let num_images = (surface_capabilities.min_image_count + 1).min(max_images);

		let needs_mutable_format = selected_format != selected_format_srgb;
		let swapchain_create_flags = if needs_mutable_format { vk::SwapchainCreateFlagsKHR::MUTABLE_FORMAT } else { vk::SwapchainCreateFlagsKHR::empty() };

		log::info!("Selected present mode: {selected_present_mode:?}");
		log::info!("Selected swapchain format: {selected_format:?}");

		let vk_swapchain = unsafe {
			let formats = [selected_format, selected_format_srgb];

			core.swapchain_fns.create_swapchain(
				&vk::SwapchainCreateInfoKHR::default()
					.surface(vk_surface)
					.min_image_count(num_images)
					.flags(swapchain_create_flags)
					.image_format(selected_format)
					.image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
					.image_extent(swapchain_extent)
					.image_array_layers(1)
					.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
					.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
					.pre_transform(surface_capabilities.current_transform)
					.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
					.present_mode(selected_present_mode)
					.old_swapchain(vk::SwapchainKHR::null())
					.clipped(true)
					// TODO(pat.m): push if supported
					.push_next(
						&mut vk::ImageFormatListCreateInfo::default()
							.view_formats(match needs_mutable_format {
								true => &formats[0..2],
								false => &formats[0..1],
							})
					),
				None
			)?
		};

		// Swapchain images
		let vk_swapchain_images = unsafe { core.swapchain_fns.get_swapchain_images(vk_swapchain)? };

		let vk_swapchain_image_views: Vec<_> = vk_swapchain_images.iter()
			.map(|&image| unsafe {
				let create_info = vk::ImageViewCreateInfo::default()
					.image(image)
					.view_type(vk::ImageViewType::TYPE_2D)
					.format(selected_format_srgb)
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

				core.vk_device.create_image_view(&create_info, None).unwrap()
			})
			.collect();


		// command buffers
		let vk_cmd_buffers = unsafe {
			let create_info = vk::CommandBufferAllocateInfo::default()
				.command_buffer_count(vk_swapchain_images.len() as u32)
				.command_pool(core.vk_cmd_pool)
				.level(vk::CommandBufferLevel::PRIMARY);

			core.vk_device.allocate_command_buffers(&create_info)?
		};

		let frame_syncs = (0..vk_swapchain_images.len())
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

			vk_swapchain,
			vk_swapchain_images,
			vk_swapchain_image_views,

			frame_syncs,
			next_sync_index: 0,

			vk_cmd_buffers,

			swapchain_extent,
			swapchain_format: selected_format,
			swapchain_present_mode: selected_present_mode,
			num_swapchain_images: num_images,
		})
	}

	pub fn destroy(self, core: &gfx::Core) {
		core.wait_idle();

		unsafe {
			for image_view in self.vk_swapchain_image_views {
				core.vk_device.destroy_image_view(image_view, None);
			}
			for frame_sync in self.frame_syncs {
				core.vk_device.destroy_semaphore(frame_sync.image_available_semaphore, None);
				core.vk_device.destroy_semaphore(frame_sync.raster_finish_semaphore, None);
			}

			core.swapchain_fns.destroy_swapchain(self.vk_swapchain, None);
			core.surface_fns.destroy_surface(self.vk_surface, None);
		}
	}

	pub fn resize(&mut self, core: &gfx::Core, deletion_queue: &mut gfx::DeletionQueue, new_size: vk::Extent2D) {
		if self.swapchain_extent == new_size {
			return;
		}

		log::info!("Resize event {new_size:?}");

		let surface_capabilities = core.get_surface_capabilities(self.vk_surface).unwrap();
		log::info!("Surface capabilities: {surface_capabilities:#?}");

		if new_size.width < surface_capabilities.min_image_extent.width
			|| new_size.width > surface_capabilities.max_image_extent.width
			|| new_size.width == 0
			|| new_size.height < surface_capabilities.min_image_extent.height
			|| new_size.height > surface_capabilities.max_image_extent.height
			|| new_size.height == 0
		{
			// TODO(pat.m): skip rendering
			self.swapchain_extent = vk::Extent2D{ width: 0, height: 0 };
			return;
		}

		self.swapchain_extent = new_size;

		deletion_queue.queue_deletion(core, self.vk_swapchain);
		for image_view in self.vk_swapchain_image_views.iter() {
			deletion_queue.queue_deletion(core, *image_view);
		}

		let swapchain_format_srgb = match self.swapchain_format {
			vk::Format::R8G8B8A8_UNORM => vk::Format::R8G8B8A8_SRGB,
			vk::Format::B8G8R8A8_UNORM => vk::Format::B8G8R8A8_SRGB,
			vk::Format::A8B8G8R8_UNORM_PACK32 => vk::Format::A8B8G8R8_SRGB_PACK32,
			x => x,
		};

		let formats = [self.swapchain_format, swapchain_format_srgb];
		let mut format_list_info = vk::ImageFormatListCreateInfo::default()
			.view_formats(&formats);

		let mut swapchain_info = vk::SwapchainCreateInfoKHR::default()
			.surface(self.vk_surface)
			.min_image_count(self.num_swapchain_images)
			.image_format(self.swapchain_format)
			.image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
			.image_extent(new_size)
			.image_array_layers(1)
			.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
			.pre_transform(surface_capabilities.current_transform)
			.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
			.present_mode(self.swapchain_present_mode)
			.old_swapchain(self.vk_swapchain)
			.clipped(true);

		if swapchain_format_srgb != self.swapchain_format {
			swapchain_info = swapchain_info
				.flags(vk::SwapchainCreateFlagsKHR::MUTABLE_FORMAT)
				.push_next(&mut format_list_info);
		}

		let vk_swapchain = unsafe { core.swapchain_fns.create_swapchain(&swapchain_info, None).expect("Creating swapchain") };
		let vk_swapchain_images = unsafe { core.swapchain_fns.get_swapchain_images(vk_swapchain).expect("Getting swapchain images") };

		let vk_swapchain_image_views: Vec<_> = vk_swapchain_images.iter()
			.map(|&image| unsafe {
				let create_info = vk::ImageViewCreateInfo::default()
					.image(image)
					.view_type(vk::ImageViewType::TYPE_2D)
					.format(swapchain_format_srgb)
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

				core.vk_device.create_image_view(&create_info, None).expect("Creating swapchain image views")
			})
			.collect();

		self.vk_swapchain = vk_swapchain;
		self.vk_swapchain_images = vk_swapchain_images;
		self.vk_swapchain_image_views = vk_swapchain_image_views;
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

		let (image_index, _) = unsafe {
			core.swapchain_fns.acquire_next_image(
				self.vk_swapchain,
				timeout_ns,
				frame_sync.image_available_semaphore,
				vk::Fence::null()
			).context("Acquiring swapchain image")?
		};

		let image_index = image_index as usize;

		let vk_swapchain_image = self.vk_swapchain_images[image_index];
		let vk_swapchain_image_view = self.vk_swapchain_image_views[image_index];

		unsafe {
			core.vk_device.begin_command_buffer(vk_cmd_buffer,
				&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;

			core.vk_device.cmd_pipeline_barrier2(
				vk_cmd_buffer,
				&vk::DependencyInfo::default()
					.image_memory_barriers(&[
						vk::ImageMemoryBarrier2::default()
							.image(vk_swapchain_image)
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
			vk_swapchain_image,
			vk_swapchain_image_view,
			vk_cmd_buffer,

			sync_index,
			image_index,
		})
	}

	pub fn submit_frame(&mut self, core: &gfx::Core, frame: Frame) -> anyhow::Result<()> {
		let frame_sync = &mut self.frame_syncs[frame.sync_index];

		unsafe {
			core.vk_device.cmd_pipeline_barrier2(
				frame.vk_cmd_buffer,
				&vk::DependencyInfo::default()
					.image_memory_barriers(&[
						vk::ImageMemoryBarrier2::default()
							.image(frame.vk_swapchain_image)
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

			core.swapchain_fns.queue_present(
				core.vk_queue,
				&vk::PresentInfoKHR::default()
					.swapchains(&[self.vk_swapchain])
					.image_indices(&[frame.image_index as u32])
					.wait_semaphores(&[frame_sync.raster_finish_semaphore])
			).context("Presenting to swapchain")?;
		}

		Ok(())
	}
}