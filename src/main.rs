use winit::{
	application::ApplicationHandler,
	event::{WindowEvent, /*ElementState*/},
	event_loop::{EventLoop, ActiveEventLoop, ControlFlow},
	window::{Window, WindowId},
	dpi::LogicalSize,
};

use anyhow::Context;

// use ash::prelude::*;
// use ash::vk;

mod gfx;


fn main() -> anyhow::Result<()> {
	{
		use simplelog::*;
		use std::fs::File;

		CombinedLogger::init(vec![
			TermLogger::new(LevelFilter::Warn, Config::default(), TerminalMode::Mixed, ColorChoice::Never),
			WriteLogger::new(LevelFilter::Info, Config::default(), File::create("vk-fuck.log").unwrap()),
		]).unwrap();
	}

	let event_loop = EventLoop::new()?;
	event_loop.set_control_flow(ControlFlow::Poll);

	let gfx_core = gfx::Core::new(event_loop.owned_display_handle())?;

	if let Err(err) = event_loop.run_app(&mut App::new(gfx_core)) {
		log::error!("\nExited with error: {err}");
	}

	Ok(())
}




struct App {
	gfx_core: gfx::Core,
	window: Option<Window>,
	presentable_surface: Option<PresentableSurface>,

	vk_pipeline: vk::Pipeline,
}

impl App {
	fn new(gfx_core: gfx::Core) -> App {
		App {
			gfx_core,
			window: None,
			presentable_surface: None,
			vk_pipeline: vk::Pipeline::null(),
		}
	}
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let window_attrs = Window::default_attributes()
			.with_title("Vk Fuck")
			.with_inner_size(LogicalSize::new(1024, 768));

		let window = event_loop.create_window(window_attrs).unwrap();
		let presentable_surface = PresentableSurface::new(&self.gfx_core, &window).unwrap();

		let vert_sh = create_shader_module(&self.gfx_core.vk_device, "shaders/main.vs.spv").unwrap();
		let frag_sh = create_shader_module(&self.gfx_core.vk_device, "shaders/main.fs.spv").unwrap();


		let vk_pipeline = unsafe {
			let shader_stages = [
				vk::PipelineShaderStageCreateInfo::default()
					.module(vert_sh)
					.name(c"main")
					.stage(vk::ShaderStageFlags::VERTEX),

				vk::PipelineShaderStageCreateInfo::default()
					.module(frag_sh)
					.name(c"main")
					.stage(vk::ShaderStageFlags::FRAGMENT),
			];

			let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default();

			let ia_state = vk::PipelineInputAssemblyStateCreateInfo::default()
				.topology(vk::PrimitiveTopology::TRIANGLE_LIST);

			let viewport_state = vk::PipelineViewportStateCreateInfo::default()
				.scissor_count(1)
				.viewport_count(1);

			let raster_state = vk::PipelineRasterizationStateCreateInfo::default()
				.cull_mode(vk::CullModeFlags::BACK)
				.front_face(vk::FrontFace::COUNTER_CLOCKWISE)
				.polygon_mode(vk::PolygonMode::FILL)
				.line_width(1.0);

			// TODO(pat.m): this should probably also be dynamic
			let ms_state = vk::PipelineMultisampleStateCreateInfo::default()
				.rasterization_samples(vk::SampleCountFlags::TYPE_1);

			let dynamic_states = [
				vk::DynamicState::VIEWPORT,
				vk::DynamicState::SCISSOR,
			];

			let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
				.dynamic_states(&dynamic_states);

			let vk_pipeline_layout = self.gfx_core.vk_device.create_pipeline_layout(&Default::default(), None).unwrap();

			let graphic_pipeline_create_infos = [
				vk::GraphicsPipelineCreateInfo::default()
					.stages(&shader_stages)
					.layout(vk_pipeline_layout)
					.vertex_input_state(&vertex_input_state)
					.input_assembly_state(&ia_state)
					.viewport_state(&viewport_state)
					.rasterization_state(&raster_state)
					.multisample_state(&ms_state)
					.dynamic_state(&dynamic_state)
			];

			let pipelines = self.gfx_core.vk_device.create_graphics_pipelines(vk::PipelineCache::null(), &graphic_pipeline_create_infos, None).unwrap();

			self.gfx_core.vk_device.destroy_shader_module(vert_sh, None);
			self.gfx_core.vk_device.destroy_shader_module(frag_sh, None);

			self.gfx_core.vk_device.destroy_pipeline_layout(vk_pipeline_layout, None);

			pipelines[0]
		};

		self.window = Some(window);
		self.presentable_surface = Some(presentable_surface);
		self.vk_pipeline = vk_pipeline;
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
		match event {
			WindowEvent::CloseRequested => {
				event_loop.exit();
			},

			WindowEvent::RedrawRequested => {
				let presentable_surface = self.presentable_surface.as_mut().unwrap();
				let window = self.window.as_ref().unwrap();

				let frame = presentable_surface.start_frame(&self.gfx_core).unwrap();

				unsafe {
					let color_attachments = [
						vk::RenderingAttachmentInfo::default()
							.image_view(frame.vk_swapchain_image_view)
							.image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
							.load_op(vk::AttachmentLoadOp::CLEAR)
							.store_op(vk::AttachmentStoreOp::STORE)
							.clear_value(vk::ClearValue {
								color: vk::ClearColorValue {
									float32: [1.0, 0.5, 1.0, 1.0],
								},
							})
					];

					let render_area = vk::Rect2D {
						offset: vk::Offset2D { x: 0, y: 0 },
						extent: presentable_surface.swapchain_extent,
					};

					let render_info = vk::RenderingInfo::default()
						.layer_count(1)
						.render_area(render_area)
						.color_attachments(&color_attachments);

					self.gfx_core.vk_device.cmd_begin_rendering(frame.vk_cmd_buffer, &render_info);

					// Set dynamic state
					self.gfx_core.vk_device.cmd_set_scissor(frame.vk_cmd_buffer, 0, &[render_area]);
					self.gfx_core.vk_device.cmd_set_viewport(frame.vk_cmd_buffer, 0, &[vk::Viewport {
						x: 0.0, y: 0.0,
						width: presentable_surface.swapchain_extent.width as f32,
						height: presentable_surface.swapchain_extent.height as f32,
						min_depth: 0.0,
						max_depth: 1.0,
					}]);

					// Draw
					self.gfx_core.vk_device.cmd_bind_pipeline(frame.vk_cmd_buffer, vk::PipelineBindPoint::GRAPHICS, self.vk_pipeline);
					self.gfx_core.vk_device.cmd_draw(frame.vk_cmd_buffer, 3, 1, 0, 0);

					self.gfx_core.vk_device.cmd_end_rendering(frame.vk_cmd_buffer);
				}


				window.pre_present_notify();
				presentable_surface.submit_frame(&self.gfx_core, frame).unwrap();

				window.request_redraw();
			}
			_ => (),
		}
	}

	fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
		self.gfx_core.wait_idle();

		unsafe {
			self.gfx_core.vk_device.destroy_pipeline(self.vk_pipeline, None);
		}

		if let Some(presentable_surface) = self.presentable_surface.take() {
			presentable_surface.destroy(&self.gfx_core);
		}
	}
}


use ash::vk;

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

	swapchain_extent: vk::Extent2D,
}

impl PresentableSurface {
	fn new(core: &gfx::Core, window: &Window) -> anyhow::Result<PresentableSurface> {
		let vk_surface = core.create_surface(&window)?;

		// Swapchain
		let swapchain_capabilities = core.get_surface_capabilities(vk_surface)?;

		const NO_CURRENT_EXTENT: vk::Extent2D = vk::Extent2D{ width: u32::MAX, height: u32::MAX };
		let swapchain_extent = match swapchain_capabilities.current_extent {
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

		dbg!(&swapchain_capabilities, &supported_formats, &supported_present_modes);

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

		let needs_mutable_format = selected_format != selected_format_srgb;

		log::info!("Selected present mode: {selected_present_mode:?}");
		log::info!("Selected swapchain format: {selected_format:?}");

		let vk_swapchain = unsafe {
			let max_images = match swapchain_capabilities.max_image_count {
				0 => u32::MAX,
				n => n
			};

			let num_images = (swapchain_capabilities.min_image_count + 1).min(max_images);
			let swapchain_create_flags = if needs_mutable_format { vk::SwapchainCreateFlagsKHR::MUTABLE_FORMAT } else { vk::SwapchainCreateFlagsKHR::empty() };
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
					.pre_transform(swapchain_capabilities.current_transform)
					.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
					.present_mode(selected_present_mode)
					.clipped(true)
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
		})
	}

	fn destroy(self, core: &gfx::Core) {
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

	fn start_frame(&mut self, core: &gfx::Core) -> anyhow::Result<Frame> {
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

	fn submit_frame(&mut self, core: &gfx::Core, frame: Frame) -> anyhow::Result<()> {
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
			).context("Submit")?;

			core.swapchain_fns.queue_present(
				core.vk_queue,
				&vk::PresentInfoKHR::default()
					.swapchains(&[self.vk_swapchain])
					.image_indices(&[frame.image_index as u32])
					.wait_semaphores(&[frame_sync.raster_finish_semaphore])
			).context("Present to swapchain")?;
		}

		Ok(())
	}
}


pub struct Frame {
	vk_swapchain_image: vk::Image,
	vk_swapchain_image_view: vk::ImageView,
	vk_cmd_buffer: vk::CommandBuffer,

	sync_index: usize,
	image_index: usize,
}



fn create_shader_module(device: &ash::Device, path: impl AsRef<std::path::Path>) -> anyhow::Result<vk::ShaderModule> {
	let contents = std::fs::read(path)?;
	anyhow::ensure!(contents.len() % 4 == 0);

	unsafe {
		let contents = std::slice::from_raw_parts(contents.as_ptr().cast(), contents.len()/4);

		let create_info = vk::ShaderModuleCreateInfo::default()
			.code(contents);

		Ok(device.create_shader_module(&create_info, None)?)
	}
}