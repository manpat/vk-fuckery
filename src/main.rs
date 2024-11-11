use winit::{
	application::ApplicationHandler,
	event::{WindowEvent, /*ElementState*/},
	event_loop::{EventLoop, ActiveEventLoop, ControlFlow},
	window::{Window, WindowId},
	dpi::LogicalSize,
};

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

// fn main_2() -> anyhow::Result<()> {





// 	// Load shaders
// 	fn create_shader_module(device: &ash::Device, path: impl AsRef<std::path::Path>) -> anyhow::Result<vk::ShaderModule> {
// 		let contents = std::fs::read(path)?;
// 		anyhow::ensure!(contents.len() % 4 == 0);

// 		unsafe {
// 			let contents = std::slice::from_raw_parts(contents.as_ptr().cast(), contents.len()/4);

// 			let create_info = vk::ShaderModuleCreateInfo::default()
// 				.code(contents);

// 			Ok(device.create_shader_module(&create_info, None)?)
// 		}
// 	}

// 	let vert_sh = create_shader_module(&vk_device, "shaders/main.vs.spv").unwrap();
// 	let frag_sh = create_shader_module(&vk_device, "shaders/main.fs.spv").unwrap();

// 	let vk_pipeline_layout = unsafe {
// 		vk_device.create_pipeline_layout(
// 			&vk::PipelineLayoutCreateInfo::default(),
// 			None
// 		)?
// 	};

// 	let vk_pipeline = unsafe {
// 		let shader_stages = [
// 			vk::PipelineShaderStageCreateInfo::default()
// 				.module(vert_sh)
// 				.name(c"main")
// 				.stage(vk::ShaderStageFlags::VERTEX),

// 			vk::PipelineShaderStageCreateInfo::default()
// 				.module(frag_sh)
// 				.name(c"main")
// 				.stage(vk::ShaderStageFlags::FRAGMENT),
// 		];

// 		let vertex_input_state = vk::PipelineVertexInputStateCreateInfo::default();

// 		let ia_state = vk::PipelineInputAssemblyStateCreateInfo::default()
// 			.topology(vk::PrimitiveTopology::TRIANGLE_LIST);

// 		let viewports = [vk::Viewport {
// 			x: 0.0,
// 			y: 0.0,
// 			width: swapchain_extent.width as f32,
// 			height: swapchain_extent.height as f32,
// 			min_depth: 0.0,
// 			max_depth: 1.0,
// 		}];

// 		let scissors = [vk::Rect2D {
// 			offset: vk::Offset2D { x: 0, y: 0 },
// 			extent: swapchain_extent,
// 		}];

// 		let viewport_state = vk::PipelineViewportStateCreateInfo::default()
// 			.scissors(&scissors)
// 			.viewports(&viewports);

// 		let raster_state = vk::PipelineRasterizationStateCreateInfo::default()
// 			.cull_mode(vk::CullModeFlags::BACK)
// 			.front_face(vk::FrontFace::COUNTER_CLOCKWISE)
// 			.polygon_mode(vk::PolygonMode::FILL)
// 			.line_width(1.0);

// 		let ms_state = vk::PipelineMultisampleStateCreateInfo::default()
// 			.rasterization_samples(vk::SampleCountFlags::TYPE_1);

// 		let graphic_pipeline_create_infos = [
// 			vk::GraphicsPipelineCreateInfo::default()
// 				.stages(&shader_stages)
// 				.layout(vk_pipeline_layout)
// 				.vertex_input_state(&vertex_input_state)
// 				.input_assembly_state(&ia_state)
// 				.viewport_state(&viewport_state)
// 				.rasterization_state(&raster_state)
// 				.multisample_state(&ms_state)
// 		];

// 		let pipelines = vk_device.create_graphics_pipelines(vk::PipelineCache::null(), &graphic_pipeline_create_infos, None).unwrap();

// 		vk_device.destroy_shader_module(vert_sh, None);
// 		vk_device.destroy_shader_module(frag_sh, None);

// 		pipelines[0]
// 	};


// 	// Set up frame sync
// 	struct Sync {
// 		image_available: vk::Semaphore,
// 		render_finished: vk::Semaphore,
// 		in_flight_fence: vk::Fence,
// 	}

// 	let sync_objects: Vec<_> = (0..swapchain_images.len())
// 		.map(|_| unsafe {
// 			Sync {
// 				image_available: vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap(),
// 				render_finished: vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap(),
// 				in_flight_fence: vk_device.create_fence(&vk::FenceCreateInfo::default().flags(vk::FenceCreateFlags::SIGNALED), None).unwrap(),
// 			}
// 		})
// 		.collect();

// 	let timeline_semaphore = unsafe {
// 		let timeline_create_info = vk::SemaphoreTypeCreateInfo::default()
// 			.semaphore_type(vk::SemaphoreType::TIMELINE)
// 			.initial_value(0);

// 		vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default().push_next(&mut timeline_create_info), None)
// 	};

// 	let mut frame_number = 0;


// 	let mut destroying = false;

// 	event_loop.run(move |event, _, control_flow| {
// 		match event {
// 			// Render a frame if our Vulkan app is not being destroyed.
// 			Event::MainEventsCleared if !destroying => {
// 				unsafe {
// 					let timeout_ns = 1000*1000*1000;

// 					let frame_sync = &sync_objects[frame_number];

// 					vk_device.wait_for_fences(&[frame_sync.in_flight_fence], true, timeout_ns).unwrap();
// 					vk_device.reset_fences(&[frame_sync.in_flight_fence]).unwrap();

// 					let (image_idx, _) = swapchain_fn.acquire_next_image(
// 						vk_swapchain,
// 						timeout_ns,
// 						frame_sync.image_available,
// 						vk::Fence::null()
// 					).unwrap();

// 					// Build the command buffer
// 					{
// 						let cmd_buffer = vk_cmd_buffers[frame_number];
// 						let swapchain_image = swapchain_images[image_idx as usize];
// 						let swapchain_image_view = swapchain_image_views[image_idx as usize];

// 						let begin_info = vk::CommandBufferBeginInfo::default()
// 							.flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

// 						vk_device.begin_command_buffer(cmd_buffer, &begin_info).unwrap();

// 						vk_device.cmd_pipeline_barrier2(
// 							cmd_buffer,
// 							&vk::DependencyInfo::default()
// 								.image_memory_barriers(&[
// 									vk::ImageMemoryBarrier2::default()
// 										.image(swapchain_image)
// 										.old_layout(vk::ImageLayout::UNDEFINED)
// 										.new_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)

// 										.src_stage_mask(vk::PipelineStageFlags2::NONE)
// 										.src_access_mask(vk::AccessFlags2::NONE)

// 										.dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
// 										.dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
// 										.subresource_range(
// 											vk::ImageSubresourceRange::default()
// 												.aspect_mask(vk::ImageAspectFlags::COLOR)
// 												.base_mip_level(0)
// 												.base_array_layer(0)
// 												.level_count(1)
// 												.layer_count(1)
// 										)
// 								]
// 							)
// 						);

// 						let color_attachments = [
// 							vk::RenderingAttachmentInfo::default()
// 								.image_view(swapchain_image_view)
// 								.image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
// 								.load_op(vk::AttachmentLoadOp::CLEAR)
// 								.store_op(vk::AttachmentStoreOp::STORE)
// 								.clear_value(vk::ClearValue {
// 									color: vk::ClearColorValue {
// 										float32: match frame_number {
// 											0 => [1.0, 0.5, 1.0, 1.0],
// 											1 => [0.5, 1.0, 1.0, 1.0],
// 											2 => [1.0, 1.0, 0.5, 1.0],
// 											_ => [0.0; 4]
// 										}
// 									},
// 								})
// 						];

// 						let render_info = vk::RenderingInfo::default()
// 							.layer_count(1)
// 							.render_area(vk::Rect2D {
// 								offset: vk::Offset2D { x: 0, y: 0 },
// 								extent: swapchain_extent,
// 							})
// 							.color_attachments(&color_attachments);

// 						vk_device.cmd_begin_rendering(cmd_buffer, &render_info);

// 						vk_device.cmd_bind_pipeline(cmd_buffer, vk::PipelineBindPoint::GRAPHICS, vk_pipeline);
// 						vk_device.cmd_draw(cmd_buffer, 3, 1, 0, 0);

// 						vk_device.cmd_end_rendering(cmd_buffer);

// 						vk_device.cmd_pipeline_barrier2(
// 							cmd_buffer,
// 							&vk::DependencyInfo::default()
// 								.image_memory_barriers(&[
// 									vk::ImageMemoryBarrier2::default()
// 										.image(swapchain_image)
// 										.old_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
// 										.new_layout(vk::ImageLayout::PRESENT_SRC_KHR)

// 										.src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
// 										.src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)

// 										.dst_stage_mask(vk::PipelineStageFlags2::NONE)
// 										.dst_access_mask(vk::AccessFlags2::NONE)
// 										.subresource_range(
// 											vk::ImageSubresourceRange::default()
// 												.aspect_mask(vk::ImageAspectFlags::COLOR)
// 												.base_mip_level(0)
// 												.base_array_layer(0)
// 												.level_count(1)
// 												.layer_count(1)
// 										)
// 								]
// 							)
// 						);

// 						vk_device.end_command_buffer(cmd_buffer).unwrap();
// 					}

// 					vk_device.queue_submit(vk_graphics_queue,
// 						&[vk::SubmitInfo::default()
// 							.command_buffers(&[vk_cmd_buffers[image_idx as usize]])
// 							.wait_semaphores(&[frame_sync.image_available])
// 							.wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
// 							.signal_semaphores(&[frame_sync.render_finished])
// 						],
// 						frame_sync.in_flight_fence
// 					).unwrap();

// 					swapchain_fn.queue_present(vk_graphics_queue,
// 						&vk::PresentInfoKHR::default()
// 							.swapchains(&[vk_swapchain])
// 							.image_indices(&[image_idx])
// 							.wait_semaphores(&[frame_sync.render_finished])
// 					).unwrap();

// 					frame_number = (frame_number + 1) % sync_objects.len();
// 				}
// 			}

// 			// Event::WindowEvent { event: WindowEvent::CloseRequested
// 			// 	| WindowEvent::KeyboardInput{ input: KeyboardInput {
// 			// 		state: ElementState::Pressed,
// 			// 		virtual_keycode: Some(VirtualKeyCode::Escape),
// 			// 		..
// 			// 	}, .. }, .. } =>
// 			// {
// 			// 	destroying = true;
// 			// 	control_flow.set_exit();
// 			// }

// 			Event::LoopDestroyed => unsafe {
// 				vk_device.device_wait_idle().unwrap();

// 				vk_device.destroy_pipeline(vk_pipeline, None);
// 				vk_device.destroy_pipeline_layout(vk_pipeline_layout, None);

// 				vk_device.destroy_command_pool(vk_cmd_pool, None);

// 				for &Sync{image_available, render_finished, in_flight_fence} in sync_objects.iter() {
// 					vk_device.destroy_semaphore(image_available, None);
// 					vk_device.destroy_semaphore(render_finished, None);
// 					vk_device.destroy_fence(in_flight_fence, None);
// 				}

// 				for view in swapchain_image_views.iter() {
// 					vk_device.destroy_image_view(*view, None);
// 				}

// 				swapchain_fn.destroy_swapchain(vk_swapchain, None);
// 				surface_instance_fn.destroy_surface(vk_surface, None);
// 				vk_device.destroy_device(None);
// 				debug_utils.destroy_debug_utils_messenger(vk_messenger, None);
// 				vk_instance.destroy_instance(None);
// 			}
// 			_ => {}
// 		}

// 	})
// }



struct App {
	gfx_core: gfx::Core,
	window: Option<Window>,
	presentable_surface: Option<PresentableSurface>,
}

impl App {
	fn new(gfx_core: gfx::Core) -> App {
		App {
			gfx_core,
			window: None,
			presentable_surface: None,
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

			let render_info = vk::RenderingInfo::default()
				.layer_count(1)
				.render_area(vk::Rect2D {
					offset: vk::Offset2D { x: 0, y: 0 },
					extent: presentable_surface.swapchain_extent,
				})
				.color_attachments(&color_attachments);

			self.gfx_core.vk_device.cmd_begin_rendering(frame.vk_cmd_buffer, &render_info);

			// self.gfx_core.vk_device.cmd_bind_pipeline(frame.vk_cmd_buffer, vk::PipelineBindPoint::GRAPHICS, vk_pipeline);
			// self.gfx_core.vk_device.cmd_draw(frame.vk_cmd_buffer, 3, 1, 0, 0);

			self.gfx_core.vk_device.cmd_end_rendering(frame.vk_cmd_buffer);
		}

		presentable_surface.submit_frame(&self.gfx_core, frame).unwrap();

		self.window = Some(window);
		self.presentable_surface = Some(presentable_surface);
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
		match event {
			WindowEvent::CloseRequested => {
				event_loop.exit();
			},

			WindowEvent::RedrawRequested => {
				// Redraw the application.
				//
				// It's preferable for applications that do not render continuously to render in
				// this event rather than in AboutToWait, since rendering in here allows
				// the program to gracefully handle redraws requested by the OS.

				// Draw.

				// Queue a RedrawRequested event.
				//
				// You only need to call this if you've determined that you need to redraw in
				// applications which do not always need to. Applications that redraw continuously
				// can render here instead.
				self.window.as_ref().unwrap().request_redraw();
			}
			_ => (),
		}
	}
}


use ash::vk;

pub struct PresentableSurface {
	vk_surface: vk::SurfaceKHR,

	vk_swapchain: vk::SwapchainKHR,
	vk_swapchain_images: Vec<vk::Image>,
	vk_swapchain_image_views: Vec<vk::ImageView>,
	vk_image_available_semaphores: Vec<vk::Semaphore>,

	vk_cmd_buffers: Vec<vk::CommandBuffer>,

	swapchain_extent: vk::Extent2D,
	frame_number: usize,
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

		let vk_swapchain = unsafe {
			let supported_formats = core.surface_fns.get_physical_device_surface_formats(core.vk_physical_device, vk_surface)?;
			let supported_present_modes = core.surface_fns.get_physical_device_surface_present_modes(core.vk_physical_device, vk_surface)?;

			log::info!("Supported formats: {supported_formats:?}");
			log::info!("Supported present modes: {supported_present_modes:?}");

			dbg!(swapchain_capabilities, supported_formats, &supported_present_modes);

			assert!(supported_present_modes.contains(&vk::PresentModeKHR::FIFO));

			let max_images = match swapchain_capabilities.max_image_count {
				0 => u32::MAX,
				n => n
			};

			let num_images = (swapchain_capabilities.min_image_count + 1).min(max_images);

			let create_info = vk::SwapchainCreateInfoKHR::default()
				.surface(vk_surface)
				.min_image_count(num_images)
				// TODO(pat.m): get these from supported_formats
				.image_format(vk::Format::B8G8R8A8_SRGB)
				.image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
				.image_extent(swapchain_extent)
				.image_array_layers(1)
				.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
				.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
				.pre_transform(swapchain_capabilities.current_transform)
				.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
				// TODO(pat.m): get this from supported_present_modes
				.present_mode(vk::PresentModeKHR::FIFO)
				.clipped(true);

			core.swapchain_fns.create_swapchain(&create_info, None)?
		};

		// Swapchain images
		let vk_swapchain_images = unsafe { core.swapchain_fns.get_swapchain_images(vk_swapchain)? };

		let vk_swapchain_image_views: Vec<_> = vk_swapchain_images.iter()
			.map(|&image| unsafe {
				let create_info = vk::ImageViewCreateInfo::default()
					.image(image)
					.view_type(vk::ImageViewType::TYPE_2D)
					.format(vk::Format::B8G8R8A8_SRGB)
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

		Ok(PresentableSurface {
			vk_surface,

			vk_swapchain,
			vk_swapchain_images,
			vk_swapchain_image_views,

			vk_cmd_buffers,

			swapchain_extent,
			frame_number: 0,
		})
	}

	fn start_frame(&self, core: &gfx::Core) -> anyhow::Result<Frame> {
		let timeout_ns = 1000*1000*1000;
		let (image_idx, _) = unsafe {
			core.swapchain_fns.acquire_next_image(
				self.vk_swapchain,
				timeout_ns,
				vk::Semaphore::null(),
				vk::Fence::null()
			)?
		};

		let image_idx = image_idx as usize;
		let vk_cmd_buffer = self.vk_cmd_buffers[image_idx];
		let vk_swapchain_image = self.vk_swapchain_images[image_idx];
		let vk_swapchain_image_view = self.vk_swapchain_image_views[image_idx];

		unsafe {
			core.vk_device.begin_command_buffer(vk_cmd_buffer,
				&vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT))?;

			core.vk_device.cmd_pipeline_barrier2(
				vk_cmd_buffer,
				&vk::DependencyInfo::default()
					.image_memory_barriers(&[
						vk::ImageMemoryBarrier2::default()
							.image(vk_swapchain_image)
							.old_layout(vk::ImageLayout::UNDEFINED)
							.new_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)

							.src_stage_mask(vk::PipelineStageFlags2::NONE)
							.src_access_mask(vk::AccessFlags2::NONE)

							.dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
							.dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
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

			image_idx,
		})
	}

	fn submit_frame(&self, core: &gfx::Core, frame: Frame) -> anyhow::Result<()> {
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

			core.vk_device.queue_submit(
				core.vk_queue,
				&[vk::SubmitInfo::default()
					.command_buffers(&[frame.vk_cmd_buffer])
					// .wait_semaphores(&[frame_sync.image_available])
					.wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
					// .signal_semaphores(&[frame_sync.render_finished])
				],
				// frame_sync.in_flight_fence
				vk::Fence::null()
			)?;

			core.vk_device.device_wait_idle()?;

			core.swapchain_fns.queue_present(
				core.vk_queue,
				&vk::PresentInfoKHR::default()
					.swapchains(&[self.vk_swapchain])
					.image_indices(&[frame.image_idx as u32])
					// .wait_semaphores(&[frame_sync.render_finished])
			)?;

			core.vk_device.end_command_buffer(frame.vk_cmd_buffer)?;
		}

		Ok(())
	}
}


pub struct Frame {
	vk_swapchain_image: vk::Image,
	vk_swapchain_image_view: vk::ImageView,
	vk_cmd_buffer: vk::CommandBuffer,

	image_idx: usize,
}