use std:: {iter, mem};
use wgpu::util::DeviceExt;
use cgmath::*;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};
use bytemuck:: {Pod, Zeroable, cast_slice};

mod vertex_data;
mod transforms;
mod texture;

const IS_PERSPECTIVE:bool = true;
const ANIMATION_SPEED:f32 = 0.002;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Light {
    // position: [f32; 4],
    color: [f32; 4],
    specular_color: [f32; 4],
    ambient: f32,
    diffuse: f32,
    specular_intensity: f32,
    specular_shininess: f32,
}

pub fn light(color: [f32; 3], specular_color: [f32; 3], ambient: f32, diffuse: f32, specular_intensity: f32, specular_shininess: f32) -> Light {
    Light {
        // position: [position[0], position[1], position[2], 1.0],
        color: [color[0], color[1], color[2], 1.0],
        specular_color: [specular_color[0], specular_color[1], specular_color[2], 1.0],
        ambient,
        diffuse,
        specular_intensity,
        specular_shininess,
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 4],
    color: [f32; 4],
}

fn vertex(p:[i8;3], c:[i8; 3]) -> Vertex {
    Vertex {
        position: [p[0] as f32, p[1] as f32, p[2] as f32, 1.0],
        color: [c[0] as f32, c[1] as f32, c[2] as f32, 1.0],
    }
}

fn create_vertices() -> Vec<Vertex> {
    let pos = vertex_data::cube_positions();
    let col = vertex_data::cube_colors();
    let mut data:Vec<Vertex> = Vec::with_capacity(pos.len());
    for i in 0..pos.len() {
        data.push(vertex(pos[i], col[i]));
    }
    data.to_vec()
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![0=>Float32x4, 1=>Float32x4];
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}

struct Camera {
    pub position: Point3<f32>,
    pub direction: Point3<f32>,
    pub up: Vector3<f32>,
}

pub(crate) struct State {
    init: transforms::InitWgpu,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    vertex_uniform_buffer: wgpu::Buffer,
    uniform_bind_group:wgpu::BindGroup,
    model_mat: Matrix4<f32>,
    view_mat: Matrix4<f32>,
    project_mat: Matrix4<f32>,
    direct: String,
    camera: Camera,
    num_vertices: u32,
}

impl State {
    async fn new(window: &Window) -> Self {        
        let init =  transforms::InitWgpu::init_wgpu(window).await;

        let shader = init.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("lightning.wgsl").into()),
        });

        let diffuse_bytes = include_bytes!("happy-tree.png");
        let diffuse_texture = 
            texture::Texture::from_bytes(&init.device, &init.queue, diffuse_bytes, "happy-tree.png").unwrap();

        let texture_bind_group_layout =
            init.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });

        let diffuse_bind_group = init.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                },
            ],
            label: Some("diffuse_bind_group"),
        });

        // uniform data
        let position = (3.0, 1.5, 3.0).into();
        let direction = (0.0,0.0,0.0).into();
        let up = cgmath::Vector3::unit_y();
        let camera = Camera {
            position,
            direction,
            up
        };
        let light = light(
            // camera.position.into(),
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            0.1,
            0.6,
            0.3,
            32.0,
        );

        
        let model_mat = transforms::create_transforms([0.0,0.0,0.0], [0.0,0.0,0.0], [1.0,1.0,1.0]);
        let (view_mat, project_mat, _) = 
            transforms::create_view_projection(camera.position, camera.direction, camera.up, 
            init.config.width as f32 / init.config.height as f32, IS_PERSPECTIVE);
        // let mvp_mat = view_project_mat * model_mat;
        
        // let mvp_ref:&[f32; 16] = mvp_mat.as_ref();
        // let uniform_buffer = init.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //     label: Some("Uniform Buffer"),
        //     contents: bytemuck::cast_slice(mvp_ref),
        //     usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        // });
        let vertex_uniform_buffer = init.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: 192,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fragment_uniform_buffer = init.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fragment Uniform Buffer"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_uniform_buffer = init.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Uniform Buffer"),
            size: 48,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let eye_position: [f32; 3] = [camera.position.x, camera.position.y, camera.position.z];
        // light position
        init.queue.write_buffer(&fragment_uniform_buffer, 0, bytemuck::cast_slice(eye_position.as_ref()));
        // eye position
        init.queue.write_buffer(&fragment_uniform_buffer, 16, bytemuck::cast_slice(eye_position.as_ref()));
        init.queue.write_buffer(&light_uniform_buffer, 0, bytemuck::cast_slice(&[light]));

        let uniform_bind_group_layout = init.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor{
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Uniform Bind Group Layout"),
        });

        let uniform_bind_group = init.device.create_bind_group(&wgpu::BindGroupDescriptor{
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: vertex_uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: fragment_uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: light_uniform_buffer.as_entire_binding(),
            }],
            label: Some("Uniform Bind Group"),
        });

        let pipeline_layout = init.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = init.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: init.config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState{
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                //cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            //depth_stencil: None,
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });
        let vertex_data = create_vertices();
        let vertex_buffer = init.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: cast_slice(&vertex_data),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        Self {
            init,
            pipeline,
            vertex_buffer,
            vertex_uniform_buffer,
            uniform_bind_group,
            model_mat,
            view_mat,
            project_mat,
            direct: "".into(),
            camera,
            num_vertices: vertex_data.len() as u32,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.init.size = new_size;
            self.init.config.width = new_size.width;
            self.init.config.height = new_size.height;
            self.init.surface.configure(&self.init.device, &self.init.config);

            self.project_mat = transforms::create_projection(new_size.width as f32 / new_size.height as f32, IS_PERSPECTIVE);
            // let mvp_mat = self.project_mat * self.view_mat * self.model_mat;        
            // let mvp_ref:&[f32; 16] = mvp_mat.as_ref();
            // self.init.queue.write_buffer(&self.vertex_uniform_buffer, 0, bytemuck::cast_slice(mvp_ref));
        }
    }

    #[allow(unused_variables)]
    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state,
                        virtual_keycode: Some(keycode),
                        ..
                    },
                ..
            } => {
                let is_pressed = *state == ElementState::Pressed;
                match keycode {
                    VirtualKeyCode::Space => {
                        self.direct = "Forward".into();
                        true
                    }
                    VirtualKeyCode::LShift => {
                        self.direct = "Backward".into();
                        true
                    }
                    VirtualKeyCode::W | VirtualKeyCode::Up => {
                        self.direct = "Up".into();
                        true
                    }
                    VirtualKeyCode::A | VirtualKeyCode::Left => {
                        self.direct = "Left".into();
                        true
                    }
                    VirtualKeyCode::S | VirtualKeyCode::Down => {
                        self.direct = "Down".into();
                        true
                    }
                    VirtualKeyCode::D | VirtualKeyCode::Right => {
                        self.direct = "Right".into();
                        true
                    }
                    _ => false,
                }
            },
            WindowEvent::MouseWheel { 
                delta: MouseScrollDelta::LineDelta(_, y),
                ..
            } => {
                if *y < 0.0 {
                    self.direct = "Backward".into();
                } else {
                    self.direct = "Forward".into();
                }
                true
            },
            _ => false,
        }
    }


    fn update(&mut self, dt: std::time::Duration) {
        // update uniform buffer
        let _dt = ANIMATION_SPEED * dt.as_secs_f32(); 
        // let mut translation = [0.0, 0.0, 0.0];
        // let mut rotation = [0.0, 0.0, 0.0];
        let forward = self.camera.direction - self.camera.position;
        self.view_mat = match &self.direct as &str {
            "Forward" => {
                self.direct = "".into();
                self.camera.position += forward.normalize();
                transforms::create_view(self.camera.position, self.camera.direction, self.camera.up)
            },
            "Backward" => {
                self.direct = "".into();
                self.camera.position -= forward.normalize();
                transforms::create_view(self.camera.position, self.camera.direction, self.camera.up)
            },
            "Up" => {
                self.direct = "".into();
                self.camera.position = self.camera.direction + (
                    -forward + self.camera.up
                ).normalize() * forward.magnitude();
                transforms::create_view(self.camera.position, self.camera.direction, self.camera.up)
            },
            "Down" => {
                self.direct = "".into();
                self.camera.position = self.camera.direction + (
                    -forward - self.camera.up
                ).normalize() * forward.magnitude();
                transforms::create_view(self.camera.position, self.camera.direction, self.camera.up)
            },
            "Left" => {
                self.direct = "".into();
                self.camera.position = self.camera.direction + (
                    -forward - forward.normalize().cross(self.camera.up)
                ).normalize() * forward.magnitude();
                transforms::create_view(self.camera.position, self.camera.direction, self.camera.up)
            },
            "Right" => {
                self.direct = "".into();
                self.camera.position = self.camera.direction + (
                    -forward + forward.normalize().cross(self.camera.up)
                ).normalize() * forward.magnitude();
                transforms::create_view(self.camera.position, self.camera.direction, self.camera.up)
            },
            _ => {
                self.view_mat
            }
        };
        // self.model_mat = self.model_mat * transforms::create_transforms(translation, rotation, [1.0, 1.0, 1.0]);
        // self.model_mat = model_mat;
        // let mvp_mat = self.project_mat * self.view_mat * self.model_mat;        
        // let mvp_ref:&[f32; 16] = mvp_mat.as_ref();
        let mref: &[f32; 16] = self.model_mat.as_ref();
        let pv_mat = self.project_mat * self.view_mat;
        let pvref: &[f32; 16] = pv_mat.as_ref();
        let normal_mat = self.model_mat.invert().unwrap().transpose();
        let nref: &[f32; 16] = normal_mat.as_ref();
        self.init.queue.write_buffer(&self.vertex_uniform_buffer, 0, bytemuck::cast_slice(mref));
        self.init.queue.write_buffer(&self.vertex_uniform_buffer, 64, bytemuck::cast_slice(pvref));
        self.init.queue.write_buffer(&self.vertex_uniform_buffer, 128, bytemuck::cast_slice(nref));
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        //let output = self.init.surface.get_current_frame()?.output;
        let output = self.init.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());  
        let depth_texture = self.init.device.create_texture(&wgpu::TextureDescriptor {
            view_formats: &[],
            size: wgpu::Extent3d {
                width: self.init.config.width,
                height: self.init.config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format:wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: None,
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        
        let mut encoder = self
            .init.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.2,
                            g: 0.247,
                            b: 0.314,
                            a: 1.0,
                        }),
                        store: true,
                    },
                })],
                //depth_stencil_attachment: None,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: false,
                    }),
                    stencil_ops: None,
                }),
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));           
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.draw(0..self.num_vertices, 0..1);
        }

        self.init.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}