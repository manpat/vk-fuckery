use crate::gfx;
use ash::vk;

use anyhow::Context;

// https://gpuopen-librariesandsdks.github.io/VulkanMemoryAllocator/html/usage_patterns.html
// https://www.gdcvault.com/play/1025458/Advanced-Graphics-Techniques-Tutorial-New

pub struct DeviceAllocator {
	staging_memory_type_index: u32,
	staging_memory_heap_budget: u64,

	device_local_memory_type_index: u32,
	device_local_memory_heap_budget: u64,

	buffer_alignment: u64,
	image_alignment: u64,
	rt_alignment: u64,
}

impl DeviceAllocator {
	pub fn new(core: &gfx::Core) -> anyhow::Result<DeviceAllocator> {
		// TODO(pat.m): store
		let mut memory_budgets = vk::PhysicalDeviceMemoryBudgetPropertiesEXT::default();
		let mut memory_props = vk::PhysicalDeviceMemoryProperties2::default()
			.push_next(&mut memory_budgets);

		// Just everything we could possibly want from a buffer
		let buffer_usage = vk::BufferUsageFlags::TRANSFER_SRC
			| vk::BufferUsageFlags::TRANSFER_DST
			| vk::BufferUsageFlags::STORAGE_BUFFER
			| vk::BufferUsageFlags::INDEX_BUFFER
			| vk::BufferUsageFlags::INDIRECT_BUFFER
			| vk::BufferUsageFlags::SHADER_DEVICE_ADDRESS;

		let buffer_create_info = vk::BufferCreateInfo::default()
			.size(1<<10)
			.usage(buffer_usage)
			.sharing_mode(vk::SharingMode::EXCLUSIVE);

		let buffer_requirements_query = vk::DeviceBufferMemoryRequirements::default()
			.create_info(&buffer_create_info);

		// Just everything we could possibly want from an image
		let image_usage = vk::ImageUsageFlags::TRANSFER_SRC
			| vk::ImageUsageFlags::TRANSFER_DST
			| vk::ImageUsageFlags::SAMPLED
			| vk::ImageUsageFlags::STORAGE;

		let rt_usage = vk::ImageUsageFlags::COLOR_ATTACHMENT
			| vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;

		let image_create_info = vk::ImageCreateInfo::default()
			.image_type(vk::ImageType::TYPE_2D)
			.format(vk::Format::R8G8B8A8_UNORM)
			.samples(vk::SampleCountFlags::TYPE_1)
			.extent(vk::Extent3D{ width: 64, height: 64, depth: 1 })
			.mip_levels(1)
			.array_layers(1)
			.usage(image_usage)
			.initial_layout(vk::ImageLayout::UNDEFINED)
			.sharing_mode(vk::SharingMode::EXCLUSIVE);

		let image_requirements_query = vk::DeviceImageMemoryRequirements::default()
			.create_info(&image_create_info);

		let rt_create_info = image_create_info.clone()
			.usage(rt_usage);

		let rt_requirements_query = vk::DeviceImageMemoryRequirements::default()
			.create_info(&rt_create_info);

		let mut buffer_requirements = vk::MemoryRequirements2::default();
		let mut image_requirements = vk::MemoryRequirements2::default();
		let mut rt_requirements = vk::MemoryRequirements2::default();

		// TODO(pat.m): max allocation count

		unsafe {
			core.vk_instance.get_physical_device_memory_properties2(core.vk_physical_device, &mut memory_props);
			core.vk_device.get_device_buffer_memory_requirements(&buffer_requirements_query, &mut buffer_requirements);
			core.vk_device.get_device_image_memory_requirements(&image_requirements_query, &mut image_requirements);
			core.vk_device.get_device_image_memory_requirements(&rt_requirements_query, &mut rt_requirements);
		};

		let buffer_requirements = buffer_requirements.memory_requirements;
		let image_requirements = image_requirements.memory_requirements;
		let rt_requirements = rt_requirements.memory_requirements;

		let memory_props = memory_props.memory_properties;

		log::info!("Available memory heaps:");
		for heap_index in 0..memory_props.memory_heap_count as usize {
			let heap = memory_props.memory_heaps[heap_index];

			let residency_str = match heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
				true => "Device",
				false => "Host",
			};

			let budget = memory_budgets.heap_budget[heap_index] >> 20;
			let usage = memory_budgets.heap_usage[heap_index] >> 20;
			let total = heap.size >> 20;

			log::info!("--- {residency_str:>6} Local: {budget:>5}MiB / {total:>5}MiB  (current usage: {usage}MiB)");

		}

		log::info!("Memory types: {:#?}", &memory_props.memory_types[0..memory_props.memory_type_count as usize]);

		log::info!("Allowed buffer memory types: 0b{:b}", buffer_requirements.memory_type_bits);
		log::info!("Buffer alignment requirement: {}", buffer_requirements.alignment);

		log::info!("Allowed image memory types: 0b{:b}", image_requirements.memory_type_bits);
		log::info!("Image alignment requirement: {}", image_requirements.alignment);

		log::info!("Allowed rendertarget memory types: 0b{:b}", rt_requirements.memory_type_bits);
		log::info!("Rendertarget alignment requirement: {}", rt_requirements.alignment);

		// Select the host local memory type with the largest associated heap for staging memory
		let (staging_memory_type_index, selected_memory_type) = memory_props.memory_types.iter().enumerate()
			.take(memory_props.memory_type_count as usize)
			.filter(|(index, memory_type)| {
				// TODO(pat.m): don't require HOST_COHERENT and instead manually flush if it is not present
				let has_desired_flags = memory_type.property_flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT);
				let allows_buffers = (1 << index) & buffer_requirements.memory_type_bits != 0;
				allows_buffers && has_desired_flags
			})
			.max_by_key(|(_, memory_type)| {
				let mut budget = memory_budgets.heap_budget[memory_type.heap_index as usize];

				// Prefer uncached memory types
				if !memory_type.property_flags.contains(vk::MemoryPropertyFlags::HOST_CACHED) {
					budget += 1000;
				}

				budget
			})
			.context("Couldn't find staging memory type")?;

		let memory_heap_index = selected_memory_type.heap_index as usize;
		let memory_heap = memory_props.memory_heaps[memory_heap_index];
		let staging_memory_heap_budget = memory_budgets.heap_budget[memory_heap_index];

		log::info!("Selected Staging Memory Heap: {memory_heap:?} (#{memory_heap_index}) - budget: {staging_memory_heap_budget}", );
		log::info!("Selected Staging Memory Type: {selected_memory_type:?} (#{staging_memory_type_index})");

		// Select the device local memory type with the largest associated heap
		let (device_local_memory_type_index, selected_memory_type) = memory_props.memory_types.iter().enumerate()
			.take(memory_props.memory_type_count as usize)
			.filter(|(index, memory_type)| {
				let has_desired_flags = memory_type.property_flags.contains(vk::MemoryPropertyFlags::DEVICE_LOCAL);
				let allows_buffers = (1 << index) & buffer_requirements.memory_type_bits != 0;
				let allows_images = (1 << index) & image_requirements.memory_type_bits != 0;
				let allows_rts = (1 << index) & rt_requirements.memory_type_bits != 0;
				allows_buffers && allows_images && allows_rts && has_desired_flags
			})
			.max_by_key(|(_, memory_type)| memory_budgets.heap_budget[memory_type.heap_index as usize])
			.context("Couldn't find device local memory type")?;

		// TODO(pat.m): weight options to prefer non-host visible!

		let memory_heap_index = selected_memory_type.heap_index as usize;
		let memory_heap = memory_props.memory_heaps[memory_heap_index];
		let device_local_memory_heap_budget = memory_budgets.heap_budget[memory_heap_index];

		log::info!("Selected Device Local Memory Heap: {memory_heap:?} (#{memory_heap_index}) - budget: {device_local_memory_heap_budget}", );
		log::info!("Selected Device Local Memory Type: {selected_memory_type:?} (#{device_local_memory_type_index})");

		Ok(DeviceAllocator {
			staging_memory_type_index: staging_memory_type_index as u32,
			device_local_memory_type_index: device_local_memory_type_index as u32,

			// TODO(pat.m): maybe storing these doesn't make sense?
			staging_memory_heap_budget,
			device_local_memory_heap_budget,

			buffer_alignment: buffer_requirements.alignment,
			image_alignment: image_requirements.alignment,
			rt_alignment: rt_requirements.alignment,
		})
	}

	fn allocate(core: &gfx::Core, size_bytes: u64, memory_type_index: u32) -> anyhow::Result<vk::DeviceMemory> {
		let mut allocate_flags = vk::MemoryAllocateFlagsInfo::default()
			.flags(vk::MemoryAllocateFlags::DEVICE_ADDRESS);

		let allocate_info = vk::MemoryAllocateInfo::default()
			.allocation_size(size_bytes)
			.memory_type_index(memory_type_index)
			.push_next(&mut allocate_flags);

		let vk_memory = unsafe {
			core.vk_device.allocate_memory(&allocate_info, None)?
		};

		Ok(vk_memory)
	}

	pub fn allocate_staging_memory(&self, core: &gfx::Core, size_bytes: u64) -> anyhow::Result<vk::DeviceMemory> {
		anyhow::ensure!(size_bytes <= self.staging_memory_heap_budget, "Staging memory heap not big enough :(");
		Self::allocate(core, size_bytes, self.staging_memory_type_index)
	}

	pub fn allocate_device_memory(&self, core: &gfx::Core, size_bytes: u64) -> anyhow::Result<vk::DeviceMemory> {
		anyhow::ensure!(size_bytes <= self.device_local_memory_heap_budget, "Device local memory heap not big enough :(");
		Self::allocate(core, size_bytes, self.device_local_memory_type_index)
	}
}