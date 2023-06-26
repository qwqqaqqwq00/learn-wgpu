use std:: {iter};
use instancing::Instance;
use model::Vertex;
use wgpu::util::DeviceExt;
use cgmath::*;
use winit::{
    event::*,
    window::Window,
};
// use fs_extra;
use bytemuck:: {Pod, Zeroable};

// use crate::transforms;
use crate::{transforms, instancing, model::{self, DrawModel}, resources};


const IS_PERSPECTIVE:bool = true;
const ANIMATION_SPEED:f32 = 0.002;

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Light {
    specular_color: [f32; 4],
    ambient: f32,
    diffuse: f32,
    specular_intensity: f32,
    specular_shininess: f32,
}

pub fn light(specular_color: [f32; 3], ambient: f32, diffuse: f32, specular_intensity: f32, specular_shininess: f32) -> Light {
    Light {
        specular_color: [specular_color[0], specular_color[1], specular_color[2], 1.0],
        ambient,
        diffuse,
        specular_intensity,
        specular_shininess,
    }
}

pub struct Camera {
    pub position: Point3<f32>,
    pub direction: Point3<f32>,
    pub up: Vector3<f32>,
}

pub(crate) struct State {
    pub init: transforms::InitWgpu,
    pipeline: wgpu::RenderPipeline,
    light_render_pipeline: wgpu::RenderPipeline,
    instances: Vec<instancing::Instance>,
    obj_model: model::Model,
    vertex_uniform_buffer: wgpu::Buffer,
    fragment_uniform_buffer: wgpu::Buffer,
    instance_buffer: wgpu::Buffer,
    uniform_bind_group:wgpu::BindGroup,
    model_mat: Matrix4<f32>,
    view_mat: Matrix4<f32>,
    project_mat: Matrix4<f32>,
    direct: String,
    camera: Camera,
    light_instance: Point3<f32>,
}

fn create_render_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    color_format: wgpu::TextureFormat,
    depth_format: Option<wgpu::TextureFormat>,
    vertex_layouts: &[wgpu::VertexBufferLayout],
    shader: wgpu::ShaderModuleDescriptor,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(shader);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Render Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: vertex_layouts,
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: color_format,
                blend: Some(wgpu::BlendState {
                    alpha: wgpu::BlendComponent::REPLACE,
                    color: wgpu::BlendComponent::REPLACE,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
            polygon_mode: wgpu::PolygonMode::Fill,
            // Requires Features::DEPTH_CLIP_CONTROL
            unclipped_depth: false,
            // Requires Features::CONSERVATIVE_RASTERIZATION
            conservative: false,
        },
        depth_stencil: depth_format.map(|format| wgpu::DepthStencilState {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Less,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        // If the pipeline will be used with a multiview render pass, this
        // indicates how many array layers the attachments will have.
        multiview: None,
    })
}

impl State {
    pub async fn new(window: &Window) -> Self {        
        let init =  transforms::InitWgpu::init_wgpu(window).await;

        

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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            });
                
        let obj_model =
            resources::load_model("cube.obj", &init.device, &init.queue, &texture_bind_group_layout)
                .await.unwrap();

        // uniform data
        let position = (0.0, 5.0, -10.0).into();
        let direction = (0.0,0.0,0.0).into();
        let up = cgmath::Vector3::unit_y();
        let camera = Camera {
            position,
            direction,
            up
        };
            // println!("{},{}",vertex_data.len(), indices.len());
        let light = light(
            // camera.position.into(),
            // [0.0, 1.0, 0.0],
            [1.0, 1.0, 1.0],
            0.3,
            0.3,
            0.3,
            32.0,
        );

        
        let model_mat = transforms::create_transforms([0.0,0.0,0.0], [0.0,0.0,0.0], [1.0,1.0,1.0]);
        let (view_mat, project_mat, _) = 
            transforms::create_view_projection(camera.position, camera.direction, camera.up, 
            init.config.width as f32 / init.config.height as f32, IS_PERSPECTIVE);

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
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let light_position: [f32; 3] = [2.0, 2.0, 2.0];
        let eye_position: [f32; 3] = [camera.position.x, camera.position.y, camera.position.z];
        // light position
        init.queue.write_buffer(&fragment_uniform_buffer, 0, bytemuck::cast_slice(light_position.as_ref()));
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
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
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

        let pipeline = {
            let pipeline_layout = init.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
                push_constant_ranges: &[],
            });
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("lightning.wgsl").into()),
            };
            create_render_pipeline(&init.device, &pipeline_layout, init.config.format, Some(wgpu::TextureFormat::Depth24Plus), &[model::ModelVertex::desc(), instancing::InstanceRaw::desc()], shader)
        };
        
        let light_render_pipeline = {
            let layout = init.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("light pipeline"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("light shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("light.wgsl").into()),
            };
            create_render_pipeline(&init.device, &layout, init.config.format, Some(wgpu::TextureFormat::Depth24Plus), &[model::ModelVertex::desc()], shader)
        };

        let instances = instancing::craete_instances();
        let instance_data = instances.iter().map(instancing::Instance::to_raw).collect::<Vec<_>>();
        let instance_buffer = init.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Instance Buffer"),
                contents: bytemuck::cast_slice(&instance_data),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }
        );
        Self {
            init,
            pipeline,
            light_render_pipeline,
            instances,
            vertex_uniform_buffer,
            fragment_uniform_buffer,
            instance_buffer,
            uniform_bind_group,
            obj_model,
            model_mat,
            view_mat,
            project_mat,
            direct: "".into(),
            camera,
            light_instance: eye_position.into(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.init.size = new_size;
            self.init.config.width = new_size.width;
            self.init.config.height = new_size.height;
            self.init.surface.configure(&self.init.device, &self.init.config);

            self.project_mat = transforms::create_projection(new_size.width as f32 / new_size.height as f32, IS_PERSPECTIVE);
        }
    }

    #[allow(unused_variables)]
    pub fn input(&mut self, event: &WindowEvent) -> bool {
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

    pub fn update(&mut self, dt: std::time::Duration) {
        // update uniform buffer
        let dt = ANIMATION_SPEED * dt.as_secs_f32(); 
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
                // let r = cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_y(), cgmath::Deg(-dt));
                // self.camera.position = r.rotate_point(self.camera.position);
                self.camera.position = self.camera.direction + (
                    -forward - forward.normalize().cross(self.camera.up)
                ).normalize() * forward.magnitude();
                transforms::create_view(self.camera.position, self.camera.direction, self.camera.up)
            },
            "Right" => {
                self.direct = "".into();
                // let r = cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_y(), cgmath::Deg(dt));
                // self.camera.position = r.rotate_point(self.camera.position);
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
        // self.light_animate(self.camera.position);
        self.init.queue.write_buffer(&self.vertex_uniform_buffer, 0, bytemuck::cast_slice(mref));
        self.init.queue.write_buffer(&self.vertex_uniform_buffer, 64, bytemuck::cast_slice(pvref));
        self.init.queue.write_buffer(&self.vertex_uniform_buffer, 128, bytemuck::cast_slice(nref));

        // let forward = self.camera.direction - self.light_instance;
        // self.light_instance = self.camera.direction + (
        //     -forward + forward.normalize().cross(self.camera.up) * dt
        // ).normalize() * forward.magnitude();
        let q = cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_y(), cgmath::Deg(dt));
        self.light_instance = q.rotate_point(self.light_instance);
        // println!("{:?}", self.light_instance);
        let light_pos: [f32; 3] = self.light_instance.into();
        self.init.queue.write_buffer(&self.fragment_uniform_buffer, 0, bytemuck::cast_slice(&[light_pos]));

        // for inst in &mut self.instances {
        //     let amount = cgmath::Quaternion::from_angle_y(Rad(ANIMATION_SPEED));
        //     let current = inst.rotation;
        //     inst.rotation = amount * current;
        // }
        // let inst_data = self.instances
        //     .iter().map(Instance::to_raw)
        //     .collect::<Vec<_>>();
        // self.init.queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&inst_data));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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

            render_pass.set_vertex_buffer(1, self.instance_buffer.slice(..));           
            render_pass.set_pipeline(&self.pipeline);
            render_pass.draw_model_instanced(&self.obj_model, 0..self.instances.len() as u32, &self.uniform_bind_group);
            
            render_pass.set_pipeline(&self.light_render_pipeline);
            render_pass.draw_light_model(&self.obj_model, 0..1, &self.uniform_bind_group);
        }

        self.init.queue.submit(iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}