use ash::extensions::khr;
use ash::{vk, Entry};
use once_cell::sync::Lazy;
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::ffi::CStr;
use std::mem;
use std::ptr::null;
use std::str;
use winit::{event::Event, event_loop::EventLoop, window::Window, window::WindowBuilder};

static VK_ENTRY: Lazy<Entry> = Lazy::new(|| Entry::linked());

fn create_instance(enable_validation: bool, window: &Window) -> ash::Instance {
    let app_info = vk::ApplicationInfo {
        api_version: vk::API_VERSION_1_3,
        p_application_name: "Hello, World!".as_ptr() as *const i8,
        ..Default::default()
    };

    let layer_names = unsafe {
        if enable_validation {
            vec![CStr::from_bytes_with_nul_unchecked(b"VK_LAYER_KHRONOS_validation\0").as_ptr()]
        } else {
            vec![]
        }
    };

    let extensions = ash_window::enumerate_required_extensions(window.raw_display_handle())
        .unwrap()
        .to_vec();

    let create_info = vk::InstanceCreateInfo {
        p_application_info: &app_info,
        enabled_layer_count: layer_names.len() as u32,
        pp_enabled_layer_names: layer_names.as_ptr(),
        enabled_extension_count: extensions.len() as u32,
        pp_enabled_extension_names: extensions.as_ptr(),
        ..Default::default()
    };
    unsafe { VK_ENTRY.create_instance(&create_info, None) }.unwrap()
}

fn get_physical_device(instance: &ash::Instance, idx: usize) -> vk::PhysicalDevice {
    unsafe { instance.enumerate_physical_devices() }.unwrap()[idx]
}

fn get_device_queue_family(instance: &ash::Instance, device: vk::PhysicalDevice) -> u32 {
    let queue_families = unsafe { instance.get_physical_device_queue_family_properties(device) };
    let mut idx = 0;
    for queue_family in queue_families {
        if queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
            return idx;
        }
        idx += 1;
    }
    panic!("Cannot find device queue family!")
}

fn get_device(instance: &ash::Instance, device: vk::PhysicalDevice) -> ash::Device {
    let extensions = vec![khr::Swapchain::name().as_ptr()];
    let prio = [1.0];
    let queue_info = vk::DeviceQueueCreateInfo {
        p_queue_priorities: prio.as_ptr(),
        queue_count: 1,
        queue_family_index: get_device_queue_family(instance, device),
        ..Default::default()
    };
    let device_create_info = vk::DeviceCreateInfo {
        p_queue_create_infos: &queue_info,
        queue_create_info_count: 1,
        enabled_extension_count: extensions.len() as u32,
        pp_enabled_extension_names: extensions.as_ptr(),
        ..Default::default()
    };
    unsafe { instance.create_device(device, &device_create_info, None) }.unwrap()
}

fn get_command_pool(device: &ash::Device, graphics_q_idx: u32) -> vk::CommandPool {
    let command_pool_info = vk::CommandPoolCreateInfo::builder()
        .queue_family_index(graphics_q_idx)
        .flags(vk::CommandPoolCreateFlags::empty());
    unsafe {
        device
            .create_command_pool(&command_pool_info, None)
            .unwrap()
    }
}

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(1920, 1080))
        .build(&event_loop)
        .unwrap();

    let instance = create_instance(true, &window);

    let surface = khr::Surface::new(&VK_ENTRY, &instance);
    let surface_khr = unsafe {
        ash_window::create_surface(
            &VK_ENTRY,
            &instance,
            window.raw_display_handle(),
            window.raw_window_handle(),
            None,
        )
    }
    .unwrap();

    for dev in unsafe { instance.enumerate_physical_devices() }.unwrap() {
        let props: vk::PhysicalDeviceProperties =
            unsafe { instance.get_physical_device_properties(dev) };
        let name = unsafe { str::from_utf8(mem::transmute(props.device_name.as_slice())) }.unwrap();
        println!("{name}");
    }

    let phys_device = get_physical_device(&instance, 0);
    let device = get_device(&instance, phys_device);

    let graphics_q_idx = get_device_queue_family(&instance, phys_device);

    let pp = unsafe {
        surface
            .get_physical_device_surface_support(phys_device, graphics_q_idx, surface_khr)
            .unwrap()
    };

    println!("Valid device surface support: {pp}");

    let graphics_queue = unsafe { device.get_device_queue(graphics_q_idx, 0) };

    let command_pool = get_command_pool(&device, graphics_q_idx);

    let command_buffer = {
        let allocate_info = vk::CommandBufferAllocateInfo::builder()
            .command_pool(command_pool)
            .level(vk::CommandBufferLevel::PRIMARY)
            .command_buffer_count(1);
        unsafe { device.allocate_command_buffers(&allocate_info).unwrap()[0] }
    };

    let mut imgui = imgui::Context::create();
    imgui.io_mut().config_flags |=
        imgui::ConfigFlags::DOCKING_ENABLE | imgui::ConfigFlags::VIEWPORTS_ENABLE;
    imgui.set_ini_filename(None);

    let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
    platform.hidpi_factor();

    platform.attach_window(
        imgui.io_mut(),
        &window,
        imgui_winit_support::HiDpiMode::Rounded,
    );

    imgui
        .fonts()
        .add_font(&[imgui::FontSource::DefaultFontData { config: None }]);
    for font in imgui.fonts().fonts() {
        println!("{font:?}");
    }

    let format = unsafe { surface.get_physical_device_surface_formats(phys_device, surface_khr) }
        .unwrap()[0];

    let capabilities = unsafe {
        surface
            .get_physical_device_surface_capabilities(phys_device, surface_khr)
            .unwrap()
    };

    let extent = {
        vk::Extent2D {
            width: capabilities.max_image_extent.width,
            height: capabilities.max_image_extent.height,
        }
    };

    let swapchain_loader = khr::Swapchain::new(&instance, &device);
    let swapchain_khr = {
        let swapchain_crete_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(surface_khr)
            .min_image_count(capabilities.min_image_count)
            .image_format(format.format)
            .image_color_space(format.color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .pre_transform(capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(vk::PresentModeKHR::IMMEDIATE)
            .clipped(true);

        unsafe { swapchain_loader.create_swapchain(&swapchain_crete_info, None) }
    }
    .unwrap();

    let images = unsafe { swapchain_loader.get_swapchain_images(swapchain_khr) }.unwrap();
    let views = images
        .iter()
        .map(|image| {
            let image_create_info = vk::ImageViewCreateInfo::builder()
                .image(*image)
                .view_type(vk::ImageViewType::TYPE_2D)
                .format(format.format)
                .subresource_range(vk::ImageSubresourceRange {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                });

            unsafe { device.create_image_view(&image_create_info, None) }.unwrap()
        })
        .collect::<Vec<_>>();

    let render_pass = {
        let attachment_descs = [vk::AttachmentDescription::builder()
            .format(format.format)
            .samples(vk::SampleCountFlags::TYPE_1)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .initial_layout(vk::ImageLayout::UNDEFINED)
            .final_layout(vk::ImageLayout::PRESENT_SRC_KHR)
            .build()];

        let color_attachment_refs = [vk::AttachmentReference::builder()
            .attachment(0)
            .layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
            .build()];

        let subpass_descs = [vk::SubpassDescription::builder()
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .color_attachments(&color_attachment_refs)
            .build()];

        let subpass_deps = [vk::SubpassDependency::builder()
            .src_subpass(vk::SUBPASS_EXTERNAL)
            .dst_subpass(0)
            .src_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .src_access_mask(vk::AccessFlags::empty())
            .dst_stage_mask(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT)
            .dst_access_mask(
                vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            )
            .build()];

        let render_pass_info = vk::RenderPassCreateInfo::builder()
            .attachments(&attachment_descs)
            .subpasses(&subpass_descs)
            .dependencies(&subpass_deps);
        unsafe { device.create_render_pass(&render_pass_info, None) }.unwrap()
    };

    let framebuffers = {
        views
            .iter()
            .map(|view| [*view])
            .map(|view| {
                let framebuffer_info = vk::FramebufferCreateInfo::builder()
                    .render_pass(render_pass)
                    .attachments(&view)
                    .width(extent.width)
                    .height(extent.height)
                    .layers(1);
                unsafe { device.create_framebuffer(&framebuffer_info, None) }.unwrap()
            })
            .collect::<Vec<_>>()
    };

    let fence = {
        let fence_info = vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);
        unsafe { device.create_fence(&fence_info, None).unwrap() }
    };

    let image_available_semaphore = {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        unsafe { device.create_semaphore(&semaphore_info, None).unwrap() }
    };

    let render_finished_semaphore = {
        let semaphore_info = vk::SemaphoreCreateInfo::builder();
        unsafe { device.create_semaphore(&semaphore_info, None).unwrap() }
    };

    let mut renderer = {
        imgui_rs_vulkan_renderer::Renderer::with_default_allocator(
            &instance,
            phys_device,
            device.clone(),
            graphics_queue,
            command_pool,
            render_pass,
            &mut imgui,
            Some(imgui_rs_vulkan_renderer::Options {
                in_flight_frames: 1,
                ..Default::default()
            }),
        )
    }
    .unwrap();

    let mut last_frame = std::time::Instant::now();
    event_loop.run(move |event, _, control_flow| {
        control_flow.set_poll();

        platform.handle_event(imgui.io_mut(), &window, &event);

        let imgui = &mut imgui;

        match event {
            Event::NewEvents(_) => {
                let now = std::time::Instant::now();
                imgui.io_mut().update_delta_time(now - last_frame);
                last_frame = now;
            }
            Event::MainEventsCleared => {
                platform.prepare_frame(imgui.io_mut(), &window).unwrap();
                {
                    let ui = imgui.frame();

                    {
                        ui.dockspace_over_main_viewport();
                        ui.window("Hello World")
                            .size([300.0, 100.0], imgui::Condition::FirstUseEver)
                            .build(|| {
                                ui.text_wrapped(format!("Window size {:?}", ui.window_size()));
                                // imgui::Image::new(null(), ui.window_size());
                            });
                        // Im gui part
                        let mut opened = false;
                        ui.show_demo_window(&mut opened);
                    }

                    platform.prepare_render(&ui, &window);
                }

                let draw_data = imgui.render();
                unsafe {
                    device
                        .wait_for_fences(&[fence], true, std::u64::MAX)
                        .unwrap();
                }

                let next_image = unsafe {
                    swapchain_loader
                        .acquire_next_image(
                            swapchain_khr,
                            std::u64::MAX,
                            image_available_semaphore,
                            vk::Fence::null(),
                        )
                        .unwrap()
                        .0
                };

                unsafe { device.reset_fences(&[fence]).unwrap() };

                let wait_stages = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
                let wait_semaphores = [image_available_semaphore];
                let signal_semaphores = [render_finished_semaphore];

                {
                    unsafe {
                        device
                            .reset_command_pool(command_pool, vk::CommandPoolResetFlags::empty())
                            .unwrap()
                    };

                    let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
                        .flags(vk::CommandBufferUsageFlags::SIMULTANEOUS_USE);
                    unsafe {
                        device
                            .begin_command_buffer(command_buffer, &command_buffer_begin_info)
                            .unwrap()
                    };

                    let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                        .render_pass(render_pass)
                        .framebuffer(framebuffers[next_image as usize])
                        .render_area(vk::Rect2D {
                            offset: vk::Offset2D { x: 0, y: 0 },
                            extent,
                        })
                        .clear_values(&[vk::ClearValue {
                            color: vk::ClearColorValue {
                                float32: [0.0, 0.0, 1.0, 1.0],
                            },
                        }]);

                    unsafe {
                        device.cmd_begin_render_pass(
                            command_buffer,
                            &render_pass_begin_info,
                            vk::SubpassContents::INLINE,
                        )
                    };

                    renderer.cmd_draw(command_buffer, draw_data).unwrap();

                    unsafe { device.cmd_end_render_pass(command_buffer) };

                    unsafe { device.end_command_buffer(command_buffer).unwrap() };
                }
                // do_command_buffers();

                let command_buffers = [command_buffer];
                let submit_info = [vk::SubmitInfo::builder()
                    .wait_semaphores(&wait_semaphores)
                    .wait_dst_stage_mask(&wait_stages)
                    .command_buffers(&command_buffers)
                    .signal_semaphores(&signal_semaphores)
                    .build()];

                unsafe {
                    device
                        .queue_submit(graphics_queue, &submit_info, fence)
                        .unwrap()
                };

                imgui.update_platform_windows();
                imgui.render_platform_windows_default();

                let swapchains = [swapchain_khr];
                let image_indicies = [next_image];
                let present_info = vk::PresentInfoKHR::builder()
                    .wait_semaphores(&signal_semaphores)
                    .swapchains(&swapchains)
                    .image_indices(&image_indicies);

                let present = unsafe {
                    swapchain_loader
                        .queue_present(graphics_queue, &present_info)
                        .unwrap()
                };

                assert!(!present);
            }
            _ => (),
        }
    });
}
