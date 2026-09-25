//! Test-only egui renderer. A GPU texture replaces the window surface entirely.
use egui_wgpu::wgpu;

pub(super) struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: egui_wgpu::Renderer,
}

impl Renderer {
    pub(super) fn new() -> Self {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: None,
            ..Default::default()
        }))
        .expect("offscreen adapter unavailable");
        println!("Offscreen adapter: {:?}", adapter.get_info());
        // Whole-page captures are taller than the default 8192 limit.
        let descriptor = wgpu::DeviceDescriptor {
            required_limits: wgpu::Limits {
                max_texture_dimension_2d: adapter.limits().max_texture_dimension_2d,
                ..Default::default()
            },
            ..Default::default()
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor, None))
            .expect("offscreen device unavailable");
        let renderer =
            egui_wgpu::Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, None, 1, false);
        Self {
            device,
            queue,
            renderer,
        }
    }

    pub(super) fn save(
        &mut self,
        ctx: &egui::Context,
        output: egui::FullOutput,
        size: egui::Vec2,
        path: &std::path::Path,
    ) {
        let image = self.capture(ctx, output, size);
        image.save(path).unwrap();
    }

    /// Renders and reads back without encoding, so callers can encode off-thread.
    pub(super) fn capture(
        &mut self,
        ctx: &egui::Context,
        output: egui::FullOutput,
        size: egui::Vec2,
    ) -> image::RgbaImage {
        let width = size.x as u32;
        let height = size.y as u32;
        let extent = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PowerShellManager offscreen UI fixture"),
            size: extent,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        for (id, delta) in &output.textures_delta.set {
            self.renderer
                .update_texture(&self.device, &self.queue, *id, delta);
        }
        let paint_jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: output.pixels_per_point,
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut commands = self.renderer.update_buffers(
            &self.device,
            &self.queue,
            &mut encoder,
            &paint_jobs,
            &screen,
        );
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("PowerShellManager headless visual pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &paint_jobs, &screen);
        }
        let stride = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("PowerShellManager offscreen readback"),
            size: (stride * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: None,
                },
            },
            extent,
        );
        commands.push(encoder.finish());
        let submission = self.queue.submit(commands);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.device
            .poll(wgpu::Maintain::WaitForSubmissionIndex(submission));
        receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap()
            .unwrap();
        let pixels: Vec<_> = {
            let mapped = buffer.slice(..).get_mapped_range();
            mapped
                .chunks(stride as usize)
                .flat_map(|row| row[..width as usize * 4].iter().copied())
                .collect()
        };
        buffer.unmap();
        for id in output.textures_delta.free {
            self.renderer.free_texture(&id);
        }
        image::RgbaImage::from_raw(width, height, pixels).expect("readback size")
    }
}
