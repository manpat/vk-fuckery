#![feature(c_str_literals)]

use winit::{
	event::{Event, WindowEvent, KeyboardInput, ElementState, VirtualKeyCode},
	event_loop::{EventLoop},
	window::{/*Window,*/ WindowBuilder},
	dpi::LogicalSize,
};

use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use ash::prelude::*;
use ash::vk;
use ash::extensions::{khr};

use std::ffi::{CStr};


fn main() -> anyhow::Result<()> {
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new()
		.with_title("Vk Fuck")
		.with_inner_size(LogicalSize::new(1024, 768))
		.build(&event_loop)?;

	let entry = unsafe { ash::Entry::load()? };
	let vk_app_info = vk::ApplicationInfo::builder()
		.application_name(c"Vk Fuck")
		.application_version(vk::make_api_version(0, 1, 0, 0))
		.engine_name(c"Vk Fuck")
		.engine_version(vk::make_api_version(0, 1, 0, 0))
		.api_version(vk::API_VERSION_1_3);

	let mut required_extensions = ash_window::enumerate_required_extensions(window.raw_display_handle())?.to_owned();
	required_extensions.push(c"VK_EXT_debug_utils".as_ptr());

	let validation_layer_name = [c"VK_LAYER_KHRONOS_validation".as_ptr()];

	let message_severity = vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
		// | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
		// | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
		| vk::DebugUtilsMessageSeverityFlagsEXT::ERROR;

	let message_type = vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
		| vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
		| vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION;

	let mut messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
		.pfn_user_callback(Some(vulkan_debug_utils_callback))
		.message_severity(message_severity)
		.message_type(message_type);

	let vk_instance_info = vk::InstanceCreateInfo::builder()
		.application_info(&vk_app_info)
		.enabled_extension_names(&required_extensions)
		.enabled_layer_names(&validation_layer_name)
		.push_next(&mut messenger_create_info);

	let vk_instance = unsafe {
		entry.create_instance(&vk_instance_info, None).unwrap()
	};

	let debug_utils = ash::extensions::ext::DebugUtils::new(&entry, &vk_instance);
	let vk_messenger = unsafe {
		debug_utils.create_debug_utils_messenger(&messenger_create_info, None)?
	};



	// Surface
	let surface_fn = khr::Surface::new(&entry, &vk_instance);
	let vk_surface = unsafe {
		ash_window::create_surface(&entry, &vk_instance, window.raw_display_handle(), window.raw_window_handle(), None)?
	};


	// Physical device
	let physical_devices = unsafe { vk_instance.enumerate_physical_devices()? };
	for physical_device in physical_devices.iter() {
		unsafe {
			let device_props = vk_instance.get_physical_device_properties(*physical_device);
			// let memory_props = vk_instance.get_physical_device_memory_properties(*physical_device);
			let queue_family_props = vk_instance.get_physical_device_queue_family_properties(*physical_device);
			let features = vk_instance.get_physical_device_features(*physical_device);

			let mut features13 = vk::PhysicalDeviceVulkan13Features::default();
			let mut features2 = vk::PhysicalDeviceFeatures2::builder()
				.push_next(&mut features13);

			vk_instance.get_physical_device_features2(*physical_device, &mut features2);

			println!("Device: {:?}", CStr::from_ptr(device_props.device_name.as_ptr()));

			// dbg!(&device_props);
			// dbg!(&memory_props);
			// dbg!(&queue_family_props);
			dbg!(&features);
			dbg!(features2.features);
			dbg!(features13);

			assert!(features13.dynamic_rendering > 0);

			println!("=== Queues");
			for (idx, family) in queue_family_props.iter().enumerate() {
				println!("--- [{idx}] {:?}", family.queue_flags);
				println!("--- --- present supported? {:?}", surface_fn.get_physical_device_surface_support(*physical_device, idx as u32, vk_surface)?);
			}
		}
	}

	let vk_physical_device = physical_devices[0];



	// Logical device and Queue
	let graphics_queue_family_index = unsafe {
		vk_instance.get_physical_device_queue_family_properties(vk_physical_device).into_iter()
			.enumerate()
			.filter(|(idx, family)| {
				let present_supported = surface_fn.get_physical_device_surface_support(vk_physical_device, *idx as u32, vk_surface) == Ok(true);
				family.queue_flags.contains(vk::QueueFlags::GRAPHICS) && present_supported
			})
			.next()
			.expect("Couldn't get appropriate queue family").0 as u32
	};

	dbg!(graphics_queue_family_index);

	let queue_create_infos = [
		vk::DeviceQueueCreateInfo::builder()
			.queue_family_index(graphics_queue_family_index)
			.queue_priorities(&[1.0])
			.build()
	];

	let vk_device = unsafe {
		let ext_names = [c"VK_KHR_swapchain".as_ptr()];

		let mut features_13 = vk::PhysicalDeviceVulkan13Features::builder()
			.dynamic_rendering(true)
			.synchronization2(true);

		let device_create_info = vk::DeviceCreateInfo::builder()
			.queue_create_infos(&queue_create_infos)
			.enabled_extension_names(&ext_names)
			.push_next(&mut features_13);

		vk_instance.create_device(vk_physical_device, &device_create_info, None)?
	};

	let vk_graphics_queue = unsafe { vk_device.get_device_queue(graphics_queue_family_index, 0) };



	// Swapchain
	let swapchain_fn = khr::Swapchain::new_from_instance(&entry, &vk_instance, vk_device.handle());
	let swapchain_capabilities = unsafe {
		surface_fn.get_physical_device_surface_capabilities(vk_physical_device, vk_surface)?
	};

	let vk_swapchain = unsafe {
		let supported_formats = surface_fn.get_physical_device_surface_formats(vk_physical_device, vk_surface)?;
		let supported_present_modes = surface_fn.get_physical_device_surface_present_modes(vk_physical_device, vk_surface)?;
		dbg!(swapchain_capabilities, supported_formats, &supported_present_modes);

		assert!(supported_present_modes.contains(&vk::PresentModeKHR::FIFO));

		let max_images = match swapchain_capabilities.max_image_count {
			0 => u32::MAX,
			n => n
		};

		let num_images = (swapchain_capabilities.min_image_count + 1).min(max_images);

		let create_info = vk::SwapchainCreateInfoKHR::builder()
			.surface(vk_surface)
			.min_image_count(num_images)
			// TODO(pat.m): get these from supported_formats
			.image_format(vk::Format::B8G8R8A8_SRGB)
			.image_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
			.image_extent(swapchain_capabilities.current_extent)
			.image_array_layers(1)
			.image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
			.image_sharing_mode(vk::SharingMode::EXCLUSIVE)
			.pre_transform(swapchain_capabilities.current_transform)
			.composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
			// TODO(pat.m): get this from supported_present_modes
			.present_mode(vk::PresentModeKHR::FIFO)
			.clipped(true);

		swapchain_fn.create_swapchain(&create_info, None)?
	};

	// Swapchain images
	let swapchain_images = unsafe { swapchain_fn.get_swapchain_images(vk_swapchain)? };

	let swapchain_image_views: Vec<_> = swapchain_images.iter()
		.map(|&image| unsafe {
			let create_info = vk::ImageViewCreateInfo::builder()
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
					vk::ImageSubresourceRange::builder()
						.aspect_mask(vk::ImageAspectFlags::COLOR)
						.base_mip_level(0)
						.base_array_layer(0)
						.level_count(1)
						.layer_count(1)
						.build()
				);

			vk_device.create_image_view(&create_info, None).unwrap()
		})
		.collect();


	// Command pool and command buffers
	let vk_cmd_pool = unsafe {
		let create_info = vk::CommandPoolCreateInfo::builder()
			.queue_family_index(graphics_queue_family_index)
			.flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER);

		vk_device.create_command_pool(&create_info, None)?
	};

	let vk_cmd_buffers = unsafe {
		let create_info = vk::CommandBufferAllocateInfo::builder()
			.command_buffer_count(swapchain_images.len() as u32)
			.command_pool(vk_cmd_pool)
			.level(vk::CommandBufferLevel::PRIMARY);

		vk_device.allocate_command_buffers(&create_info)?
	};


	// Set up frame sync
	struct Sync {
		image_available: vk::Semaphore,
		render_finished: vk::Semaphore,
		in_flight_fence: vk::Fence,
	}

	let sync_objects: Vec<_> = (0..swapchain_images.len())
		.map(|_| unsafe {
			Sync {
				image_available: vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap(),
				render_finished: vk_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None).unwrap(),
				in_flight_fence: vk_device.create_fence(&vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED), None).unwrap(),
			}
		})
		.collect();

	let mut frame_number = 0;


	let mut destroying = false;

	event_loop.run(move |event, _, control_flow| {
		match event {
			// Render a frame if our Vulkan app is not being destroyed.
			Event::MainEventsCleared if !destroying => {
				unsafe {
					let timeout_ns = 1000*1000*1000;

					let frame_sync = &sync_objects[frame_number];

					vk_device.wait_for_fences(&[frame_sync.in_flight_fence], true, timeout_ns).unwrap();
					vk_device.reset_fences(&[frame_sync.in_flight_fence]).unwrap();

					let (image_idx, _) = swapchain_fn.acquire_next_image(
						vk_swapchain,
						timeout_ns,
						frame_sync.image_available,
						vk::Fence::null()
					).unwrap();
					
					// Build the command buffer
					{
						let cmd_buffer = vk_cmd_buffers[frame_number];
						let swapchain_image = swapchain_images[image_idx as usize];
						let swapchain_image_view = swapchain_image_views[image_idx as usize];

						let begin_info = vk::CommandBufferBeginInfo::builder()
							.flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);

						vk_device.begin_command_buffer(cmd_buffer, &begin_info).unwrap();

						vk_device.cmd_pipeline_barrier2(
							cmd_buffer,
							&vk::DependencyInfo::builder()
								.image_memory_barriers(&[
									vk::ImageMemoryBarrier2::builder()
										.image(swapchain_image)
										.old_layout(vk::ImageLayout::UNDEFINED)
										.new_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)

										.src_stage_mask(vk::PipelineStageFlags2::NONE)
										.src_access_mask(vk::AccessFlags2::NONE)
										
										.dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
										.dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
										.subresource_range(
											vk::ImageSubresourceRange::builder()
												.aspect_mask(vk::ImageAspectFlags::COLOR)
												.base_mip_level(0)
												.base_array_layer(0)
												.level_count(1)
												.layer_count(1)
												.build()
										)
										.build()
								]
							)
						);

						let color_attachments = [
							vk::RenderingAttachmentInfo::builder()
								.image_view(swapchain_image_view)
								.image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
								.load_op(vk::AttachmentLoadOp::CLEAR)
								.store_op(vk::AttachmentStoreOp::STORE)
								.clear_value(vk::ClearValue {
									color: vk::ClearColorValue {
										float32: match frame_number {
											0 => [1.0, 0.5, 1.0, 1.0],
											1 => [0.5, 1.0, 1.0, 1.0],
											2 => [1.0, 1.0, 0.5, 1.0],
											_ => [0.0; 4]
										}
									},
								})
								.build()
						];

						let render_info = vk::RenderingInfo::builder()
							.layer_count(1)
							.render_area(vk::Rect2D {
								offset: vk::Offset2D { x: 0, y: 0 },
								extent: swapchain_capabilities.current_extent,
							})
							.color_attachments(&color_attachments);

						vk_device.cmd_begin_rendering(cmd_buffer, &render_info);
						vk_device.cmd_end_rendering(cmd_buffer);

						vk_device.cmd_pipeline_barrier2(
							cmd_buffer,
							&vk::DependencyInfo::builder()
								.image_memory_barriers(&[
									vk::ImageMemoryBarrier2::builder()
										.image(swapchain_image)
										.old_layout(vk::ImageLayout::ATTACHMENT_OPTIMAL)
										.new_layout(vk::ImageLayout::PRESENT_SRC_KHR)

										.src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
										.src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
										
										.dst_stage_mask(vk::PipelineStageFlags2::NONE)
										.dst_access_mask(vk::AccessFlags2::NONE)
										.subresource_range(
											vk::ImageSubresourceRange::builder()
												.aspect_mask(vk::ImageAspectFlags::COLOR)
												.base_mip_level(0)
												.base_array_layer(0)
												.level_count(1)
												.layer_count(1)
												.build()
										)
										.build()
								]
							)
						);

						vk_device.end_command_buffer(cmd_buffer).unwrap();
					}

					vk_device.queue_submit(vk_graphics_queue,
						&[vk::SubmitInfo::builder()
							.command_buffers(&[vk_cmd_buffers[image_idx as usize]])
							.wait_semaphores(&[frame_sync.image_available])
							.wait_dst_stage_mask(&[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT])
							.signal_semaphores(&[frame_sync.render_finished])
							.build()],
						frame_sync.in_flight_fence
					).unwrap();

					swapchain_fn.queue_present(vk_graphics_queue,
						&vk::PresentInfoKHR::builder()
							.swapchains(&[vk_swapchain])
							.image_indices(&[image_idx])
							.wait_semaphores(&[frame_sync.render_finished])
					).unwrap();

					frame_number = (frame_number + 1) % sync_objects.len();
				}
			}

			Event::WindowEvent { event: WindowEvent::CloseRequested 
				| WindowEvent::KeyboardInput{ input: KeyboardInput {
					state: ElementState::Pressed,
					virtual_keycode: Some(VirtualKeyCode::Escape),
					..
				}, .. }, .. } =>
			{
				destroying = true;
				control_flow.set_exit();
			}

			Event::LoopDestroyed => unsafe {
				vk_device.device_wait_idle().unwrap();

				vk_device.destroy_command_pool(vk_cmd_pool, None);

				for &Sync{image_available, render_finished, in_flight_fence} in sync_objects.iter() {
					vk_device.destroy_semaphore(image_available, None);
					vk_device.destroy_semaphore(render_finished, None);
					vk_device.destroy_fence(in_flight_fence, None);
				}

				for view in swapchain_image_views.iter() {
					vk_device.destroy_image_view(*view, None);
				}

				swapchain_fn.destroy_swapchain(vk_swapchain, None);
				surface_fn.destroy_surface(vk_surface, None);
				vk_device.destroy_device(None);
				debug_utils.destroy_debug_utils_messenger(vk_messenger, None);
				vk_instance.destroy_instance(None);
			}
			_ => {}
		}

	})
}


unsafe extern "system" fn vulkan_debug_utils_callback(
	message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
	message_type: vk::DebugUtilsMessageTypeFlagsEXT,
	p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
	_p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
	let severity = match message_severity {
		vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => "[Verbose]",
		vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => "[Warning]",
		vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => "[Error]",
		vk::DebugUtilsMessageSeverityFlagsEXT::INFO => "[Info]",
		_ => "[Unknown]",
	};
	let types = match message_type {
		vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
		vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
		vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
		_ => "[Unknown]",
	};
	let message = CStr::from_ptr((*p_callback_data).p_message);
	println!("[Debug]{}{}{:?}", severity, types, message);

	vk::FALSE
}