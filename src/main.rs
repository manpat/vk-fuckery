use winit::{
	application::ApplicationHandler,
	event::{WindowEvent, /*ElementState*/},
	event_loop::{EventLoop, ActiveEventLoop, ControlFlow},
	window::{Window, WindowId},
	dpi::{LogicalSize, PhysicalSize},
};

use anyhow::Context;

// use ash::prelude::*;
use ash::vk;

mod gfx;


fn main() -> anyhow::Result<()> {
	{
		use simplelog::*;
		use std::fs::File;

		let file_config = ConfigBuilder::new()
			.add_filter_ignore_str("calloop")
			.add_filter_ignore_str("sctk")
			.build();

		let color_choice = match cfg!(windows) {
			false => ColorChoice::Auto,
			true => ColorChoice::Never,
		};

		CombinedLogger::init(vec![
			TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, color_choice),
			WriteLogger::new(LevelFilter::Trace, file_config, File::create("vk-fuck.log").unwrap()),
		]).unwrap();
	}

	let mut event_loop = EventLoop::builder();

	#[cfg(target_os="linux")]
	if false /* rendedoc attached */ {
		// Renderdoc doesn't support wayland :(
		// TODO(pat.m): better would be to actually check what instance extensions are available and
		// select a backend based on that
		use winit::platform::x11::EventLoopBuilderExtX11;
		event_loop.with_x11();
	}

	let event_loop = event_loop.build()?;

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
	presentable_surface: Option<gfx::PresentableSurface>,

	deletion_queue: gfx::DeletionQueue,
	allocator: gfx::DeviceAllocator,
	staging_buffer: gfx::StagingBuffer,

	vk_pipeline: vk::Pipeline,
	vk_pipeline_layout: vk::PipelineLayout,

	vk_depth_allocation: vk::DeviceMemory,
	vk_depth_image: vk::Image,
	vk_depth_view: vk::ImageView,

	time: f32,
}

impl App {
	fn new(gfx_core: gfx::Core) -> App {
		let vert_sh = create_shader_module(&gfx_core.vk_device, "shaders/main.vs.spv").unwrap();
		let frag_sh = create_shader_module(&gfx_core.vk_device, "shaders/main.fs.spv").unwrap();

		let (vk_pipeline, vk_pipeline_layout) = create_graphics_pipeline(&gfx_core, vert_sh, frag_sh).unwrap();

		unsafe {
			gfx_core.vk_device.destroy_shader_module(vert_sh, None);
			gfx_core.vk_device.destroy_shader_module(frag_sh, None);
		};

		let allocator = gfx::DeviceAllocator::new(&gfx_core).unwrap();
		let staging_buffer = gfx::StagingBuffer::new(&gfx_core, &allocator).unwrap();

		App {
			gfx_core,
			window: None,
			presentable_surface: None,
			deletion_queue: gfx::DeletionQueue::default(),
			allocator,
			staging_buffer,
			vk_pipeline,
			vk_pipeline_layout,

			vk_depth_allocation: vk::DeviceMemory::null(),
			vk_depth_image: vk::Image::null(),
			vk_depth_view: vk::ImageView::null(),

			time: 0.0,
		}
	}

	fn destroy_depth_attachment(&mut self) {
		if self.vk_depth_view != vk::ImageView::null() {
			self.deletion_queue.queue_deletion(self.vk_depth_view, &self.gfx_core);
			self.deletion_queue.queue_deletion(self.vk_depth_image, &self.gfx_core);
			self.deletion_queue.queue_deletion(self.vk_depth_allocation, &self.gfx_core);
		}

		self.vk_depth_view = vk::ImageView::null();
		self.vk_depth_image = vk::Image::null();
		self.vk_depth_allocation = vk::DeviceMemory::null();
	}

	fn recreate_depth_attachment(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
		self.destroy_depth_attachment();

		let image_create_info = vk::ImageCreateInfo::default()
			.image_type(vk::ImageType::TYPE_2D)
			.format(vk::Format::D32_SFLOAT)
			.samples(vk::SampleCountFlags::TYPE_1)
			.extent(vk::Extent3D{ width, height, depth: 1 })
			.mip_levels(1)
			.array_layers(1)
			.tiling(vk::ImageTiling::OPTIMAL)
			.usage(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT)
			.initial_layout(vk::ImageLayout::UNDEFINED)
			.sharing_mode(vk::SharingMode::EXCLUSIVE);

		unsafe {
			self.vk_depth_image = self.gfx_core.vk_device.create_image(&image_create_info, None)?;
			let requirements = self.gfx_core.vk_device.get_image_memory_requirements(self.vk_depth_image);

			self.vk_depth_allocation = self.allocator.allocate_device_memory(&self.gfx_core, requirements.size)?;
			self.gfx_core.vk_device.bind_image_memory(self.vk_depth_image, self.vk_depth_allocation, 0)?;

			let view_create_info = vk::ImageViewCreateInfo::default()
				.image(self.vk_depth_image)
				.view_type(vk::ImageViewType::TYPE_2D)
				.format(vk::Format::D32_SFLOAT)
				.components(
					// TODO(pat.m): should just be a const.
					vk::ComponentMapping {
						r: vk::ComponentSwizzle::R,
						g: vk::ComponentSwizzle::G,
						b: vk::ComponentSwizzle::B,
						a: vk::ComponentSwizzle::A,
					}
				)
				.subresource_range(
					vk::ImageSubresourceRange::default()
						.aspect_mask(vk::ImageAspectFlags::DEPTH)
						.base_mip_level(0)
						.base_array_layer(0)
						.level_count(1)
						.layer_count(1)
				);

			self.vk_depth_view = self.gfx_core.vk_device.create_image_view(&view_create_info, None)?;
		}

		Ok(())
	}
}

impl ApplicationHandler for App {
	fn resumed(&mut self, event_loop: &ActiveEventLoop) {
		let window_attrs = Window::default_attributes()
			.with_title("Vk Fuck")
			.with_inner_size(LogicalSize::new(1366, 768));

		let window = event_loop.create_window(window_attrs).unwrap();
		let presentable_surface = gfx::PresentableSurface::new(&self.gfx_core, &window).unwrap();

		self.window = Some(window);
		self.presentable_surface = Some(presentable_surface);
	}

	fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
		match event {
			WindowEvent::CloseRequested => {
				event_loop.exit();
			},

			WindowEvent::Resized(PhysicalSize{ width, height }) => {
				if let Some(presentable_surface) = self.presentable_surface.as_mut() {
					let result = presentable_surface.resize(&self.gfx_core, &mut self.deletion_queue, vk::Extent2D{width, height});
					if let Err(error) = result {
						log::error!("Failed to resize presentable surface: {error}");
					};

					self.recreate_depth_attachment(width, height).unwrap();
				}
			}

			WindowEvent::RedrawRequested => {
				self.time += std::f32::consts::PI / 60.0;

				let presentable_surface = self.presentable_surface.as_mut().unwrap();
				let window = self.window.as_ref().unwrap();

				self.deletion_queue.destroy_ready(&self.gfx_core);

				let frame = match presentable_surface.start_frame(&self.gfx_core) {
					Ok(frame) => frame,
					Err(err) => {
						log::error!("Unable to start frame: {err}");

						let extent = presentable_surface.swapchain_extent;

						// TODO(pat.m): YUCK
						let _ = presentable_surface.resize(&self.gfx_core, &mut self.deletion_queue, vk::Extent2D{width: 0, height: 0});
						let result = presentable_surface.resize(&self.gfx_core, &mut self.deletion_queue, extent);
						if let Err(error) = result {
							log::error!("Failed to resize presentable surface: {error}");
							return;
						};

						window.request_redraw();
						return;
					}
				};

				let vk_cmd_buffer = frame.cmd_buffer();
				let vk_swapchain_image = frame.swapchain_image_view();

				let render_area = vk::Rect2D {
					offset: vk::Offset2D { x: 0, y: 0 },
					extent: frame.extent,
				};

				#[derive(Copy, Clone, bytemuck::NoUninit)]
				#[repr(C)]
				struct GlobalBuffer {
					projection_view: [[f32; 4]; 4],
					time: f32,
				}

				let global_buffer_addr = self.staging_buffer.write(&GlobalBuffer {
					projection_view: {
						let aspect = frame.extent.width as f32 / frame.extent.height as f32;
						let xsc = 1.0 / aspect;
						let ysc = 1.0;
						let zsc = -1.0 / 10.0;
						let ztr = 1.0;

						[
							[xsc, 0.0, 0.0, 0.0],
							[0.0, ysc, 0.0, 0.0],
							[0.0, 0.0, zsc, 1.0],
							[0.0, 0.0, ztr, 1.0],
						]
					},

					time: self.time,
				});

				// Note: no barriers needed for host writes since vkQueueSubmit acts as an implicit memory barrier.

				unsafe {
					// Set dynamic state
					self.gfx_core.vk_device.cmd_set_scissor(vk_cmd_buffer, 0, &[render_area]);
					self.gfx_core.vk_device.cmd_set_viewport(vk_cmd_buffer, 0, &[vk::Viewport {
						x: render_area.offset.x as f32,
						y: render_area.offset.y as f32,
						width: render_area.extent.width as f32,
						height: render_area.extent.height as f32,
						min_depth: 0.0,
						max_depth: 1.0,
					}]);

					let color_attachments = [
						vk::RenderingAttachmentInfo::default()
							.image_view(vk_swapchain_image)
							.image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
							.load_op(vk::AttachmentLoadOp::CLEAR)
							.store_op(vk::AttachmentStoreOp::STORE)
							.clear_value(vk::ClearValue {
								color: vk::ClearColorValue {
									float32: [1.0, 0.5, 1.0, 1.0],
								},
							})
					];

					let depth_attachment = vk::RenderingAttachmentInfo::default()
						.image_view(self.vk_depth_view)
						.image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
						.load_op(vk::AttachmentLoadOp::CLEAR)
						.store_op(vk::AttachmentStoreOp::DONT_CARE)
						.clear_value(vk::ClearValue {
							depth_stencil: vk::ClearDepthStencilValue {
								depth: 0.0,
								stencil: 0,
							},
						});

					let render_info = vk::RenderingInfo::default()
						.layer_count(1)
						.render_area(render_area)
						.color_attachments(&color_attachments)
						.depth_attachment(&depth_attachment);

					self.gfx_core.vk_device.cmd_begin_rendering(vk_cmd_buffer, &render_info);

					// Draw
					self.gfx_core.vk_device.cmd_bind_pipeline(vk_cmd_buffer, vk::PipelineBindPoint::GRAPHICS, self.vk_pipeline);
					self.gfx_core.vk_device.cmd_push_constants(vk_cmd_buffer, self.vk_pipeline_layout, vk::ShaderStageFlags::ALL_GRAPHICS, 0, bytemuck::bytes_of(&global_buffer_addr));

					let offsets = [
						[0.0f32, 0.0, 0.0, 0.0],
						[1.0, 0.0, 1.0, 3.0],
						[-1.0, 0.0, 3.0, 6.0],
						[-0.5, 1.0, 2.0, 9.0],
					];

					for offset in offsets {
						let per_draw_addr = self.staging_buffer.write(&offset);
						self.gfx_core.vk_device.cmd_push_constants(vk_cmd_buffer, self.vk_pipeline_layout, vk::ShaderStageFlags::ALL_GRAPHICS, 8, bytemuck::bytes_of(&per_draw_addr));
						self.gfx_core.vk_device.cmd_draw(vk_cmd_buffer, 3, 1, 0, 0);
					}

					self.gfx_core.vk_device.cmd_end_rendering(vk_cmd_buffer);
				}

				window.pre_present_notify();

				if let Err(error) = presentable_surface.submit_frame(&self.gfx_core, frame) {
					log::error!("Present failed: {error}");
				}

				window.request_redraw();
			}
			_ => (),
		}
	}

	fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
		self.destroy_depth_attachment();

		self.deletion_queue.queue_deletion(self.vk_pipeline, &self.gfx_core);

		if let Some(presentable_surface) = self.presentable_surface.take() {
			presentable_surface.queue_deletion(&mut self.deletion_queue);
		}

		self.staging_buffer.queue_deletion(&mut self.deletion_queue);

		self.gfx_core.wait_idle();

		unsafe {
			self.deletion_queue.destroy_all_immediate(&self.gfx_core);

			// TODO(pat.m): deletion queue! although these can probably be destroyed as soon as we're done with them
			self.gfx_core.vk_device.destroy_pipeline_layout(self.vk_pipeline_layout, None);
		}
	}
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

fn create_graphics_pipeline(core: &gfx::Core, vert_sh: vk::ShaderModule, frag_sh: vk::ShaderModule) -> anyhow::Result<(vk::Pipeline, vk::PipelineLayout)> {
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

	let ms_state = vk::PipelineMultisampleStateCreateInfo::default()
		.rasterization_samples(vk::SampleCountFlags::TYPE_1);

	let dynamic_states = [
		vk::DynamicState::VIEWPORT,
		vk::DynamicState::SCISSOR,
	];

	let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
		.dynamic_states(&dynamic_states);

	let push_constant_ranges = [
		vk::PushConstantRange {
			stage_flags: vk::ShaderStageFlags::ALL_GRAPHICS,
			offset: 0,
			size: std::mem::size_of::<vk::DeviceAddress>() as u32 * 2,
		}
	];

	let pipeline_layout_info = vk::PipelineLayoutCreateInfo::default()
		.push_constant_ranges(&push_constant_ranges);

	let depth_stencil_state = vk::PipelineDepthStencilStateCreateInfo::default()
		.depth_test_enable(true)
		.depth_write_enable(true)
		.depth_compare_op(vk::CompareOp::GREATER_OR_EQUAL);

	let mut rendering_create_info = vk::PipelineRenderingCreateInfo::default()
		.depth_attachment_format(vk::Format::D32_SFLOAT);

	unsafe {
		let vk_pipeline_layout = core.vk_device.create_pipeline_layout(&pipeline_layout_info, None)?;

		let graphic_pipeline_create_infos = [
			vk::GraphicsPipelineCreateInfo::default()
				.stages(&shader_stages)
				.layout(vk_pipeline_layout)
				.vertex_input_state(&vertex_input_state)
				.input_assembly_state(&ia_state)
				.viewport_state(&viewport_state)
				.rasterization_state(&raster_state)
				.depth_stencil_state(&depth_stencil_state)
				.multisample_state(&ms_state)
				.dynamic_state(&dynamic_state)
				.push_next(&mut rendering_create_info)
		];

		let pipelines = core.vk_device.create_graphics_pipelines(vk::PipelineCache::null(), &graphic_pipeline_create_infos, None)
			.map_err(|(_, err)| err)?;

		Ok((pipelines[0], vk_pipeline_layout))
	}
}


